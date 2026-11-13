use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

pub fn watch(on_lock: Arc<dyn Fn(&'static str) + Send + Sync>) {
    {
        let on_lock = on_lock.clone();
        std::thread::Builder::new()
            .name("zpass-suspend".into())
            .spawn(move || {
                let tick = Duration::from_secs(5);
                let mut mono = Instant::now();
                let mut wall = SystemTime::now();
                loop {
                    std::thread::sleep(tick);
                    let dm = mono.elapsed();
                    let dw = SystemTime::now().duration_since(wall).unwrap_or_default();
                    if dw > dm + Duration::from_secs(20) {
                        on_lock("suspend");
                    }
                    mono = Instant::now();
                    wall = SystemTime::now();
                }
            })
            .ok();
    }
    platform_watch(on_lock);
}

#[cfg(target_os = "linux")]
fn platform_watch(on_lock: Arc<dyn Fn(&'static str) + Send + Sync>) {
    {
        let on_lock = on_lock.clone();
        std::thread::Builder::new()
            .name("zpass-logind".into())
            .spawn(move || {
                let run = || -> zbus::Result<()> {
                    let conn = zbus::blocking::Connection::system()?;
                    let proxy = zbus::blocking::Proxy::new(
                        &conn,
                        "org.freedesktop.login1",
                        "/org/freedesktop/login1/session/auto",
                        "org.freedesktop.login1.Session",
                    )?;
                    for _ in proxy.receive_signal("Lock")? {
                        on_lock("screenLock");
                    }
                    Ok(())
                };
                let _ = run();
            })
            .ok();
    }
    std::thread::Builder::new()
        .name("zpass-screensaver".into())
        .spawn(move || {
            let run = || -> zbus::Result<()> {
                let conn = zbus::blocking::Connection::session()?;
                let proxy = zbus::blocking::Proxy::new(
                    &conn,
                    "org.freedesktop.ScreenSaver",
                    "/org/freedesktop/ScreenSaver",
                    "org.freedesktop.ScreenSaver",
                )?;
                for msg in proxy.receive_signal("ActiveChanged")? {
                    if msg.body().deserialize::<bool>().unwrap_or(false) {
                        on_lock("screenLock");
                    }
                }
                Ok(())
            };
            let _ = run();
        })
        .ok();
}

#[cfg(windows)]
fn platform_watch(on_lock: Arc<dyn Fn(&'static str) + Send + Sync>) {
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_SWITCHDESKTOP, OpenInputDesktop,
    };
    std::thread::Builder::new()
        .name("zpass-winlock".into())
        .spawn(move || {
            let mut was_locked = false;
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let locked = unsafe {
                    match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_SWITCHDESKTOP) {
                        Ok(h) => {
                            let _ = CloseDesktop(h);
                            false
                        }
                        Err(_) => true,
                    }
                };
                if locked && !was_locked {
                    on_lock("screenLock");
                }
                was_locked = locked;
            }
        })
        .ok();
}

#[cfg(not(any(windows, target_os = "linux")))]
fn platform_watch(_on_lock: Arc<dyn Fn(&'static str) + Send + Sync>) {}
