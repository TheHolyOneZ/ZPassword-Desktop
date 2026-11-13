import { useEffect, useRef, useState, type FormEvent } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api } from "../../lib/ipc";
import type { ApiError, Status } from "../../lib/types";
import { useApplyStatus } from "../../app/queries";
import { Button, SecretField, TextField } from "../../ui/primitives";
import { Icon } from "../../ui/Icon";
import { Door, type DoorHandle } from "./Door";
import "./unlock.css";

type Method = "password" | "pin" | "device" | "recoveryCode" | "questions";

const deviceName = (b: string | null) => (b === "windowsHello" ? "Windows Hello" : "Device keyring");

export function UnlockScreen({ status }: { status: Status }) {
  const info = status.unlock!;
  const apply = useApplyStatus();
  const dial = useRef<DoorHandle>(null);
  const [method, setMethod] = useState<Method>(info.pinAttempts ? "pin" : "password");
  const [secret, setSecret] = useState("");
  const [answers, setAnswers] = useState<string[]>(() => (info.recoveryQuestions ?? []).map(() => ""));
  const [keyfile, setKeyfile] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setSecret("");
    setError(null);
    dial.current?.turnTo(0);
    inputRef.current?.focus();
  }, [method]);

  const onType = (v: string) => {
    setSecret(v);
    setError(null);
    dial.current?.turnTo(v.length);
  };

  async function run(fn: () => Promise<Status>) {
    setBusy(true);
    setError(null);
    dial.current?.working(true);
    try {
      const next = await fn();
      await dial.current?.open();
      setSecret("");
      apply(next);
    } catch (e) {
      const err = e as ApiError;
      dial.current?.reject();
      setError(err.message);
      if (err.code === "pinExhausted") setMethod("password");
      apply(await api.status());
      requestAnimationFrame(() => inputRef.current?.select());
    } finally {
      setBusy(false);
    }
  }

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (busy) return;
    switch (method) {
      case "password":
        if (!secret) return;
        if (info.keyfile && !keyfile) return setError("Select this vault's keyfile first.");
        return run(() => api.unlockPassword(secret, keyfile));
      case "pin":
        if (!secret) return;
        return run(() => api.unlockPin(secret));
      case "device":
        return run(() => api.unlockDevice());
      case "recoveryCode":
        if (!secret) return;
        return run(() => api.unlockRecoveryCode(secret));
      case "questions":
        return run(() => api.unlockRecoveryQuestions(answers));
    }
  };

  const pickKeyfile = async () => {
    const p = await openDialog({ title: "Select keyfile", multiple: false, directory: false });
    if (typeof p === "string") setKeyfile(p);
  };

  const recoveryAvailable = info.recoveryCodes > 0 || !!info.recoveryQuestions;

  return (
    <Door ref={dial}>
        <div className="unlock__head">
          <h1 className="unlock__title plate-type">{status.vaultLabel ?? "Vault"}</h1>
          <p className="unlock__path code selectable" title={status.vaultPath ?? ""}>
            {status.vaultPath}
          </p>
        </div>

        <form className="unlock__form" onSubmit={submit}>
          {method === "password" && (
            <>
              <SecretField
                ref={inputRef}
                label="Master password"
                value={secret}
                onChange={(e) => onType(e.target.value)}
                autoFocus
                error={error}
                disabled={busy}
              />
              {info.keyfile && (
                <button type="button" className={`unlock__factor ${keyfile ? "is-set" : ""}`} onClick={pickKeyfile}>
                  <Icon name="file" />
                  <span>{keyfile ? keyfile.split(/[\\/]/).pop() : "Select keyfile"}</span>
                </button>
              )}
              {info.yubikey && (
                <p className="unlock__factor is-static">
                  <Icon name="usb" />
                  <span>Insert your YubiKey. Touch it when it blinks after you press Open.</span>
                </p>
              )}
            </>
          )}

          {method === "pin" && (
            <TextField
              ref={inputRef}
              label={`Quick-unlock PIN (${info.pinAttempts} ${info.pinAttempts === 1 ? "try" : "tries"} left)`}
              type="password"
              inputMode="numeric"
              mono
              value={secret}
              onChange={(e) => onType(e.target.value.replace(/\D/g, ""))}
              autoFocus
              error={error}
              disabled={busy}
            />
          )}

          {method === "device" && (
            <div className="unlock__device">
              <Icon name="fingerprint" size={28} />
              <p>{info.deviceUnlock === "windowsHello" ? "Windows will ask you to verify it's you." : "Uses the key stored in your desktop keyring for this vault."}</p>
              {error && (
                <p className="zp-field__error" role="alert">
                  {error}
                </p>
              )}
            </div>
          )}

          {method === "recoveryCode" && (
            <TextField
              ref={inputRef}
              label="Recovery code"
              placeholder="XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
              mono
              value={secret}
              onChange={(e) => onType(e.target.value)}
              autoFocus
              error={error}
              disabled={busy}
              hint="Each code works once. You'll set a new master password right after."
            />
          )}

          {method === "questions" && info.recoveryQuestions && (
            <div className="unlock__questions">
              {info.recoveryQuestions.map((q, i) => (
                <TextField
                  key={i}
                  ref={i === 0 ? inputRef : undefined}
                  label={q}
                  value={answers[i] ?? ""}
                  onChange={(e) => {
                    const next = [...answers];
                    next[i] = e.target.value;
                    setAnswers(next);
                    dial.current?.turnTo(next.join("").length);
                  }}
                  disabled={busy}
                />
              ))}
              {error && (
                <p className="zp-field__error" role="alert">
                  {error}
                </p>
              )}
              <p className="zp-field__hint">Answers ignore capitals, spaces and punctuation. You'll set a new master password right after.</p>
            </div>
          )}

          <Button type="submit" variant="primary" size="lg" busy={busy} icon="unlock" className="unlock__submit">
            {method === "device" ? `Use ${deviceName(info.deviceUnlock)}` : "Open vault"}
          </Button>
        </form>

        <nav className="unlock__methods" aria-label="Other ways to unlock">
          {method !== "password" && (
            <button onClick={() => setMethod("password")}>
              <Icon name="key" size={14} /> Master password
            </button>
          )}
          {info.pinAttempts !== null && method !== "pin" && (
            <button onClick={() => setMethod("pin")}>
              <Icon name="hash" size={14} /> PIN
            </button>
          )}
          {info.deviceUnlock && method !== "device" && (
            <button onClick={() => setMethod("device")}>
              <Icon name="fingerprint" size={14} /> {deviceName(info.deviceUnlock)}
            </button>
          )}
          {info.recoveryCodes > 0 && method !== "recoveryCode" && (
            <button onClick={() => setMethod("recoveryCode")}>
              <Icon name="restore" size={14} /> Recovery code
            </button>
          )}
          {info.recoveryQuestions && method !== "questions" && (
            <button onClick={() => setMethod("questions")}>
              <Icon name="shield" size={14} /> Security questions
            </button>
          )}
          {!recoveryAvailable && method === "password" && (
            <span className="unlock__note">No recovery method is set up for this vault.</span>
          )}
        </nav>

        <div className="unlock__foot">
          <Button variant="ghost" size="sm" icon="vault" onClick={async () => apply(await api.closeVault())}>
            Open a different vault
          </Button>
        </div>
    </Door>
  );
}
