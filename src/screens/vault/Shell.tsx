import { useEffect, useRef } from "react";
import { api } from "../../lib/ipc";
import type { Status } from "../../lib/types";
import { useApplyStatus, useSnapshot } from "../../app/queries";
import { useUi } from "../../app/store";
import { Faceplate } from "../../app/Faceplate";
import { Icon } from "../../ui/Icon";
import { Detent, IconButton, Kbd } from "../../ui/primitives";
import { Rail } from "./Rail";
import { Ledger } from "./Ledger";
import { Detail } from "./Detail";
import { Editor } from "./Editor";
import { GeneratorPanel } from "../tools/GeneratorPanel";
import { HealthPanel } from "../tools/HealthPanel";
import { SecurityPanel, SetupFlow } from "../tools/SecurityPanel";
import { SettingsPanel } from "../tools/SettingsPanel";
import { TransferPanel } from "../tools/TransferPanel";
import { AboutPanel } from "../tools/AboutPanel";
import "./vault.css";

export function Shell({ status }: { status: Status }) {
  const { data: snap } = useSnapshot();
  const apply = useApplyStatus();
  const { query, setQuery, editing, setEditing, panel, openPanel, setupFlow, setSetupFlow } = useUi();
  const search = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && (e.key === "k" || e.key === "f")) {
        e.preventDefault();
        search.current?.focus();
        search.current?.select();
      } else if (mod && e.key === "n") {
        e.preventDefault();
        setEditing({ mode: "new", template: "login" });
      } else if (mod && e.key === "l") {
        e.preventDefault();
        api.lock().then(apply);
      } else if (mod && e.key === "g") {
        e.preventDefault();
        openPanel("generator");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [apply, setEditing, openPanel]);

  return (
    <div className="app-frame">
      <Faceplate
        open
        title={snap?.name ?? status.vaultLabel ?? ""}
        center={
          <label className="search">
            <Icon name="search" size={15} />
            <input
              ref={search}
              className="search__input"
              placeholder="Search titles, usernames, sites, labels"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setQuery("");
                  (e.target as HTMLInputElement).blur();
                }
                if (e.key === "ArrowDown" || e.key === "Enter") {
                  e.preventDefault();
                  document.querySelector<HTMLElement>(".ledger__list")?.focus();
                }
              }}
              spellCheck={false}
              aria-label="Search the vault"
            />
            {!query && <Kbd>Ctrl K</Kbd>}
          </label>
        }
        actions={
          <>
            <IconButton icon="upload" label="Import and export" onClick={() => openPanel("transfer")} />
            <IconButton icon="shield" label="Security" onClick={() => openPanel("security")} />
            <IconButton icon="settings" label="Settings" onClick={() => openPanel("settings")} />
            <button className="lockbtn" onClick={async () => apply(await api.lock())} title="Lock (Ctrl L)">
              <Icon name="lock" size={14} />
              Lock
            </button>
          </>
        }
      />
      <div className="vault-stage">
        {!snap ? (
          <div className="app-loading">
            <Detent size={28} />
          </div>
        ) : (
          <div className="vault">
            <Rail snap={snap} />
            <Ledger snap={snap} />
            {editing ? <Editor snap={snap} /> : <Detail snap={snap} />}
          </div>
        )}
      </div>
      <GeneratorPanel open={panel === "generator"} onClose={() => openPanel(null)} />
      <HealthPanel open={panel === "health"} onClose={() => openPanel(null)} />
      <SecurityPanel open={panel === "security"} onClose={() => openPanel(null)} />
      <SettingsPanel open={panel === "settings"} onClose={() => openPanel(null)} />
      <TransferPanel open={panel === "transfer"} onClose={() => openPanel(null)} />
      <AboutPanel open={panel === "about"} onClose={() => openPanel(null)} />
      <SetupFlow open={setupFlow} onClose={() => setSetupFlow(false)} />
    </div>
  );
}
