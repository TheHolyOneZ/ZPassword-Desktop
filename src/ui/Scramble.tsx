import { useEffect, useState } from "react";

const GLYPHS = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789#$%&*+=?@";

const noise = (length: number) => Array.from({ length }, () => GLYPHS[Math.floor(Math.random() * GLYPHS.length)]).join("");

export function Scramble({ text, step = 18, className = "" }: { text: string; step?: number; className?: string }) {
  const [shown, setShown] = useState(() => (matchMedia("(prefers-reduced-motion: reduce)").matches ? text.length : 0));

  useEffect(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setShown(text.length);
      return;
    }
    setShown(0);
    const timer = setInterval(() => {
      setShown((n) => {
        if (n + 1 >= text.length) clearInterval(timer);
        return Math.min(n + 1, text.length);
      });
    }, step);
    return () => clearInterval(timer);
  }, [text, step]);

  const done = shown >= text.length;
  return (
    <span className="detail__secret">
      <span className="sr-only">{text}</span>
      <span aria-hidden="true" className={`code ${done ? className : ""}`}>
        {text.slice(0, shown)}
        {!done && <span className="detail__scramble">{noise(text.length - shown)}</span>}
      </span>
    </span>
  );
}
