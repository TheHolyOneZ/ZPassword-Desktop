use std::sync::Mutex;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tauri::Manager;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;

use crate::config::AppConfig;
use crate::platform::clipboard::ClipboardService;
use crate::state::{AppState, Session};

struct Harness {
    _app: tauri::App<MockRuntime>,
    webview: tauri::WebviewWindow<MockRuntime>,
    dir: tempfile::TempDir,
}

#[derive(Debug, serde::Deserialize)]
struct Err {
    code: String,
    #[allow(dead_code)]
    message: String,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let app = mock_builder()
            .manage(AppState {
                session: Mutex::new(Session::default()),
                config: Mutex::new(AppConfig::default()),
                config_path: dir.path().join("config.json"),
                default_vault: dir.path().join("vault.zpv"),
                clipboard: ClipboardService::start(|| {}),
            })
            .invoke_handler(crate::handler())
            .build(mock_context(noop_assets()))
            .unwrap();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        Self {
            _app: app,
            webview,
            dir,
        }
    }

    fn try_call<T: DeserializeOwned>(&self, cmd: &str, args: Value) -> Result<T, Err> {
        let req = InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: self.webview.url().unwrap(),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        };
        match get_ipc_response(&self.webview, req) {
            Ok(body) => Ok(body.deserialize::<T>().unwrap()),
            Err(v) => Err(serde_json::from_value(v).unwrap()),
        }
    }

    fn call<T: DeserializeOwned>(&self, cmd: &str, args: Value) -> T {
        self.try_call(cmd, args)
            .unwrap_or_else(|e| panic!("{cmd} failed: {e:?}"))
    }

    fn stage(&self) -> String {
        self.call::<Value>("app_status", json!({}))["stage"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn lock(&self) {
        let _: Value = self.call("lock", json!({}));
        assert_eq!(self.stage(), "locked");
    }
}

fn login_input(title: &str, password: &str) -> Value {
    json!({
        "template": "login",
        "title": title,
        "description": "",
        "folderId": null,
        "labels": [],
        "favorite": false,
        "fields": [
            {"id": null, "name": "Username", "kind": "username", "concealed": false, "value": "me"},
            {"id": null, "name": "Password", "kind": "password", "concealed": true, "value": password}
        ]
    })
}

#[test]
fn full_session_lifecycle() {
    let h = Harness::new();
    assert_eq!(h.stage(), "noVault");
    let path = h.dir.path().join("e2e.zpv");
    let master = "correct horse battery staple";

    let status: Value = h.call(
        "vault_create",
        json!({"path": path, "name": "E2E", "password": master, "keyfile": null, "yubikey": null}),
    );
    assert_eq!(status["stage"], "unlocked");
    assert!(path.exists());

    let id: String = h.call("item_create", json!({"input": login_input("Example", "first-secret")}));
    let detail: Value = h.call("item_get", json!({"id": id}));
    let pw_field = detail["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["kind"] == "password")
        .unwrap()
        .clone();
    assert!(pw_field["value"].is_null(), "concealed value leaked to the UI");
    let fid = pw_field["id"].as_str().unwrap().to_string();
    let revealed: String = h.call("item_reveal", json!({"id": id, "fieldId": fid}));
    assert_eq!(revealed, "first-secret");

    let mut input = login_input("Example", "x");
    input["fields"][1]["id"] = json!(fid);
    input["fields"][1]["value"] = Value::Null;
    let _: Value = h.call("item_update", json!({"id": id, "input": input.clone()}));
    assert_eq!(
        h.call::<String>("item_reveal", json!({"id": id, "fieldId": fid})),
        "first-secret"
    );
    input["fields"][1]["value"] = json!("second-secret");
    let _: Value = h.call("item_update", json!({"id": id, "input": input}));
    let detail: Value = h.call("item_get", json!({"id": id}));
    assert_eq!(detail["history"].as_array().unwrap().len(), 1);
    assert_eq!(
        h.call::<String>("history_reveal", json!({"id": id, "index": 0})),
        "first-secret"
    );

    let snap: Value = h.call("vault_snapshot", json!({}));
    assert!(!snap.to_string().contains("second-secret"));

    h.lock();
    assert!(h.try_call::<Value>("vault_snapshot", json!({})).is_err());
    let e = h
        .try_call::<Value>("unlock_password", json!({"password": "wrong", "keyfile": null}))
        .unwrap_err();
    assert_eq!(e.code, "invalidCredentials");
    let s: Value = h.call("unlock_password", json!({"password": master, "keyfile": null}));
    assert_eq!(s["stage"], "unlocked");

    let codes: Vec<String> = h.call("recovery_codes_generate", json!({}));
    assert_eq!(codes.len(), 10);
    h.lock();
    let s: Value = h.call("unlock_recovery_code", json!({"code": codes[0]}));
    assert_eq!(s["stage"], "needsPasswordReset");
    assert!(
        h.try_call::<Value>("vault_snapshot", json!({})).is_err(),
        "vault usable before reset"
    );
    let new_master = "a whole new master password";
    let s: Value = h.call(
        "master_password_set",
        json!({"current": null, "password": new_master, "keyfile": null, "currentKeyfile": null, "yubikey": null}),
    );
    assert_eq!(s["stage"], "unlocked");
    h.lock();
    assert!(
        h.try_call::<Value>("unlock_recovery_code", json!({"code": codes[0]}))
            .is_err(),
        "recovery code reused"
    );
    let _: Value = h.call("unlock_password", json!({"password": new_master, "keyfile": null}));

    let e = h
        .try_call::<Value>(
            "master_password_set",
            json!({"current": "nope", "password": master, "keyfile": null, "currentKeyfile": null, "yubikey": null}),
        )
        .unwrap_err();
    assert_eq!(e.code, "invalidCredentials");

    let _: Value = h.call("pin_set", json!({"pin": "4821"}));
    h.lock();
    assert!(h.try_call::<Value>("unlock_pin", json!({"pin": "0000"})).is_err());
    let s: Value = h.call("unlock_pin", json!({"pin": "4821"}));
    assert_eq!(s["stage"], "unlocked");
    h.lock();
    for _ in 0..4 {
        assert_eq!(
            h.try_call::<Value>("unlock_pin", json!({"pin": "0000"}))
                .unwrap_err()
                .code,
            "invalidCredentials"
        );
    }
    assert_eq!(
        h.try_call::<Value>("unlock_pin", json!({"pin": "0000"}))
            .unwrap_err()
            .code,
        "pinExhausted"
    );
    assert!(h.call::<Value>("app_status", json!({}))["unlock"]["pinAttempts"].is_null());
    let _: Value = h.call("unlock_password", json!({"password": new_master, "keyfile": null}));

    let enroll: Value = h.call("totp_enroll_begin", json!({}));
    let cfg = zpass_core::totp::parse(enroll["uri"].as_str().unwrap()).unwrap();
    let now = zpass_core::util::now() as u64;
    let _: Value = h.call(
        "totp_enroll_confirm",
        json!({"code": zpass_core::totp::code_at(&cfg, now)}),
    );
    h.lock();
    let s: Value = h.call("unlock_password", json!({"password": new_master, "keyfile": null}));
    assert_eq!(s["stage"], "awaitingTotp");
    assert!(
        h.try_call::<Value>("vault_snapshot", json!({})).is_err(),
        "vault usable before TOTP"
    );
    assert!(h.try_call::<Value>("verify_totp", json!({"code": "000000"})).is_err());
    let s: Value = h.call(
        "verify_totp",
        json!({"code": zpass_core::totp::code_at(&cfg, now + 30)}),
    );
    assert_eq!(s["stage"], "unlocked");

    let csv = h.dir.path().join("chrome.csv");
    std::fs::write(
        &csv,
        "name,url,username,password,note\nShop,https://shop.example,me,pw,\nExample,,me,second-secret,\n",
    )
    .unwrap();
    let summary: Value = h.call(
        "import_preview",
        json!({"path": csv, "password": null, "keyfile": null}),
    );
    assert_eq!(summary["items"], 2);
    assert_eq!(summary["duplicates"], 1);
    let report: Value = h.call(
        "import_commit",
        json!({"skipDuplicates": true, "intoFolder": "Imported"}),
    );
    assert_eq!(report["added"], 1);
    assert_eq!(report["skipped"], 1);

    let backup = h.dir.path().join("backup.zpv");
    let _: Value = h.call(
        "export_vault",
        json!({"path": backup, "format": "zpassword", "password": "backup password 123"}),
    );
    let summary: Value = h.call(
        "import_preview",
        json!({"path": backup, "password": "backup password 123", "keyfile": null}),
    );
    assert_eq!(summary["items"], 2);
    assert_eq!(summary["duplicates"], 2);
    let _: () = h.call("import_cancel", json!({}));

    let _: Value = h.call("items_trash", json!({"ids": [id]}));
    let snap: Value = h.call("vault_snapshot", json!({}));
    assert!(
        snap["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["id"] == json!(id) && !i["trashedAt"].is_null())
    );

    assert!(zpass_core::storage::VaultLock::acquire(&path).is_err());

    h.lock();
    for _ in 0..3 {
        let _ = h.try_call::<Value>("unlock_password", json!({"password": "bad", "keyfile": null}));
    }
    let e = h
        .try_call::<Value>("unlock_password", json!({"password": new_master, "keyfile": null}))
        .unwrap_err();
    assert_eq!(e.code, "throttled");

    assert!(!zpass_core::storage::list_backups(&path).unwrap().is_empty());
    let _ = h._app.state::<AppState>();
}
