import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../lib/ipc";
import type { ApiError, GeneratorOptions } from "../../lib/types";
import { useUi } from "../../app/store";
import { Button, Segmented, Sheet, Slider, Switch, TextField } from "../../ui/primitives";
import { loadGeneratorOptions, saveGeneratorOptions } from "./generatorOptions";
import { gsap, reduced, useGSAP } from "../../lib/motion";
import "./tools.css";

export function GeneratorPanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  const toast = useUi((s) => s.toast);
  const [o, setO] = useState<GeneratorOptions>(loadGeneratorOptions);
  const [value, setValue] = useState("");
  const [bits, setBits] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const regen = useCallback(async (opts: GeneratorOptions) => {
    try {
      const g = await api.generate(opts);
      setValue(g.value);
      setBits(g.entropyBits);
      setError(null);
    } catch (e) {
      setError((e as ApiError).message);
    }
  }, []);

  useEffect(() => {
    if (open) regen(o);
    else setValue("");
  }, [open, o, regen]);

  const set = (patch: Partial<GeneratorOptions>) => {
    const next = { ...o, ...patch };
    setO(next);
    saveGeneratorOptions(next);
  };

  const copy = async () => {
    const r = await api.clipboardCopy(value, true);
    toast(r.clearsIn ? `Copied. Clipboard clears in ${r.clearsIn}s.` : "Copied.", "ok");
  };

  const out = useRef<HTMLOutputElement>(null);
  useGSAP(
    () => {
      const el = out.current;
      if (!el) return;
      gsap.killTweensOf(el);
      if (!value || reduced()) return void (el.textContent = value);
      gsap.to(el, { duration: Math.min(0.5, 0.18 + value.length * 0.008), ease: "none", scrambleText: { text: value, chars: "ABCDEFGHJKMNPQRSTUVWXYZ23456789#$%&", speed: 0.7, revealDelay: 0.06 } });
    },
    { dependencies: [value] },
  );

  const rating = bits >= 100 ? "Excellent" : bits >= 75 ? "Strong" : bits >= 50 ? "Fair" : "Weak";

  return (
    <Sheet open={open} onClose={onClose} title="Generator" width={600}>
      <div className="gen">
        <output className="gen__out code selectable" aria-label="Generated value" ref={out} />
        <div className={`gen__gauge is-${rating.toLowerCase()}`} role="img" aria-label={`${Math.round(bits)} bits of entropy, ${rating.toLowerCase()}`}>
          <i style={{ transform: `scaleX(${Math.min(1, bits / 128)})` }} />
        </div>
        <div className="gen__meter">
          <span className={`gen__rating is-${rating.toLowerCase()}`}>{rating}</span>
          <span>{Math.round(bits)} bits of entropy</span>
          <span className="gen__spacer" />
          <Button size="sm" variant="ghost" icon="dice" onClick={() => regen(o)}>
            Again
          </Button>
          <Button size="sm" variant="primary" icon="copy" onClick={copy} disabled={!value}>
            Copy
          </Button>
        </div>
        {error && <p className="zp-field__error">{error}</p>}

        <Segmented
          label="Type"
          value={o.mode}
          onChange={(mode) => set({ mode, length: mode === "pin" ? 6 : o.mode === "pin" ? 24 : o.length })}
          options={[
            { value: "characters", label: "Characters" },
            { value: "passphrase", label: "Passphrase" },
            { value: "pin", label: "PIN" },
          ]}
        />

        {o.mode !== "passphrase" ? (
          <div className="gen__slider">
            <span>Length</span>
            <Slider label="Length" min={o.mode === "pin" ? 4 : 8} max={o.mode === "pin" ? 16 : 96} value={o.length} onChange={(n) => set({ length: n })} />
            <span className="gen__count">{o.length}</span>
          </div>
        ) : (
          <div className="gen__slider">
            <span>Words</span>
            <Slider label="Words" min={3} max={12} value={o.words} onChange={(n) => set({ words: n })} />
            <span className="gen__count">{o.words}</span>
          </div>
        )}

        {o.mode === "characters" && (
          <div className="gen__opts">
            <Switch label="Uppercase A–Z" checked={o.uppercase} onChange={(v) => set({ uppercase: v })} />
            <Switch label="Lowercase a–z" checked={o.lowercase} onChange={(v) => set({ lowercase: v })} />
            <Switch label="Digits 0–9" checked={o.digits} onChange={(v) => set({ digits: v })} />
            <Switch label="Symbols !@#…" checked={o.symbols} onChange={(v) => set({ symbols: v })} />
            <Switch label="Avoid look-alikes" description="Leaves out I, l, 1, O, 0 and similar." checked={o.avoidAmbiguous} onChange={(v) => set({ avoidAmbiguous: v })} />
          </div>
        )}
        {o.mode === "passphrase" && (
          <div className="gen__opts">
            <TextField label="Separator" value={o.separator} maxLength={3} onChange={(e) => set({ separator: e.target.value })} mono />
            <Switch label="Capitalize words" checked={o.capitalize} onChange={(v) => set({ capitalize: v })} />
            <Switch label="Add a digit" checked={o.appendNumber} onChange={(v) => set({ appendNumber: v })} />
            <p className="zp-field__hint">Words come from the EFF long list (7,776 words), so each adds about 12.9 bits.</p>
          </div>
        )}
      </div>
    </Sheet>
  );
}
