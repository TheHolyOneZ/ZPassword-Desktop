import { useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { api } from "../../lib/ipc";
import type { ApiError, SecurityOverview, TotpEnrollment } from "../../lib/types";
import { relative } from "../../lib/format";
import { keys, useApplyStatus } from "../../app/queries";
import { useUi } from "../../app/store";
import { Button, Checkbox, Detent, SecretField, Segmented, Sheet, TextField } from "../../ui/primitives";
import { Icon, type IconName } from "../../ui/Icon";
import { StrengthMeter } from "../../features/StrengthMeter";
import "./security.css";

function useSecurity(enabled: boolean) {
  return useQuery({ queryKey: keys.security, queryFn: api.securityOverview, enabled, staleTime: 0 });
}

function useRefresh() {
  const qc = useQueryClient();
  return () => {
    qc.invalidateQueries({ queryKey: keys.security });
    qc.invalidateQueries({ queryKey: keys.status });
  };
}

function Section({ icon, title, state, children }: { icon: IconName; title: string; state: ReactNode; children: ReactNode }) {
  return (
    <section className="sec">
      <header className="sec__head">
        <Icon name={icon} size={18} />
        <h3 className="sec__title">{title}</h3>
        <span className="sec__state">{state}</span>
      </header>
      <div className="sec__body">{children}</div>
    </section>
  );
}

const On = ({ children = "On" }: { children?: ReactNode }) => <span className="sec__on">{children}</span>;
const Off = ({ children = "Off" }: { children?: ReactNode }) => <span className="sec__off">{children}</span>;

function Err({ e }: { e: string | null }) {
  return e ? (
    <p className="zp-field__error" role="alert">
      {e}
    </p>
  ) : null;
}

function MasterPassword({ o }: { o: SecurityOverview }) {
  const apply = useApplyStatus();
  const refresh = useRefresh();
  const toast = useUi((s) => s.toast);
  const [open, setOpen] = useState(false);
  const [current, setCurrent] = useState("");
  const [currentKeyfile, setCurrentKeyfile] = useState<string | null>(null);
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [score, setScore] = useState(-1);
  const [factor, setFactor] = useState<"none" | "keyfile" | "yubikey">(o.yubikey ? "yubikey" : o.keyfile ? "keyfile" : "none");
  const [keyfile, setKeyfile] = useState<string | null>(null);
  const [slot, setSlot] = useState<"1" | "2">(String(o.yubikey?.slot ?? 2) as "1" | "2");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      let yubikey = null;
      if (factor === "yubikey") {
        const devs = await api.yubikeyList();
        if (!devs.length) throw { message: "No YubiKey found. Insert it and try again." };
        yubikey = { slot: Number(slot), serial: devs[0].serial };
      }
      apply(await api.setMasterPassword({ current, password: pw, keyfile: factor === "keyfile" ? keyfile : null, currentKeyfile, yubikey }));
      setOpen(false);
      setCurrent("");
      setPw("");
      setPw2("");
      refresh();
      toast("Master password changed.", "ok");
    } catch (e) {
      setError((e as ApiError).message);
    } finally {
      setBusy(false);
    }
  };

  const extras = [o.keyfile && "keyfile", o.yubikey && `YubiKey slot ${o.yubikey.slot}`].filter(Boolean).join(" and ");
  return (
    <Section icon="key" title="Master password" state={<On>{extras ? `Plus ${extras}` : "Password only"}</On>}>
      <p className="sec__desc">
        Stretched with Argon2id ({o.kdfMemoryMib} MiB, {o.kdfPasses} passes) before it unlocks anything.
      </p>
      {!open ? (
        <Button size="sm" onClick={() => setOpen(true)}>
          Change master password
        </Button>
      ) : (
        <div className="sec__form">
          <SecretField label="Current master password" value={current} onChange={(e) => setCurrent(e.target.value)} />
          {o.keyfile && (
            <Button
              size="sm"
              icon="file"
              onClick={async () => {
                const p = await openDialog({ title: "Current keyfile", multiple: false });
                if (typeof p === "string") setCurrentKeyfile(p);
              }}
            >
              {currentKeyfile ? currentKeyfile.split(/[\\/]/).pop() : "Select current keyfile"}
            </Button>
          )}
          <SecretField label="New master password" value={pw} onChange={(e) => setPw(e.target.value)} />
          <StrengthMeter password={pw} onStrength={(s) => setScore(s?.score ?? -1)} />
          <SecretField label="Type it again" value={pw2} onChange={(e) => setPw2(e.target.value)} error={pw2 && pw !== pw2 ? "The two passwords don't match." : null} />
          <Segmented
            label="Extra factor"
            value={factor}
            onChange={setFactor}
            options={[
              { value: "none", label: "No extra factor" },
              { value: "keyfile", label: "Keyfile" },
              { value: "yubikey", label: "YubiKey" },
            ]}
          />
          {factor === "keyfile" && (
            <div className="sec__row">
              <Button
                size="sm"
                icon="file"
                onClick={async () => {
                  const p = await openDialog({ title: "Keyfile", multiple: false });
                  if (typeof p === "string") setKeyfile(p);
                }}
              >
                {keyfile ? keyfile.split(/[\\/]/).pop() : "Choose keyfile"}
              </Button>
              <Button
                size="sm"
                icon="plus"
                onClick={async () => {
                  const p = await saveDialog({ title: "Save new keyfile", defaultPath: "zpassword.key" });
                  if (!p) return;
                  try {
                    await api.keyfileGenerate(p);
                    setKeyfile(p);
                  } catch (e) {
                    setError((e as ApiError).message);
                  }
                }}
              >
                Create new keyfile
              </Button>
            </div>
          )}
          {factor === "yubikey" && (
            <Segmented label="YubiKey slot" value={slot} onChange={setSlot} options={[{ value: "1", label: "Slot 1" }, { value: "2", label: "Slot 2" }]} />
          )}
          <Err e={error} />
          <div className="sec__row">
            <Button variant="ghost" size="sm" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              busy={busy}
              onClick={submit}
              disabled={!current || pw.length < 10 || pw !== pw2 || score < 2 || (factor === "keyfile" && !keyfile)}
            >
              Change password
            </Button>
          </div>
        </div>
      )}
    </Section>
  );
}

