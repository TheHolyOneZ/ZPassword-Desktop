import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Icon, type IconName } from "./Icon";

export interface MenuItem {
  label: string;
  icon?: IconName;
  onSelect: () => void;
  danger?: boolean;
  disabled?: boolean;
  selected?: boolean;
  hint?: string;
  depth?: number;
}

type TriggerProps = { onClick: () => void; "aria-expanded": boolean; "aria-haspopup": "menu" | "listbox" };

const GAP = 5;
const MARGIN = 8;

export function Menu({
  trigger,
  items,
  align = "start",
  label,
  variant = "menu",
  matchWidth = false,
  className = "",
}: {
  trigger: (props: TriggerProps) => ReactNode;
  items: (MenuItem | "divider")[];
  align?: "start" | "end";
  label: string;
  variant?: "menu" | "listbox";
  matchWidth?: boolean;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);

  const close = (refocus = true) => {
    setOpen(false);
    if (refocus) root.current?.querySelector<HTMLElement>("button")?.focus({ preventScroll: true });
  };

  useLayoutEffect(() => {
    const el = list.current;
    const anchor = root.current;
    if (!open || !el || !anchor) return;
    const a = anchor.getBoundingClientRect();
    if (matchWidth) el.style.width = `${a.width}px`;
    el.style.maxHeight = "";
    const below = window.innerHeight - a.bottom - MARGIN - GAP;
    const above = a.top - MARGIN - GAP;
    const up = el.offsetHeight > below && above > below;
    const maxH = Math.max(96, Math.min(380, up ? above : below));
    el.style.maxHeight = `${maxH}px`;
    const h = Math.min(el.offsetHeight, maxH);
    const w = el.offsetWidth;
    const left = align === "end" ? a.right - w : a.left;
    el.style.left = `${Math.round(Math.max(MARGIN, Math.min(window.innerWidth - w - MARGIN, left)))}px`;
    el.style.top = `${Math.round(up ? a.top - GAP - h : a.bottom + GAP)}px`;
    el.dataset.side = up ? "top" : "bottom";
    el.dataset.ready = "true";
    const first = el.querySelector<HTMLElement>('.zp-menu__item[data-selected="true"]:not(:disabled)') ?? el.querySelector<HTMLElement>(".zp-menu__item:not(:disabled)");
    first?.focus({ preventScroll: true });
    first?.scrollIntoView({ block: "nearest" });
  }, [open, align, matchWidth]);

  useEffect(() => {
    if (!open) return;
    const inside = (t: EventTarget | null) => t instanceof Node && (root.current?.contains(t) || list.current?.contains(t));
    const onDown = (e: MouseEvent) => {
      if (!inside(e.target)) setOpen(false);
    };
    const onScroll = (e: Event) => {
      if (!list.current?.contains(e.target as Node)) setOpen(false);
    };
    const dismiss = () => setOpen(false);
    const onKey = (e: KeyboardEvent) => {
      const btns = [...(list.current?.querySelectorAll<HTMLButtonElement>(".zp-menu__item:not(:disabled)") ?? [])];
      const i = btns.indexOf(document.activeElement as HTMLButtonElement);
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        close();
      } else if (e.key === "Tab") {
        close();
      } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        btns[e.key === "ArrowDown" ? (i + 1) % btns.length : (i - 1 + btns.length) % btns.length]?.focus();
      } else if (e.key === "Home" || e.key === "End") {
        e.preventDefault();
        btns[e.key === "Home" ? 0 : btns.length - 1]?.focus();
      } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
        const ch = e.key.toLowerCase();
        const next = [...btns.slice(i + 1), ...btns.slice(0, i + 1)].find((b) => b.textContent?.trim().toLowerCase().startsWith(ch));
        if (next) {
          e.preventDefault();
          next.focus();
        }
      }
    };
    document.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", dismiss);
    window.addEventListener("blur", dismiss);
    return () => {
      document.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", dismiss);
      window.removeEventListener("blur", dismiss);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const listbox = variant === "listbox";
  const hasIcons = items.some((it) => it !== "divider" && it.icon);
  const pickOne = items.some((it) => it !== "divider" && it.selected !== undefined);

  return (
    <div className={`zp-menu ${open ? "is-open" : ""} ${className}`} ref={root}>
      {trigger({ onClick: () => setOpen((o) => !o), "aria-expanded": open, "aria-haspopup": variant })}
      {open &&
        createPortal(
          <div ref={list} className={`zp-menu__list ${hasIcons ? "has-icons" : ""}`} role={variant} aria-label={label}>
            {items.map((it, i) =>
              it === "divider" ? (
                <hr key={i} className="zp-menu__div" />
              ) : (
                <button
                  key={i}
                  type="button"
                  role={listbox ? "option" : pickOne ? "menuitemradio" : "menuitem"}
                  aria-selected={listbox ? !!it.selected : undefined}
                  aria-checked={!listbox && pickOne ? !!it.selected : undefined}
                  data-selected={it.selected ? "true" : undefined}
                  className={`zp-menu__item ${it.danger ? "is-danger" : ""}`}
                  style={it.depth ? { paddingLeft: 10 + it.depth * 14 } : undefined}
                  disabled={it.disabled}
                  onClick={() => {
                    close();
                    it.onSelect();
                  }}
                >
                  {hasIcons && <span className="zp-menu__icon">{it.icon && <Icon name={it.icon} size={15} />}</span>}
                  <span className="zp-menu__label">{it.label}</span>
                  {it.hint && <span className="zp-menu__hint">{it.hint}</span>}
                  {pickOne && <Icon name="check" size={14} className="zp-menu__check" />}
                </button>
              ),
            )}
          </div>,
          document.body,
        )}
    </div>
  );
}

export interface SelectOption<T extends string> {
  value: T;
  label: string;
  icon?: IconName;
  depth?: number;
}

export function Select<T extends string>({
  value,
  options,
  onChange,
  label,
  icon,
  className = "",
}: {
  value: T;
  options: SelectOption<T>[];
  onChange: (v: T) => void;
  label: string;
  icon?: IconName;
  className?: string;
}) {
  const current = options.find((o) => o.value === value);
  return (
    <Menu
      label={label}
      variant="listbox"
      matchWidth
      className={`zp-select ${className}`}
      trigger={(p) => (
        <button type="button" className="zp-select__btn" aria-label={`${label}: ${current?.label ?? ""}`} {...p}>
          {(current?.icon ?? icon) && <Icon name={(current?.icon ?? icon)!} size={15} />}
          <span className="zp-select__value">{current?.label}</span>
          <Icon name="chevronDown" size={14} className="zp-select__chev" />
        </button>
      )}
      items={options.map((o) => ({ label: o.label, icon: o.icon, depth: o.depth, selected: o.value === value, onSelect: () => onChange(o.value) }))}
    />
  );
}
