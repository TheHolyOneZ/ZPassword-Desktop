import { useRef, useState, type FormEvent } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api } from "../../lib/ipc";
import type { ApiError, Status } from "../../lib/types";
import { useApplyStatus } from "../../app/queries";
import { Button, SecretField, Switch, TextField } from "../../ui/primitives";
import { StrengthMeter } from "../../features/StrengthMeter";
import { Door, type DoorHandle } from "../unlock/Door";
import "../unlock/unlock.css";
import "../welcome/welcome.css";

export function TotpGate({ status }: { status: Status }) {
  const apply = useApplyStatus();
  const dial = useRef<DoorHandle>(null);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (e?: FormEvent) => {
    e?.preventDefault();
    if (code.length < 6 || busy) return;
    setBusy(true);
    setError(null);
    try {
      const s = await api.verifyTotp(code);
      await dial.current?.open();
      apply(s);
    } catch (err) {
      dial.current?.reject();
      setError((err as ApiError).message);
      setCode("");
      apply(await api.status());
    } finally {
      setBusy(false);
    }
  };

  return (
    <Door ref={dial}>
        <div className="unlock__head">
          <h1 className="unlock__title plate-type">One more step</h1>
          <p className="welcome__lede">Enter the 6-digit code from your authenticator app for {status.vaultLabel}.</p>
        </div>
        <form className="unlock__form" onSubmit={submit}>
          <TextField
            label="Authenticator code"
            inputMode="numeric"
            autoComplete="one-time-code"
            mono
            autoFocus
            value={code}
            maxLength={8}
            onChange={(e) => {
              const v = e.target.value.replace(/\D/g, "");
              setCode(v);
              dial.current?.turnTo(v.length);
            }}
            error={error}
            disabled={busy}
          />
          <Button type="submit" variant="primary" size="lg" busy={busy} icon="check" className="unlock__submit" disabled={code.length < 6}>
            Verify
          </Button>
        </form>
        <div className="unlock__foot">
          <Button variant="ghost" size="sm" icon="lock" onClick={async () => apply(await api.lock())}>
            Cancel and lock
          </Button>
        </div>
    </Door>
  );
}

export function ResetPasswordScreen({ status }: { status: Status }) {
  const apply = useApplyStatus();
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [score, setScore] = useState(-1);
  const [keepKeyfile, setKeyfile] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hadKeyfile = status.unlock?.keyfile ?? false;
  const yk = status.unlock?.yubikey ?? null;
  const [keepYk, setKeepYk] = useState(true);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const s = await api.setMasterPassword({ current: null, password: pw, keyfile: keepKeyfile, currentKeyfile: null, yubikey: keepYk ? yk : null });
      setPw("");
      setPw2("");
      apply(s);
    } catch (err) {
      setError((err as ApiError).message);
    } finally {
      setBusy(false);
    }
  };

  const mismatch = pw2.length > 0 && pw !== pw2 ? "The two passwords don't match." : null;
  return (
    <Door>
        <div className="unlock__head">
          <h1 className="unlock__title plate-type">Set a new master password</h1>
          <p className="welcome__lede">
            You opened the vault with a recovery method. Choose a new master password before continuing. Your other recovery codes still work.
          </p>
        </div>
        <form className="unlock__form" onSubmit={save}>
          <SecretField label="New master password" value={pw} onChange={(e) => setPw(e.target.value)} autoFocus />
          <StrengthMeter password={pw} onStrength={(s) => setScore(s?.score ?? -1)} />
          <SecretField label="Type it again" value={pw2} onChange={(e) => setPw2(e.target.value)} error={mismatch ?? error} />
          {hadKeyfile && (
            <Button
              type="button"
              variant="plate"
              size="sm"
              icon="file"
              onClick={async () => {
                const p = await openDialog({ title: "Keyfile for the new password", multiple: false });
                if (typeof p === "string") setKeyfile(p);
              }}
            >
              {keepKeyfile ? keepKeyfile.split(/[\\/]/).pop() : "Keep using a keyfile (optional)"}
            </Button>
          )}
          {yk && (
            <Switch
              label="Keep requiring my YubiKey"
              description={keepYk ? "Insert it now and touch it when it blinks after you save." : "Turn this off if you lost the key. You can add a key again in Security."}
              checked={keepYk}
              onChange={setKeepYk}
            />
          )}
          <Button type="submit" variant="primary" size="lg" busy={busy} className="unlock__submit" disabled={pw.length < 10 || pw !== pw2 || score < 2}>
            Save new password
          </Button>
        </form>
    </Door>
  );
}
