import { useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { api } from "../../lib/ipc";
import type { ApiError, Status, YubiKeyDevice } from "../../lib/types";
import { useApplyStatus } from "../../app/queries";
import { useUi } from "../../app/store";
import { Button, SecretField, Segmented, TextField } from "../../ui/primitives";
import { Icon } from "../../ui/Icon";
import { StrengthMeter } from "../../features/StrengthMeter";
import { Door } from "../unlock/Door";
import "../unlock/unlock.css";
import "./welcome.css";

const VAULT_FILTER = [{ name: "ZPassword vault", extensions: ["zpv"] }];

export function WelcomeScreen({ status }: { status: Status }) {
  const [creating, setCreating] = useState(false);
  return (
    <Door className="welcome" bolts="drawn">
      {creating ? <CreateVault status={status} onCancel={() => setCreating(false)} /> : <Start status={status} onCreate={() => setCreating(true)} />}
    </Door>
  );
}

function Start({ status, onCreate }: { status: Status; onCreate: () => void }) {
  const apply = useApplyStatus();
  const toast = useUi((s) => s.toast);

  const openFile = async (path?: string) => {
    const p = path ?? (await openDialog({ title: "Open vault", filters: VAULT_FILTER, multiple: false }));
    if (typeof p !== "string") return;
    try {
      apply(await api.selectVault(p));
    } catch (e) {
      toast((e as ApiError).message, "error");
    }
  };

  return (
    <>
      <div className="unlock__head">
        <h1 className="unlock__title plate-type">Your passwords, in one encrypted file on this computer.</h1>
        <p className="welcome__lede">
          ZPassword never syncs or sends anything. Keep the vault file wherever you like and back it up like any other document.
        </p>
      </div>
      <div className="welcome__actions">
        <Button variant="primary" size="lg" icon="plus" onClick={onCreate}>
          Create a vault
        </Button>
        <Button variant="plate" size="lg" icon="vault" onClick={() => openFile()}>
          Open a vault file
        </Button>
      </div>
      {status.recent.length > 0 && (
        <div className="welcome__recent">
          <h2 className="welcome__h2">Recently opened</h2>
          <ul>
            {status.recent.map((r) => (
              <li key={r.path}>
                <button className="welcome__recent-row" disabled={!r.exists} onClick={() => openFile(r.path)}>
                  <span className="welcome__recent-name">{r.name}</span>
                  <span className="welcome__recent-path code">{r.exists ? r.path : "File missing"}</span>
                </button>
                <button className="welcome__recent-x" aria-label={`Remove ${r.name} from this list`} onClick={async () => apply(await api.forgetRecent(r.path))}>
                  <Icon name="close" size={12} />
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </>
  );
}

function CreateVault({ status, onCancel }: { status: Status; onCancel: () => void }) {
  const apply = useApplyStatus();
  const setSetupFlow = useUi((s) => s.setSetupFlow);
  const [step, setStep] = useState(1);
  const [name, setName] = useState("Personal");
  const [path, setPath] = useState(status.defaultVaultPath);
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [score, setScore] = useState(-1);
  const [factor, setFactor] = useState<"none" | "keyfile" | "yubikey">("none");
  const [keyfile, setKeyfile] = useState<string | null>(null);
  const [yubikeys, setYubikeys] = useState<YubiKeyDevice[] | null>(null);
  const [ykSlot, setYkSlot] = useState<"1" | "2">("2");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const choosePath = async () => {
    const p = await saveDialog({ title: "Save vault as", defaultPath: path, filters: VAULT_FILTER });
    if (p) setPath(p.endsWith(".zpv") ? p : `${p}.zpv`);
  };

  const pickKeyfile = async () => {
    const p = await openDialog({ title: "Select keyfile", multiple: false });
    if (typeof p === "string") setKeyfile(p);
  };

  const newKeyfile = async () => {
    const p = await saveDialog({ title: "Save new keyfile", defaultPath: "zpassword.key" });
    if (!p) return;
    try {
      await api.keyfileGenerate(p);
      setKeyfile(p);
    } catch (e) {
      setError((e as ApiError).message);
    }
  };

  const scanYubikey = async () => {
    setError(null);
    try {
      setYubikeys(await api.yubikeyList());
    } catch (e) {
      setError((e as ApiError).message);
      setYubikeys([]);
    }
  };

  const pwError =
    pw.length > 0 && pw.length < 10 ? "Use at least 10 characters." : pw2.length > 0 && pw !== pw2 ? "The two passwords don't match." : null;
  const canNext2 = pw.length >= 10 && pw === pw2 && score >= 2;

  const create = async () => {
    setBusy(true);
    setError(null);
    try {
      const yk = factor === "yubikey" && yubikeys && yubikeys.length > 0 ? { slot: Number(ykSlot), serial: yubikeys[0].serial } : null;
      const s = await api.createVault({ path, name, password: pw, keyfile: factor === "keyfile" ? keyfile : null, yubikey: yk });
      setPw("");
      setPw2("");
      setSetupFlow(true);
      apply(s);
    } catch (e) {
      setError((e as ApiError).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div className="unlock__head">
        <ol className="welcome__steps" aria-label="Steps">
          {["Location", "Master password", "Extra factor"].map((l, i) => (
            <li key={l} className={step === i + 1 ? "is-current" : step > i + 1 ? "is-done" : ""}>
              <span className="code">{i + 1}</span> {l}
            </li>
          ))}
        </ol>
      </div>

      {step === 1 && (
        <div className="unlock__form">
          <TextField label="Vault name" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          <div className="welcome__path">
            <TextField label="File" value={path} mono readOnly hint="You can move or copy this file later; it opens on any computer with ZPassword." />
            <Button variant="plate" onClick={choosePath}>
              Change
            </Button>
          </div>
          <div className="welcome__nav">
            <Button variant="ghost" onClick={onCancel}>
              Back
            </Button>
            <Button variant="primary" onClick={() => setStep(2)} disabled={!path}>
              Continue
            </Button>
          </div>
        </div>
      )}

      {step === 2 && (
        <div className="unlock__form">
          <p className="welcome__lede">
            This is the one password you'll need to remember. Nobody can reset it for you, so you'll set up recovery codes next.
          </p>
          <SecretField label="Master password" value={pw} onChange={(e) => setPw(e.target.value)} autoFocus />
          <StrengthMeter password={pw} onStrength={(s) => setScore(s?.score ?? -1)} />
          <SecretField label="Type it again" value={pw2} onChange={(e) => setPw2(e.target.value)} error={pwError} />
          <div className="welcome__nav">
            <Button variant="ghost" onClick={() => setStep(1)}>
              Back
            </Button>
            <Button variant="primary" onClick={() => setStep(3)} disabled={!canNext2}>
              Continue
            </Button>
          </div>
          {pw.length >= 10 && score >= 0 && score < 2 && <p className="zp-field__hint">Make it a bit stronger to continue.</p>}
        </div>
      )}

      {step === 3 && (
        <div className="unlock__form">
          <p className="welcome__lede">
            Optional: require something you have in addition to the password. Losing it locks you out unless you keep your recovery codes.
          </p>
          <Segmented
            label="Extra factor"
            value={factor}
            onChange={(v) => {
              setFactor(v);
              setError(null);
              if (v === "yubikey" && yubikeys === null) scanYubikey();
            }}
            options={[
              { value: "none", label: "None" },
              { value: "keyfile", label: "Keyfile" },
              { value: "yubikey", label: "YubiKey" },
            ]}
          />
          {factor === "keyfile" && (
            <div className="welcome__factor">
              <p>{keyfile ? <span className="code selectable">{keyfile}</span> : "Pick any file that never changes, or create a new random keyfile."}</p>
              <div className="welcome__row">
                <Button variant="plate" size="sm" icon="file" onClick={pickKeyfile}>
                  Use existing file
                </Button>
                <Button variant="plate" size="sm" icon="plus" onClick={newKeyfile}>
                  Create keyfile
                </Button>
              </div>
            </div>
          )}
          {factor === "yubikey" && (
            <div className="welcome__factor">
              {yubikeys === null ? (
                <p>Looking for keys…</p>
              ) : yubikeys.length === 0 ? (
                <p>No YubiKey found. Insert one configured for HMAC-SHA1 challenge-response, then scan again.</p>
              ) : (
                <p>
                  Found {yubikeys[0].name ?? "YubiKey"}
                  {yubikeys[0].serial ? ` (serial ${yubikeys[0].serial})` : ""}. Choose the slot that holds the challenge-response secret.
                </p>
              )}
              <div className="welcome__row">
                <Segmented label="Slot" value={ykSlot} onChange={setYkSlot} options={[{ value: "1", label: "Slot 1" }, { value: "2", label: "Slot 2" }]} />
                <Button variant="plate" size="sm" icon="usb" onClick={scanYubikey}>
                  Scan again
                </Button>
              </div>
            </div>
          )}
          {error && (
            <p className="zp-field__error" role="alert">
              {error}
            </p>
          )}
          <div className="welcome__nav">
            <Button variant="ghost" onClick={() => setStep(2)} disabled={busy}>
              Back
            </Button>
            <Button
              variant="primary"
              icon="vault"
              busy={busy}
              onClick={create}
              disabled={(factor === "keyfile" && !keyfile) || (factor === "yubikey" && !yubikeys?.length)}
            >
              Create vault
            </Button>
          </div>
          {busy && <p className="zp-field__hint">Tuning encryption strength for this computer{factor === "yubikey" ? " — touch your key if it blinks" : ""}.</p>}
        </div>
      )}
    </>
  );
}
