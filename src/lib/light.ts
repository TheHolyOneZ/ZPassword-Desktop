import { useEffect, type RefObject } from "react";
import { gsap, reduced } from "./motion";

interface Surface {
  el: HTMLElement;
  state: { x: number; y: number };
  toX: (v: number) => void;
  toY: (v: number) => void;
}

const surfaces = new Set<Surface>();
let px = 0;
let py = 0;
let queued = false;

function flush() {
  queued = false;
  for (const s of surfaces) {
    const r = s.el.getBoundingClientRect();
    if (!r.width || !r.height) continue;
    s.toX(gsap.utils.clamp(-0.2, 1.2, (px - r.left) / r.width));
    s.toY(gsap.utils.clamp(-0.2, 1.2, (py - r.top) / r.height));
  }
}

function onMove(e: PointerEvent) {
  px = e.clientX;
  py = e.clientY;
  if (!queued) {
    queued = true;
    requestAnimationFrame(flush);
  }
}

function register(el: HTMLElement) {
  const state = { x: 0.3, y: 0.3 };
  const apply = () => {
    el.style.setProperty("--lx", state.x.toFixed(3));
    el.style.setProperty("--ly", state.y.toFixed(3));
  };
  const s: Surface = {
    el,
    state,
    toX: gsap.quickTo(state, "x", { duration: 0.7, ease: "power3.out", onUpdate: apply }),
    toY: gsap.quickTo(state, "y", { duration: 0.7, ease: "power3.out" }),
  };
  if (surfaces.size === 0) window.addEventListener("pointermove", onMove, { passive: true });
  surfaces.add(s);
  return () => {
    surfaces.delete(s);
    gsap.killTweensOf(state);
    if (surfaces.size === 0) window.removeEventListener("pointermove", onMove);
  };
}

export function useSpecular(ref: RefObject<HTMLElement | null>, key?: unknown) {
  useEffect(() => {
    if (!ref.current || reduced()) return;
    return register(ref.current);
  }, [ref, key]);
}
