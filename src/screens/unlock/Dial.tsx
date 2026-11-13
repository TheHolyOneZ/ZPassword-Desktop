import { forwardRef, useImperativeHandle, useRef } from "react";
import { gsap, reduced, useGSAP } from "../../lib/motion";
import { useSpecular } from "../../lib/light";
import "./dial.css";

export type LampState = "off" | "on" | "error";

export interface DialHandle {
  turnTo: (count: number) => void;
  working: (on: boolean) => void;
  settle: () => gsap.core.Timeline;
  lamp: (state: LampState) => gsap.core.Timeline;
}

function detentFor(count: number): number {
  let angle = 0;
  for (let i = 1; i <= count; i++) {
    const step = 18 + ((i * 37) % 5) * 6;
    const dir = Math.floor((i - 1) / 4) % 2 === 0 ? 1 : -1;
    angle += step * dir;
  }
  return angle;
}

const TICKS = 100;
const C = 200;
const SCREWS = [38, 142, 218, 322];

export const Dial = forwardRef<DialHandle>(function Dial(_props, ref) {
  const root = useRef<HTMLDivElement>(null);
  const spin = useRef<gsap.core.Tween | null>(null);
  const { contextSafe } = useGSAP({ scope: root });
  useSpecular(root);

  useImperativeHandle(ref, () => ({
    turnTo: contextSafe((count: number) => {
      gsap.to(".dial__face", {
        rotation: detentFor(count),
        duration: reduced() ? 0 : 0.28,
        ease: "back.out(2.2)",
        overwrite: "auto",
      });
    }),
    working: contextSafe((on: boolean) => {
      spin.current?.kill();
      spin.current = null;
      root.current?.classList.toggle("is-working", on);
      if (on && !reduced()) {
        spin.current = gsap.to(".dial__face", { rotation: "+=360", duration: 2.2, ease: "none", repeat: -1 });
      }
    }),
    settle: contextSafe(() => {
      spin.current?.kill();
      spin.current = null;
      root.current?.classList.remove("is-working");
      const tl = gsap.timeline();
      if (reduced()) return tl.set(".dial__face", { rotation: 0 });
      const cur = Number(gsap.getProperty(".dial__face", "rotation")) || 0;
      const home = Math.ceil((cur + 150) / 360) * 360;
      return tl
        .to(".dial__face", { rotation: cur - 46, duration: 0.16, ease: "power2.inOut" })
        .to(".dial__face", { rotation: home, duration: 0.46, ease: "power3.out" });
    }),
    lamp: contextSafe((state: LampState) => {
      const tl = gsap.timeline();
      const d = reduced() ? 0 : 1;
      if (state === "error") {
        return tl
          .set(".dial__lamp-lit", { fill: "var(--oxide)" })
          .fromTo(".dial__lamp-lit", { opacity: 1 }, { opacity: 0, duration: 0.9 * d, ease: "power1.in" })
          .set(".dial__lamp-lit", { clearProps: "fill" });
      }
      return tl
        .to(".dial__lamp-lit", { opacity: state === "on" ? 1 : 0, duration: 0.14 * d, ease: "power1.out" })
        .to(".dial__glow", { opacity: state === "on" ? 1 : 0, duration: 0.3 * d, ease: "power1.out" }, "<");
    }),
  }));

  return (
    <div ref={root} className="dial" aria-hidden>
      <svg className="dial__base" viewBox="0 0 400 400">
        <defs>
          <linearGradient id="dial-flange" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" style={{ stopColor: "var(--steel-600)" }} />
            <stop offset="0.5" style={{ stopColor: "var(--steel-800)" }} />
            <stop offset="1" style={{ stopColor: "var(--steel-900)" }} />
          </linearGradient>
          <linearGradient id="dial-chamfer" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" style={{ stopColor: "var(--pit)" }} />
            <stop offset="1" style={{ stopColor: "var(--steel-500)" }} />
          </linearGradient>
        </defs>
        <circle cx={C} cy={C} r="198.5" fill="url(#dial-flange)" className="dial__edge" />
        {SCREWS.map((a, i) => (
          <g key={a} transform={`rotate(${a} ${C} ${C}) translate(${C} 10.5)`}>
            <circle r="4.6" className="dial__screw" />
            <line x1="-3.2" x2="3.2" className="dial__slot" transform={`rotate(${i * 47 + 20})`} />
          </g>
        ))}
        <circle cx={C} cy={C} r="181.5" fill="none" stroke="url(#dial-chamfer)" strokeWidth="3" />
        <circle cx={C} cy={C} r="180" className="dial__well" />
      </svg>

      <svg className="dial__face" viewBox="0 0 400 400">
        <circle cx={C} cy={C} r="173" className="dial__disc" />
        {Array.from({ length: TICKS }, (_, i) => {
          const major = i % 10 === 0;
          const mid = i % 5 === 0;
          return (
            <line
              key={i}
              x1={C}
              y1="31"
              x2={C}
              y2={31 + (major ? 17 : mid ? 11 : 6)}
              className={major ? "dial__tick is-major" : "dial__tick"}
              transform={`rotate(${(i * 360) / TICKS} ${C} ${C})`}
            />
          );
        })}
        {Array.from({ length: 10 }, (_, i) => (
          <text key={i} x={C} y="71" className="dial__num" textAnchor="middle" transform={`rotate(${i * 36} ${C} ${C})`}>
            {i * 10}
          </text>
        ))}
        <circle cx={C} cy={C} r="106" className="dial__step" />
        <circle cx={C} cy={C} r="97" className="dial__knob" />
        {Array.from({ length: 72 }, (_, i) => (
          <line key={i} x1={C} y1="103" x2={C} y2="114" className="dial__knurl" transform={`rotate(${i * 5} ${C} ${C})`} />
        ))}
        <circle cx={C} cy={C} r="85" className="dial__cap" />
        <rect x="197.5" y="121" width="5" height="30" className="dial__pointer" />
      </svg>

      <div className="dial__sheen" />
      <div className="dial__sheen dial__sheen--cap" />
      <div className="dial__glow" />

      <svg className="dial__top" viewBox="0 0 400 400">
        <path className="dial__index" d="M200 31 l-7.5 -15 h15 z" />
        <circle cx={C} cy={C} r="21" className="dial__lamp-bezel" />
        <circle cx={C} cy={C} r="10" className="dial__lamp" />
        <circle cx={C} cy={C} r="10" className="dial__lamp-lit" />
        <ellipse cx="196.5" cy="196" rx="3.4" ry="2.2" className="dial__lens" />
      </svg>
    </div>
  );
});
