use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use zeroize::Zeroizing;

pub enum ClipMsg {
    Copy {
        text: Zeroizing<String>,
        clear_after: Option<Duration>,
    },
    ClearNow,
}

#[derive(Clone)]
pub struct ClipboardService {
    tx: Sender<ClipMsg>,
}

impl ClipboardService {
    pub fn start(on_cleared: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel::<ClipMsg>();
        std::thread::Builder::new()
            .name("zpass-clipboard".into())
            .spawn(move || {
                let mut cb = arboard::Clipboard::new().ok();
                let mut pending: Option<(Zeroizing<String>, Instant)> = None;
                loop {
                    let timeout = pending
                        .as_ref()
                        .map(|(_, at)| at.saturating_duration_since(Instant::now()))
                        .unwrap_or(Duration::from_secs(3600));
                    match rx.recv_timeout(timeout) {
                        Ok(ClipMsg::Copy { text, clear_after }) => {
                            if cb.is_none() {
                                cb = arboard::Clipboard::new().ok();
                            }
                            if let Some(c) = cb.as_mut() {
                                set_secret(c, &text);
                            }
                            pending = clear_after.map(|d| (text, Instant::now() + d));
                        }
                        Ok(ClipMsg::ClearNow) => {
                            if let (Some(c), Some((text, _))) = (cb.as_mut(), pending.take()) {
                                clear_if_ours(c, &text);
                                on_cleared();
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            if let Some((text, at)) = pending.as_ref()
                                && Instant::now() >= *at
                            {
                                if let Some(c) = cb.as_mut() {
                                    clear_if_ours(c, text);
                                }
                                pending = None;
                                on_cleared();
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .expect("spawn clipboard thread");
        Self { tx }
    }

    pub fn copy(&self, text: Zeroizing<String>, clear_after: Option<Duration>) {
        let _ = self.tx.send(ClipMsg::Copy { text, clear_after });
    }

    pub fn clear_now(&self) {
        let _ = self.tx.send(ClipMsg::ClearNow);
    }
}

fn set_secret(c: &mut arboard::Clipboard, text: &str) {
    #[cfg(target_os = "linux")]
    {
        use arboard::SetExtLinux;
        let _ = c.set().exclude_from_history().text(text.to_owned());
    }
    #[cfg(windows)]
    {
        use arboard::SetExtWindows;
        let _ = c
            .set()
            .exclude_from_history()
            .exclude_from_cloud()
            .text(text.to_owned());
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = c.set_text(text.to_owned());
    }
}

fn clear_if_ours(c: &mut arboard::Clipboard, text: &str) {
    let ours = c
        .get_text()
        .map(|t| Zeroizing::new(t).as_str() == text)
        .unwrap_or(false);
    if ours {
        let _ = c.set_text(String::new());
        let _ = c.clear();
    }
}
