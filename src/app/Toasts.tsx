import { useUi } from "./store";
import { Icon } from "../ui/Icon";
import "./toasts.css";

export function Toasts() {
  const { toasts, dismiss } = useUi();
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((t) => (
        <button key={t.id} className={`toast toast--${t.tone}`} onClick={() => dismiss(t.id)}>
          <Icon name={t.tone === "error" ? "alert" : t.tone === "ok" ? "check" : "clock"} size={14} />
          <span>{t.message}</span>
        </button>
      ))}
    </div>
  );
}