export function RecoveryCodes({ o, firstRun = false, onDone }: { o: SecurityOverview; firstRun?: boolean; onDone?: () => void }) {
  const refresh = useRefresh();
  const toast = useUi((s) => s.toast);
  const [codes, setCodes] = useState<string[] | null>(null);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);

  const generate = async () => {
    setBusy(true);
    try {
      setCodes(await api.recoveryCodesGenerate());
      setSaved(false);
      refresh();
    } catch (e) {
      toast((e as ApiError).message, "error");
    } finally {
      setBusy(false);
    }
  };

  const text = () => `ZPassword recovery codes\nEach code unlocks the vault once.\n\n${codes!.join("\n")}\n`;

  return (
    <Section icon="restore" title="Recovery codes" state={o.recoveryCodes ? <On>{o.recoveryCodes} left</On> : <Off>None</Off>}>
      <p className="sec__desc">
        If you forget the master password, any one of these opens the vault once and asks you to set a new password. Store them away from this computer.
      </p>
      {codes ? (
        <div className="codes">
          <ol className="codes__grid code selectable">
            {codes.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ol>
          <div className="sec__row">
            <Button
              size="sm"
              icon="download"
              onClick={async () => {
                const p = await saveDialog({ title: "Save recovery codes", defaultPath: "zpassword-recovery-codes.txt" });
                if (!p) return;
                try {
                  await api.exportText(p, text());
                  setSaved(true);
                  toast("Recovery codes saved.", "ok");
                } catch (e) {
                  toast((e as ApiError).message, "error");
                }
              }}
            >
              Save as text file
            </Button>
            <Button
              size="sm"
              icon="copy"
              onClick={async () => {
                await api.clipboardCopy(codes.join("\n"), true);
                setSaved(true);
                toast("Copied. Paste them somewhere safe; the clipboard clears soon.", "ok");
              }}
            >
              Copy
            </Button>
          </div>
          <Checkbox checked={saved} onChange={setSaved}>
            I've stored these codes somewhere safe
          </Checkbox>
          <Button
            variant="primary"
            size="sm"
            disabled={!saved}
            onClick={() => {
              setCodes(null);
              onDone?.();
            }}
          >
            {firstRun ? "Continue" : "Done"}
          </Button>
        </div>
      ) : (
        <div className="sec__row">
          <Button size="sm" variant={firstRun ? "primary" : "plate"} busy={busy} onClick={generate}>
            {o.recoveryCodes ? "Replace all codes" : "Generate codes"}
          </Button>
          {o.recoveryCodes > 0 && !firstRun && (
            <Button
              size="sm"
              variant="danger"
              onClick={async () => {
                await api.recoveryCodesClear();
                refresh();
              }}
            >
              Remove codes
            </Button>
          )}
        </div>
      )}
      {o.recoveryCodes > 0 && o.recoveryCodes <= 3 && !codes && <p className="sec__warn">Only {o.recoveryCodes} left. Generate a fresh set.</p>}
    </Section>
  );
}

