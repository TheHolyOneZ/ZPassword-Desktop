import { useEffect, useState } from "react";
import { api } from "../lib/ipc";
import type { Strength } from "../lib/types";
import "./strength.css";

const words = ["Very weak", "Weak", "Fair", "Strong", "Very strong"];

export function StrengthMeter({ password, onStrength }: { password: string; onStrength?: (s: Strength | null) => void }) {
  const [s, setS] = useState<Strength | null>(null);
  useEffect(() => {
    if (!password) {
      setS(null);
      onStrength?.(null);
      return;
    }
    const t = setTimeout(async () => {
      const r = await api.strength(password).catch(() => null);
      setS(r);
      onStrength?.(r);
    }, 120);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [password]);

  const score = s?.score ?? -1;
  return (
    <div className="strength" data-score={score} aria-live="polite">
      <div className="tumbler" role="img" aria-label={s ? `Strength ${s.score + 1} of 5` : "Strength not measured yet"}>
        {[0, 1, 2, 3, 4].map((i) => (
          <span key={i} className={`tumbler__pin ${i <= score ? "is-set" : ""}`} style={{ ["--i" as string]: i }}>
            <span className="tumbler__stack" />
          </span>
        ))}
      </div>
      <p className="strength__text">
        {s ? (
          <>
            <strong>{words[s.score]}.</strong> <span>Roughly {s.crackTime} to crack offline.</span>
            {s.warning && <span className="strength__warn">{s.warning}</span>}
          </>
        ) : (
          <span>Use a long passphrase you don't use anywhere else.</span>
        )}
      </p>
    </div>
  );
}
