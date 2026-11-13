import { forwardRef, useEffect, useId, useRef, useState, type ButtonHTMLAttributes, type InputHTMLAttributes, type ReactNode } from "react";
import { gsap, reduced, useGSAP } from "../lib/motion";
import { Icon, type IconName } from "./Icon";
import "./ui.css";

type ButtonVariant = "primary" | "plate" | "ghost" | "danger";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  icon?: IconName;
  busy?: boolean;
  size?: "sm" | "md" | "lg";
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "plate", icon, busy, size = "md", className = "", children, disabled, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      className={`zp-btn zp-btn--${variant} zp-btn--${size} ${busy ? "is-busy" : ""} ${className}`}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      {...rest}
    >
      {busy ? <Detent size={14} /> : icon && <Icon name={icon} size={size === "sm" ? 14 : 16} />}
      {children && <span>{children}</span>}
    </button>
  );
});

export function IconButton({
  icon,
  label,
  active,
  className = "",
  size = 16,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { icon: IconName; label: string; active?: boolean; size?: number }) {
  return (
    <button className={`zp-iconbtn ${active ? "is-active" : ""} ${className}`} aria-label={label} title={label} {...rest}>
      <Icon name={icon} size={size} />
    </button>
  );
}

export function Detent({ size = 16 }: { size?: number }) {
  return (
    <svg className="zp-detent" width={size} height={size} viewBox="0 0 20 20" aria-hidden>
      {Array.from({ length: 12 }, (_, i) => (
        <line
          key={i}
          x1="10"
          y1="2"
          x2="10"
          y2={i === 0 ? "6.5" : "4.5"}
          stroke="currentColor"
          strokeWidth="1.6"
          opacity={i === 0 ? 1 : 0.25 + (i / 12) * 0.4}
          transform={`rotate(${i * 30} 10 10)`}
        />
      ))}
    </svg>
  );
}

export interface TextFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  label?: string;
  hint?: ReactNode;
  error?: string | null;
  mono?: boolean;
  trailing?: ReactNode;
  multiline?: boolean;
  rows?: number;
}

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  { label, hint, error, mono, trailing, className = "", id, multiline, rows = 4, ...rest },
  ref,
) {
  const autoId = useId();
  const fieldId = id ?? autoId;
  return (
    <div className={`zp-field ${error ? "has-error" : ""} ${className}`}>
      {label && (
        <label className="zp-field__label" htmlFor={fieldId}>
          {label}
        </label>
      )}
      <div className="zp-field__well">
        {multiline ? (
          <textarea
            id={fieldId}
            className={`zp-field__input ${mono ? "code" : ""}`}
            rows={rows}
            spellCheck={false}
            {...(rest as unknown as React.TextareaHTMLAttributes<HTMLTextAreaElement>)}
          />
        ) : (
          <input ref={ref} id={fieldId} className={`zp-field__input ${mono ? "code" : ""}`} spellCheck={false} autoComplete="off" {...rest} />
        )}
        {trailing && <div className="zp-field__trailing">{trailing}</div>}
      </div>
      {error ? (
        <p className="zp-field__error" role="alert">
          {error}
        </p>
      ) : (
        hint && <p className="zp-field__hint">{hint}</p>
      )}
    </div>
  );
});

export const SecretField = forwardRef<HTMLInputElement, TextFieldProps & { revealLabel?: string }>(function SecretField(
  { trailing, ...props },
  ref,
) {
  const [shown, setShown] = useState(false);
  return (
    <TextField
      ref={ref}
      type={shown ? "text" : "password"}
      mono={shown}
      trailing={
        <>
          {trailing}
          <IconButton
            type="button"
            icon={shown ? "eyeOff" : "eye"}
            label={shown ? "Hide" : "Show"}
            onClick={() => setShown((s) => !s)}
          />
        </>
      }
      {...props}
    />
  );
});

export function Switch({
  checked,
  onChange,
  label,
  description,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  description?: ReactNode;
  disabled?: boolean;
}) {
  const id = useId();
  return (
    <div className={`zp-switch ${disabled ? "is-disabled" : ""}`}>
      <div className="zp-switch__text">
        <label htmlFor={id} className="zp-switch__label">
          {label}
        </label>
        {description && <p className="zp-switch__desc">{description}</p>}
      </div>
      <button
        id={id}
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        className={`zp-switch__toggle ${checked ? "is-on" : ""}`}
        onClick={() => onChange(!checked)}
      >
        <span className="zp-switch__knob" />
      </button>
    </div>
  );
}

export function Checkbox({ checked, onChange, children, disabled }: { checked: boolean; onChange: (v: boolean) => void; children: ReactNode; disabled?: boolean }) {
  return (
    <label className={`zp-check ${disabled ? "is-disabled" : ""}`}>
      <input type="checkbox" className="zp-check__input" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      <span className="zp-check__box" aria-hidden>
        <svg viewBox="0 0 12 12" width="12" height="12">
          <path d="M2.4 6.3 4.9 8.8 9.7 3.5" />
        </svg>
      </span>
      <span className="zp-check__label">{children}</span>
    </label>
  );
}