const SUGGESTED = [
  "What was the name of your first pet?",
  "What street did you grow up on?",
  "What was the model of your first car?",
  "What is the name of the town where your parents met?",
  "What was the first concert you went to?",
];

function Questions({ o }: { o: SecurityOverview }) {
  const refresh = useRefresh();
  const toast = useUi((s) => s.toast);
  const [open, setOpen] = useState(false);
  const [qs, setQs] = useState<string[]>([SUGGESTED[0], SUGGESTED[1], SUGGESTED[2]]);
  const [as, setAs] = useState<string[]>(["", "", ""]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.recoveryQuestionsSet(qs, as);
      setOpen(false);
      setAs(["", "", ""]);
      refresh();
      toast("Security questions saved.", "ok");
    } catch (e) {
      setError((e as ApiError).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Section icon="shield" title="Security questions" state={o.recoveryQuestions ? <On>{o.recoveryQuestions.length} questions</On> : <Off />}>
      <p className="sec__desc">
        All answers together unlock the vault once and force a new password. Answers that others could look up make this the weakest way in, so pick obscure ones.
      </p>
      {open ? (
        <div className="sec__form">
          {qs.map((q, i) => (
            <div key={i} className="sec__qa">
              <TextField label={`Question ${i + 1}`} value={q} list="zp-questions" onChange={(e) => setQs(qs.map((x, j) => (j === i ? e.target.value : x)))} />
              <SecretField label="Answer" value={as[i]} onChange={(e) => setAs(as.map((x, j) => (j === i ? e.target.value : x)))} />
            </div>
          ))}
          <datalist id="zp-questions">
            {SUGGESTED.map((s) => (
              <option key={s} value={s} />
            ))}
          </datalist>
          <Err e={error} />
          <div className="sec__row">
            <Button variant="ghost" size="sm" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button variant="primary" size="sm" busy={busy} disabled={as.some((a) => a.trim().length < 3) || qs.some((q) => !q.trim())} onClick={save}>
              Save questions
            </Button>
          </div>
        </div>
      ) : (
        <div className="sec__row">
          <Button size="sm" onClick={() => setOpen(true)}>
            {o.recoveryQuestions ? "Replace questions" : "Set up questions"}
          </Button>
          {o.recoveryQuestions && (
            <Button
              size="sm"
              variant="danger"
              onClick={async () => {
                await api.recoveryQuestionsClear();
                refresh();
              }}
            >
              Remove
            </Button>
          )}
        </div>
      )}
    </Section>
  );
}

function Authenticator({ o }: { o: SecurityOverview }) {
  const refresh = useRefresh();
  const toast = useUi((s) => s.toast);
  const [enroll, setEnroll] = useState<TotpEnrollment | null>(null);
  const [disabling, setDisabling] = useState(false);
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);

  const begin = async () => {
    setError(null);
    try {
      setEnroll(await api.totpEnrollBegin());
    } catch (e) {
      setError((e as ApiError).message);
    }
  };

  const confirm = async () => {
    setError(null);
    try {
      if (disabling) await api.totpDisable(code);
      else await api.totpEnrollConfirm(code);
      toast(disabling ? "Authenticator turned off." : "Authenticator turned on.", "ok");
      setEnroll(null);
      setDisabling(false);
      setCode("");
      refresh();
    } catch (e) {
      setError((e as ApiError).message);
    }
  };

  return (
    <Section icon="clock" title="Authenticator app" state={o.totp ? <On /> : <Off />}>
      <p className="sec__desc">
        Asks for a 6-digit code from an app such as Google Authenticator, Aegis or 2FAS after the password. It stops someone who learned your password at this
        keyboard. Recovery codes and the quick-unlock PIN skip this step.
      </p>
      {enroll ? (
        <div className="totp-enroll">
          <div className="totp-enroll__qr" dangerouslySetInnerHTML={{ __html: enroll.qrSvg }} />
          <div className="sec__form">
            <p className="sec__desc">Scan the code, or type this key into the app:</p>
            <p className="code selectable totp-enroll__secret">{enroll.secret}</p>
            <TextField label="Code from the app" mono inputMode="numeric" value={code} autoFocus onChange={(e) => setCode(e.target.value.replace(/\D/g, ""))} />
            <Err e={error} />
            <div className="sec__row">
              <Button variant="ghost" size="sm" onClick={() => setEnroll(null)}>
                Cancel
              </Button>
              <Button variant="primary" size="sm" disabled={code.length < 6} onClick={confirm}>
                Turn on
              </Button>
            </div>
          </div>
        </div>
      ) : disabling ? (
        <div className="sec__form">
          <TextField label="Current code from the app" mono inputMode="numeric" value={code} autoFocus onChange={(e) => setCode(e.target.value.replace(/\D/g, ""))} />
          <Err e={error} />
          <div className="sec__row">
            <Button variant="ghost" size="sm" onClick={() => setDisabling(false)}>
              Cancel
            </Button>
            <Button variant="danger" size="sm" disabled={code.length < 6} onClick={confirm}>
              Turn off
            </Button>
          </div>
        </div>
      ) : (
        <>
          <Err e={error} />
          {o.totp ? (
            <Button size="sm" variant="danger" onClick={() => setDisabling(true)}>
              Turn off
            </Button>
          ) : (
            <Button size="sm" onClick={begin}>
              Set up authenticator
            </Button>
          )}
        </>
      )}
    </Section>
  );
}

