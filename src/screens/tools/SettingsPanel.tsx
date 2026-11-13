import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../lib/ipc";
import type { ApiError, VaultSettings } from "../../lib/types";
import { bytes, relative } from "../../lib/format";
import { keys, useApplyStatus, useSnapshot, useStatus, useVaultMutation } from "../../app/queries";
import { useUi } from "../../app/store";
import { Button, Segmented, Sheet, Stepper, Switch, TextField } from "../../ui/primitives";
import { loadTheme, saveTheme, type Theme } from "../../lib/theme";
import "./security.css";
import "./settings.css";

function NumberRow({ label, hint, value, onChange, min, max, unit }: { label: string; hint?: string; value: number; onChange: (n: number) => void; min: number; max: number; unit: string }) {
  return (
    <div className="setting">
      <div>
        <p className="setting__label">{label}</p>
        {hint && <p className="sec__desc">{hint}</p>}
      </div>
      <Stepper label={label} value={value} min={min} max={max} unit={unit} onChange={onChange} />
    </div>
  );
}

export function SettingsPanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { data: snap } = useSnapshot(open);
  const { data: status } = useStatus();
  const apply = useApplyStatus();
  const toast = useUi((s) => s.toast);
  const [s, setS] = useState<VaultSettings | null>(null);
  const [name, setName] = useState("");
  const [theme, setTheme] = useState<Theme>(loadTheme);
  const backups = useQuery({ queryKey: keys.backups, queryFn: api.backupsList, enabled: open, staleTime: 0 });
  const save = useVaultMutation((a: { s: VaultSettings; name: string }) => api.settingsUpdate(a.s, a.name), { success: "Settings saved." });

  useEffect(() => {
    if (open && snap) {
      setS(snap.settings);
      setName(snap.name);
    }
  }, [open, snap]);

  if (!s) return null;
  const set = (patch: Partial<VaultSettings>) => setS({ ...s, ...patch });
  const dirty = JSON.stringify(s) !== JSON.stringify(snap?.settings) || name !== snap?.name;

  return (
    <Sheet
      open={open}
      onClose={onClose}
      title="Settings"
      width={680}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
          <Button variant="primary" disabled={!dirty} busy={save.isPending} onClick={() => save.mutate({ s, name })}>
            Save settings
          </Button>
        </>
      }
    >
      <div className="settings">
        <section className="settings__group">
          <h3 className="settings__h">Vault</h3>
          <TextField label="Name" value={name} onChange={(e) => setName(e.target.value)} />
          <p className="sec__desc">
            File: <span className="code selectable">{status?.vaultPath}</span>
          </p>
        </section>

        <section className="settings__group">
          <h3 className="settings__h">Appearance</h3>
          <div className="setting">
            <div>
              <p className="setting__label">Finish</p>
              <p className="sec__desc">Applies on this computer straight away. It is not stored in the vault.</p>
            </div>
            <Segmented<Theme>
              label="Finish"
              value={theme}
              onChange={(t) => {
                setTheme(t);
                saveTheme(t);
              }}
              options={[
                { value: "steel", label: "Steel" },
                { value: "aluminium", label: "Aluminium" },
                { value: "system", label: "Match system" },
              ]}
            />
          </div>
        </section>

        <section className="settings__group">
          <h3 className="settings__h">Locking</h3>
          <NumberRow label="Lock after inactivity" hint="0 keeps it open until you lock it." value={s.autoLockMinutes} min={0} max={1440} unit="min" onChange={(n) => set({ autoLockMinutes: n })} />
          <Switch label="Lock when the computer locks or sleeps" checked={s.lockOnOsLock} onChange={(v) => set({ lockOnOsLock: v })} />
          <Switch label="Lock when minimized" checked={s.lockOnMinimize} onChange={(v) => set({ lockOnMinimize: v })} />
          <NumberRow
            label="Clear copied secrets after"
            hint="Only if the clipboard still holds what ZPassword put there. 0 never clears."
            value={s.clipboardClearSeconds}
            min={0}
            max={600}
            unit="sec"
            onChange={(n) => set({ clipboardClearSeconds: n })}
          />
        </section>

        <section className="settings__group">
          <h3 className="settings__h">Housekeeping</h3>
          <NumberRow label="Empty trash after" hint="0 keeps trashed items until you delete them." value={s.trashRetentionDays} min={0} max={3650} unit="days" onChange={(n) => set({ trashRetentionDays: n })} />
          <NumberRow label="Flag passwords older than" value={s.passwordMaxAgeDays} min={0} max={3650} unit="days" onChange={(n) => set({ passwordMaxAgeDays: n })} />
          <NumberRow label="Keep backups" hint="Encrypted copies of earlier versions, next to the vault file." value={s.backupsKeep} min={0} max={100} unit="copies" onChange={(n) => set({ backupsKeep: n })} />
        </section>

        <section className="settings__group">
          <h3 className="settings__h">Backups</h3>
          {backups.data?.length ? (
            <ul className="backups">
              {backups.data.map((b) => (
                <li key={b.path}>
                  <span>{relative(b.modified)}</span>
                  <span className="sec__off code">{bytes(b.size)}</span>
                  <Button
                    size="sm"
                    variant="ghost"
                    icon="restore"
                    onClick={async () => {
                      try {
                        await api.backupRestore(b.path);
                        toast("Backup restored. Unlock it with the credentials from that time.", "ok");
                        apply(await api.status());
                        onClose();
                      } catch (e) {
                        toast((e as ApiError).message, "error");
                      }
                    }}
                  >
                    Restore
                  </Button>
                </li>
              ))}
            </ul>
          ) : (
            <p className="sec__desc">No backups yet. One is taken before the first change after each unlock, then at most every 15 minutes.</p>
          )}
        </section>
      </div>
    </Sheet>
  );
}
