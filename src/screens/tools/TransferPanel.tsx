import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { api } from "../../lib/ipc";
import type { ApiError, ExportFormat, ImportSource, ImportSummary } from "../../lib/types";
import { keys } from "../../app/queries";
import { useUi } from "../../app/store";
import { Button, Checkbox, SecretField, Segmented, Sheet, Switch, TextField } from "../../ui/primitives";
import { Icon } from "../../ui/Icon";
import { StrengthMeter } from "../../features/StrengthMeter";
import "./security.css";
import "./transfer.css";

const SOURCE_NAMES: Record<ImportSource, string> = {
  bitwarden: "Bitwarden (JSON)",
  chrome: "Chrome / Edge / Brave (CSV)",
  firefox: "Firefox (CSV)",
  safari: "Safari (CSV)",
  onePassword: "1Password (CSV)",
  lastPass: "LastPass (CSV)",
  bitwardenCsv: "Bitwarden (CSV)",
  genericCsv: "CSV",
  keePass: "KeePass",
  zPassword: "ZPassword backup",
};

const needsPassword = (p: string) => /\.(kdbx|kdb|zpv|zpvbak)$/i.test(p);
const isKeepass = (p: string) => /\.(kdbx|kdb)$/i.test(p);
const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

function Import({ onDone }: { onDone: () => void }) {
  const qc = useQueryClient();
  const toast = useUi((s) => s.toast);
  const [path, setPath] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [keyfile, setKeyfile] = useState<string | null>(null);
  const [summary, setSummary] = useState<ImportSummary | null>(null);
  const [skipDupes, setSkipDupes] = useState(true);
  const [folder, setFolder] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const choose = async () => {
    const p = await openDialog({
      title: "Choose a file to import",
      multiple: false,
      filters: [
        { name: "Password exports", extensions: ["json", "csv", "kdbx", "zpv", "zpvbak"] },
        { name: "All files", extensions: ["*"] },
      ],
    });
    if (typeof p !== "string") return;
    setPath(p);
    setSummary(null);
    setError(null);
    setPassword("");
    setKeyfile(null);
    if (!needsPassword(p)) preview(p, null);
  };

  const preview = async (p: string, pw: string | null) => {
    setBusy(true);
    setError(null);
    try {
      const s = await api.importPreview(p, pw, keyfile);
      setSummary(s);
      setFolder(`Imported from ${SOURCE_NAMES[s.source ?? "genericCsv"].replace(/ \(.*\)/, "")}`);
    } catch (e) {
      setError((e as ApiError).message);
    } finally {
      setBusy(false);
    }
  };

  const commit = async () => {
    setBusy(true);
    try {
      const r = await api.importCommit(skipDupes, folder || null);
      await qc.invalidateQueries({ queryKey: keys.snapshot });
      toast(`Imported ${r.added} ${r.added === 1 ? "item" : "items"}${r.skipped ? `, skipped ${r.skipped} duplicates` : ""}.`, "ok");
      setPath(null);
      setSummary(null);
      onDone();
    } catch (e) {
      setError((e as ApiError).message);
    } finally {
      setBusy(false);
    }
  };

  const reset = () => {
    api.importCancel();
    setPath(null);
    setSummary(null);
    setError(null);
  };

  return (
    <section className="sec">
      <header className="sec__head">
        <Icon name="download" size={18} />
        <h3 className="sec__title">Import</h3>
        <span />
      </header>
      <div className="sec__body">
        {!path ? (
          <>
            <p className="sec__desc">
              Bring in a Bitwarden JSON export, a CSV from Chrome, Edge, Firefox, Safari, 1Password or LastPass, a KeePass database, or a ZPassword
              backup. You'll see what's inside before anything is added.
            </p>
            <Button size="sm" icon="file" onClick={choose}>
              Choose file
            </Button>
          </>
        ) : (
          <div className="xfer">
            <p className="xfer__file">
              <Icon name="file" size={14} /> <span className="code selectable">{fileName(path)}</span>
              <button className="xfer__change" onClick={reset}>
                Change
              </button>
            </p>
            {needsPassword(path) && !summary && (
              <form
                className="sec__form"
                onSubmit={(e) => {
                  e.preventDefault();
                  preview(path, password);
                }}
              >
                <SecretField label={isKeepass(path) ? "KeePass database password" : "Backup password"} value={password} autoFocus onChange={(e) => setPassword(e.target.value)} />
                {isKeepass(path) && (
                  <Button
                    type="button"
                    size="sm"
                    icon="file"
                    onClick={async () => {
                      const p = await openDialog({ title: "KeePass keyfile", multiple: false });
                      if (typeof p === "string") setKeyfile(p);
                    }}
                  >
                    {keyfile ? fileName(keyfile) : "Add keyfile (optional)"}
                  </Button>
                )}
                <Button type="submit" variant="primary" size="sm" busy={busy} disabled={!password && !keyfile}>
                  Open file
                </Button>
              </form>
            )}
            {error && (
              <p className="zp-field__error" role="alert">
                {error}
              </p>
            )}
            {busy && !summary && !needsPassword(path) && <p className="sec__desc">Reading…</p>}
            {summary && (
              <div className="xfer__summary">
                <dl className="xfer__facts">
                  <div>
                    <dt>Format</dt>
                    <dd>{SOURCE_NAMES[summary.source ?? "genericCsv"]}</dd>
                  </div>
                  <div>
                    <dt>Items</dt>
                    <dd className="code">{summary.items}</dd>
                  </div>
                  <div>
                    <dt>Folders</dt>
                    <dd className="code">{summary.folders}</dd>
                  </div>
                  <div>
                    <dt>With one-time codes</dt>
                    <dd className="code">{summary.withTotp}</dd>
                  </div>
                  <div>
                    <dt>Already in this vault</dt>
                    <dd className="code">{summary.duplicates}</dd>
                  </div>
                </dl>
                {summary.sample.length > 0 && (
                  <p className="sec__desc">
                    Includes {summary.sample.slice(0, 5).join(", ")}
                    {summary.items > 5 ? ` and ${summary.items - 5} more` : ""}.
                  </p>
                )}
                {summary.warnings.map((w) => (
                  <p key={w} className="sec__warn">
                    {w}
                  </p>
                ))}
                <TextField label="Put everything in folder" value={folder} onChange={(e) => setFolder(e.target.value)} hint="Leave empty to keep the original folders at the top level." />
                {summary.duplicates > 0 && <Switch label="Skip items that are already in the vault" description="Matches title, username, password and website." checked={skipDupes} onChange={setSkipDupes} />}
                <div className="sec__row">
                  <Button variant="ghost" size="sm" onClick={reset}>
                    Cancel
                  </Button>
                  <Button variant="primary" size="sm" icon="download" busy={busy} onClick={commit}>
                    Import {skipDupes ? summary.items - summary.duplicates : summary.items} items
                  </Button>
                </div>
                {!/\.(kdbx|kdb|zpv|zpvbak)$/i.test(path) && <p className="sec__warn">The file you imported is not encrypted. Delete it once you're done.</p>}
              </div>
            )}
          </div>
        )}
      </div>
    </section>
  );
}

function Export() {
  const toast = useUi((s) => s.toast);
  const [format, setFormat] = useState<ExportFormat>("zpassword");
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [ack, setAck] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const ext = format === "zpassword" ? "zpv" : format === "csv" ? "csv" : "json";
  const ready = format === "zpassword" ? pw.length >= 10 && pw === pw2 : ack;

  const run = async () => {
    const stamp = new Date().toISOString().slice(0, 10);
    const p = await saveDialog({ title: "Export vault", defaultPath: `zpassword-export-${stamp}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    if (!p) return;
    setBusy(true);
    setError(null);
    try {
      await api.exportVault(p, format, format === "zpassword" ? pw : null);
      toast(format === "zpassword" ? "Encrypted backup saved." : "Export saved. It is not encrypted.", "ok");
      setPw("");
      setPw2("");
      setAck(false);
    } catch (e) {
      setError((e as ApiError).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="sec">
      <header className="sec__head">
        <Icon name="upload" size={18} />
        <h3 className="sec__title">Export</h3>
        <span />
      </header>
      <div className="sec__body">
        <Segmented
          label="Format"
          value={format}
          onChange={(f) => {
            setFormat(f);
            setError(null);
          }}
          options={[
            { value: "zpassword", label: "Encrypted backup" },
            { value: "bitwardenJson", label: "Bitwarden JSON" },
            { value: "csv", label: "CSV" },
          ]}
        />
        {format === "zpassword" ? (
          <div className="sec__form">
            <p className="sec__desc">
              A standalone copy with its own password, independent of your master password and unlock methods. Open it in ZPassword on any computer, or
              import it here later.
            </p>
            <SecretField label="Backup password" value={pw} onChange={(e) => setPw(e.target.value)} />
            <StrengthMeter password={pw} />
            <SecretField label="Type it again" value={pw2} onChange={(e) => setPw2(e.target.value)} error={pw2 && pw !== pw2 ? "The two passwords don't match." : null} />
          </div>
        ) : (
          <div className="sec__form">
            <p className="sec__warn">
              {format === "csv"
                ? "CSV keeps the title, website, username, password, notes and one-time code of each item. Other fields are left out."
                : "Bitwarden JSON keeps every field, folder and note."}{" "}
              Either way the file is plain text: anyone who opens it sees every password.
            </p>
            <Checkbox checked={ack} onChange={setAck}>
              I understand this file is not encrypted
            </Checkbox>
          </div>
        )}
        {error && (
          <p className="zp-field__error" role="alert">
            {error}
          </p>
        )}
        <Button size="sm" variant={format === "zpassword" ? "primary" : "danger"} icon="upload" busy={busy} disabled={!ready} onClick={run}>
          {format === "zpassword" ? "Save encrypted backup" : "Save unencrypted export"}
        </Button>
      </div>
    </section>
  );
}

export function TransferPanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Sheet
      open={open}
      onClose={() => {
        api.importCancel().catch(() => {});
        onClose();
      }}
      title="Import and export"
      width={680}
    >
      <div className="secs">
        <Import onDone={onClose} />
        <Export />
      </div>
    </Sheet>
  );
}