function Pin({ o }: { o: SecurityOverview }) {
  const refresh = useRefresh();
  const toast = useUi((s) => s.toast);
  const [open, setOpen] = useState(false);
  const [pin, setPin] = useState("");
  const [error, setError] = useState<string | null>(null);
  return (
    <Section icon="hash" title="Quick-unlock PIN" state={o.pin ? <On /> : <Off />}>
      <p className="sec__desc">
        After you've unlocked once, a short PIN re-opens the vault while ZPassword keeps running. Five wrong tries or a restart and the full password is needed
        again. The PIN never touches the disk outside the encrypted vault.
      </p>
      {open ? (
        <div className="sec__form sec__form--narrow">
          <TextField label="PIN (4–12 digits)" type="password" mono inputMode="numeric" autoFocus value={pin} onChange={(e) => setPin(e.target.value.replace(/\D/g, ""))} error={error} />
          <div className="sec__row">
            <Button variant="ghost" size="sm" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              disabled={pin.length < 4}
              onClick={async () => {
                try {
                  await api.pinSet(pin);
                  setPin("");
                  setOpen(false);
                  refresh();
                  toast("PIN set.", "ok");
                } catch (e) {
                  setError((e as ApiError).message);
                }
              }}
            >
              Save PIN
            </Button>
          </div>
        </div>
      ) : (
        <div className="sec__row">
          <Button size="sm" onClick={() => setOpen(true)}>
            {o.pin ? "Change PIN" : "Set a PIN"}
          </Button>
          {o.pin && (
            <Button
              size="sm"
              variant="danger"
              onClick={async () => {
                await api.pinClear();
                refresh();
              }}
            >
              Remove
            </Button>
          )}
        </div>
      )}
    </Section>
  );
}

