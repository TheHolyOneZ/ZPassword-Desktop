import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ReactNode } from "react";
import { Icon } from "../ui/Icon";
import { Lamp } from "../ui/primitives";
import "./faceplate.css";

const win = () => getCurrentWindow();

export function Faceplate({ open, title, center, actions }: { open: boolean; title: string; center?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="faceplate" data-tauri-drag-region>
      <div className="faceplate__id" data-tauri-drag-region>
        <Lamp on={open} label={open ? "Vault open" : "Vault sealed"} />
        <span className="faceplate__mark plate-type" data-tauri-drag-region>
          ZPassword
        </span>
        <span className="faceplate__title" data-tauri-drag-region>
          {title}
        </span>
      </div>
      <div className="faceplate__center" data-tauri-drag-region>
        {center}
      </div>
      <div className="faceplate__actions">
        {actions}
        <div className="faceplate__win">
          <button aria-label="Minimize" onClick={() => win().minimize()}>
            <Icon name="minimize" size={14} />
          </button>
          <button aria-label="Maximize" onClick={() => win().toggleMaximize()}>
            <Icon name="maximize" size={12} />
          </button>
          <button aria-label="Close" className="is-close" onClick={() => win().close()}>
            <Icon name="close" size={14} />
          </button>
        </div>
      </div>
    </header>
  );
}
