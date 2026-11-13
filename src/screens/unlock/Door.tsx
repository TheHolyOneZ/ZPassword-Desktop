import { forwardRef, useImperativeHandle, useRef, type ReactNode } from "react";
import { gsap, reduced, useGSAP } from "../../lib/motion";
import { useSpecular } from "../../lib/light";
import { Dial, type DialHandle } from "./Dial";
import "./door.css";

export interface DoorHandle {
  turnTo: (count: number) => void;
  working: (on: boolean) => void;
  open: () => Promise<void>;
  reject: () => void;
}

const BOLT_TRAVEL = -38;
const pairwise = (i: number) => Math.floor(i / 2) * 0.07;

export const Door = forwardRef<DoorHandle, { children: ReactNode; bolts?: "thrown" | "drawn"; className?: string }>(function Door(
  { children, bolts = "thrown", className = "" },
  ref,
) {
  const root = useRef<HTMLElement>(null);
  const slab = useRef<HTMLDivElement>(null);
  const dial = useRef<DialHandle>(null);
  const { contextSafe } = useGSAP({ scope: root });
  useSpecular(slab);

  useImperativeHandle(ref, () => ({
    turnTo: (count) => dial.current?.turnTo(count),
    working: (on) => dial.current?.working(on),
    open: contextSafe(
      () =>
        new Promise<void>((resolve) => {
          const d = dial.current;
          if (!d) return resolve();
          if (reduced()) {
            d.settle();
            d.lamp("on");
            gsap.set(".door__bolt-bar", { x: BOLT_TRAVEL });
            return resolve();
          }
          gsap
            .timeline({ onComplete: resolve })
            .add(d.settle())
            .addLabel("draw", "-=0.1")
            .to(".door__bolt-bar", { x: BOLT_TRAVEL, duration: 0.2, ease: "power3.in", stagger: pairwise }, "draw")
            .to(".door__slab", { keyframes: { x: [0, -1.5, 0.5, -1, 0] }, duration: 0.34, ease: "none" }, "draw+=0.18")
            .add(d.lamp("on"), "draw+=0.3");
        }),
    ),
    reject: contextSafe(() => {
      const d = dial.current;
      if (!d) return;
      d.working(false);
      d.lamp("error");
      if (reduced()) return;
      gsap.fromTo(".door__slab", { x: 0 }, { keyframes: { x: [0, -7, 6, -4, 3, -1, 0] }, duration: 0.42, ease: "none" });
      gsap.fromTo(".door__bolt-bar", { x: 0 }, { keyframes: { x: [0, -5, 0, -2, 0] }, duration: 0.3, ease: "none", stagger: (i: number) => Math.floor(i / 2) * 0.03 });
    }),
  }));

  return (
    <main ref={root} className={`jamb ${className}`}>
      <div className="jamb__frame" />
      <div className="jamb__void" />
      {[0, 1, 2].map((i) => (
        <span key={i} className={`jamb__hinge jamb__hinge--${i}`} aria-hidden>
          <i />
        </span>
      ))}
      {[0, 1, 2].map((i) => (
        <span key={i} className={`jamb__keeper jamb__keeper--${i}`} aria-hidden />
      ))}

      <div className="door" data-bolts={bolts}>
        <div className="door__edge" aria-hidden />
        <div className="door__slab" ref={slab}>
          <section className="door__mount">
            <Dial ref={dial} />
            <p className="door__plate">
              <span className="door__plate-name plate-type">ZPassword</span>
              <span>Offline vault. Sealed with XChaCha20-Poly1305 and Argon2id.</span>
            </p>
          </section>
          <section className="door__panel">{children}</section>
          {[0, 1, 2].flatMap((i) => [
            <span key={`b${i}`} className={`door__bolt door__bolt--${i}`} aria-hidden>
              <span className="door__bolt-bar" />
            </span>,
            <span key={`n${i}`} className={`door__nose door__bolt--${i}`} aria-hidden>
              <span className="door__bolt-bar" />
            </span>,
          ])}
        </div>
      </div>
    </main>
  );
});