function Device({ o }: { o: SecurityOverview }) {
  const refresh = useRefresh();
  const toast = useUi((s) => s.toast);
  const [busy, setBusy] = useState(false);
  const name = o.deviceUnlockBackend === "windowsHello" ? "Windows Hello" : "System keyring";
  const desc =
    o.deviceUnlockBackend === "windowsHello"
      ? "Unlock with your face, fingerprint or Windows PIN on this PC. The key is bound to this machine's TPM and never leaves it."
      : "Store a device key in your desktop keyring (GNOME Keyring or KWallet), which opens when you log in. Anyone logged in as you on this computer could then open the vault without the password.";
  return (
    <Section icon="fingerprint" title={name} state={o.deviceUnlock ? <On>This device</On> : <Off />}>
      <p className="sec__desc">{desc}</p>
      {!o.deviceUnlockAvailable && !o.deviceUnlock ? (
        <p className="sec__warn">Not available on this system.</p>
      ) : (
        <Button
          size="sm"
          variant={o.deviceUnlock ? "danger" : "plate"}
          busy={busy}
          onClick={async () => {
            setBusy(true);
            try {
              if (o.deviceUnlock) await api.deviceUnlockDisable();
              else await api.deviceUnlockEnable();
              refresh();
              toast(o.deviceUnlock ? `${name} unlock removed.` : `${name} unlock enabled.`, "ok");
            } catch (e) {
              toast((e as ApiError).message, "error");
            } finally {
              setBusy(false);
            }
          }}
        >
          {o.deviceUnlock ? "Remove from this device" : `Enable ${name}`}
        </Button>
      )}
    </Section>
  );
}

const SLOT_NAMES: Record<string, string> = {
  password: "Master password",
  recoveryCode: "Recovery code",
  recoveryQuestions: "Security questions",
  osKeystore: "Device",
};

function Slots({ o }: { o: SecurityOverview }) {
  const refresh = useRefresh();
  const [open, setOpen] = useState(false);
  return (
    <section className="sec sec--slots">
      <button className="detail__disclose" aria-expanded={open} onClick={() => setOpen(!open)}>
        <Icon name={open ? "chevronDown" : "chevronRight"} size={14} /> Key slots <span className="sec__off">({o.slots.length})</span>
      </button>
      {open && (
        <>
          <p className="sec__desc">Every way in wraps the same vault key. Removing a slot only removes that way in; old backups still contain it.</p>
          <table className="slots">
            <tbody>
              {o.slots.map((s) => (
                <tr key={s.id}>
                  <td>{SLOT_NAMES[s.kind]}</td>
                  <td className="sec__off">{s.detail}</td>
                  <td className="sec__off">{relative(s.createdAt)}</td>
                  <td>
                    {s.kind === "osKeystore" && (
                      <button
                        className="slots__rm"
                        onClick={async () => {
                          await api.slotRemove(s.id);
                          refresh();
                        }}
                      >
                        Remove
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </section>
  );
}

export function SecurityPanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { data: o } = useSecurity(open);
  return (
    <Sheet open={open} onClose={onClose} title="Security" width={720}>
      {!o ? (
        <Detent size={24} />
      ) : (
        <div className="secs">
          <MasterPassword o={o} />
          <RecoveryCodes o={o} />
          <Authenticator o={o} />
          <Questions o={o} />
          <Pin o={o} />
          {o.deviceUnlockBackend && <Device o={o} />}
          <Slots o={o} />
        </div>
      )}
    </Sheet>
  );
}

export function SetupFlow({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { data: o } = useSecurity(open);
  const [step, setStep] = useState(1);
  return (
    <Sheet open={open} onClose={() => (step === 2 ? onClose() : setStep(2))} title={step === 1 ? "Before you start: recovery codes" : "Optional ways in"} width={720}
      footer={step === 2 ? <Button variant="primary" onClick={onClose}>Go to my vault</Button> : undefined}
    >
      {!o ? (
        <Detent size={24} />
      ) : step === 1 ? (
        <div className="secs">
          <RecoveryCodes o={o} firstRun onDone={() => setStep(2)} />
          <button className="sec__skip" onClick={() => setStep(2)}>
            Skip. I accept that a forgotten password means losing everything.
          </button>
        </div>
      ) : (
        <div className="secs">
          <Authenticator o={o} />
          <Pin o={o} />
          {o.deviceUnlockBackend && <Device o={o} />}
          <Questions o={o} />
        </div>
      )}
    </Sheet>
  );
}