export function Stepper({
  value,
  onChange,
  min,
  max,
  step = 1,
  unit,
  label,
}: {
  value: number;
  onChange: (n: number) => void;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  label: string;
}) {
  const clamp = (n: number) => Math.max(min, Math.min(max, n));
  return (
    <div className="zp-stepper-wrap">
      <div className="zp-stepper" role="group" aria-label={label}>
        <button type="button" className="zp-stepper__key" aria-label={`Decrease ${label}`} disabled={value <= min} onClick={() => onChange(clamp(value - step))}>
          <Icon name="minimize" size={14} />
        </button>
        <input
          className="zp-stepper__input"
          type="text"
          inputMode="numeric"
          aria-label={label}
          value={value}
          onChange={(e) => onChange(clamp(Number(e.target.value.replace(/\D/g, "")) || 0))}
          onKeyDown={(e) => {
            if (e.key === "ArrowUp" || e.key === "ArrowDown") {
              e.preventDefault();
              onChange(clamp(value + (e.key === "ArrowUp" ? step : -step)));
            }
          }}
        />
        <button type="button" className="zp-stepper__key" aria-label={`Increase ${label}`} disabled={value >= max} onClick={() => onChange(clamp(value + step))}>
          <Icon name="plus" size={14} />
        </button>
      </div>
      {unit && <span className="zp-stepper__unit">{unit}</span>}
    </div>
  );
}

export function Slider({ value, min, max, onChange, label }: { value: number; min: number; max: number; onChange: (n: number) => void; label: string }) {
  const fill = max > min ? ((Math.max(min, Math.min(max, value)) - min) / (max - min)) * 100 : 0;
  return (
    <input
      type="range"
      className="zp-range"
      aria-label={label}
      min={min}
      max={max}
      value={value}
      onChange={(e) => onChange(Number(e.target.value))}
      style={{ ["--fill" as string]: `${fill}%` }}
    />
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div
      className="zp-seg"
      role="radiogroup"
      aria-label={label}
      style={{ ["--n" as string]: options.length, ["--i" as string]: Math.max(0, options.findIndex((o) => o.value === value)) }}
    >
      <span className="zp-seg__thumb" aria-hidden />
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={o.value === value}
          className={`zp-seg__opt ${o.value === value ? "is-on" : ""}`}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Lamp({ on, label }: { on: boolean; label: string }) {
  return <span className={`zp-lamp ${on ? "is-on" : ""}`} role="img" aria-label={label} />;
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="zp-kbd">{children}</kbd>;
}

export function Sheet({
  open,
  onClose,
  title,
  children,
  width = 560,
  footer,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  width?: number;
  footer?: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [mounted, setMounted] = useState(open);

  useEffect(() => {
    if (open) setMounted(true);
  }, [open]);

  useGSAP(
    () => {
      if (!ref.current) return;
      const panel = ref.current.querySelector(".zp-sheet__panel");
      const scrim = ref.current.querySelector(".zp-sheet__scrim");
      const still = reduced();
      if (open) {
        gsap.fromTo(scrim, { opacity: 0 }, { opacity: 1, duration: still ? 0 : 0.2, ease: "none" });
        gsap.fromTo(panel, { yPercent: -101 }, { yPercent: 0, duration: still ? 0 : 0.36, ease: "clunk" });
      } else if (mounted) {
        gsap.to(scrim, { opacity: 0, duration: still ? 0 : 0.16, ease: "none" });
        gsap.to(panel, { yPercent: -101, duration: still ? 0 : 0.2, ease: "power2.in", onComplete: () => setMounted(false) });
      }
    },
    { dependencies: [open, mounted], scope: ref },
  );

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    const first = ref.current?.querySelector<HTMLElement>("input, textarea, button:not(.zp-sheet__close)");
    first?.focus();
    return () => window.removeEventListener("keydown", onKey, true);
  }, [open, onClose]);

  if (!mounted) return null;
  return (
    <div className="zp-sheet" ref={ref}>
      <div className="zp-sheet__scrim" onClick={onClose} />
      <div className="zp-sheet__panel" role="dialog" aria-modal="true" aria-label={title} style={{ width }}>
        <header className="zp-sheet__head">
          <h2 className="zp-sheet__title plate-type">{title}</h2>
          <IconButton className="zp-sheet__close" icon="close" label="Close" onClick={onClose} />
        </header>
        <div className="zp-sheet__body">{children}</div>
        {footer && <footer className="zp-sheet__foot">{footer}</footer>}
        <span className="zp-sheet__pull" aria-hidden />
      </div>
    </div>
  );
}

export function EmptyState({ icon, title, children, action }: { icon: IconName; title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="zp-empty">
      <span className="zp-empty__well">
        <Icon name={icon} size={22} />
      </span>
      <p className="zp-empty__title">{title}</p>
      {children && <p className="zp-empty__body">{children}</p>}
      {action}
    </div>
  );
}
