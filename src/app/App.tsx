import { useEffect, useRef, useState } from "react";
import { QueryClient, QueryClientProvider, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { api } from "../lib/ipc";
import type { Status } from "../lib/types";
import { gsap, reduced, useGSAP } from "../lib/motion";
import { keys, useStatus } from "./queries";
import { useUi } from "./store";
import { Faceplate } from "./Faceplate";
import { Toasts } from "./Toasts";
import { Detent } from "../ui/primitives";
import { UnlockScreen } from "../screens/unlock/UnlockScreen";
import { WelcomeScreen } from "../screens/welcome/WelcomeScreen";
import { ResetPasswordScreen, TotpGate } from "../screens/gate/GateScreens";
import { Shell } from "../screens/vault/Shell";

const client = new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } } });

const LOCK_REASONS: Record<string, string> = {
  idle: "Locked after inactivity.",
  screenLock: "Locked because the computer locked.",
  suspend: "Locked because the computer went to sleep.",
  minimize: "Locked on minimize.",
  totpFailures: "Locked after too many wrong codes.",
  restored: "Backup restored. Unlock to continue.",
};

function useActivityPing() {
  useEffect(() => {
    let last = 0;
    const ping = () => {
      const now = Date.now();
      if (now - last > 15_000) {
        last = now;
        api.activity().catch(() => {});
      }
    };
    window.addEventListener("pointerdown", ping);
    window.addEventListener("keydown", ping);
    window.addEventListener("wheel", ping, { passive: true });
    return () => {
      window.removeEventListener("pointerdown", ping);
      window.removeEventListener("keydown", ping);
      window.removeEventListener("wheel", ping);
    };
  }, []);
}

function Root() {
  const qc = useQueryClient();
  const { data: status } = useStatus();
  const toast = useUi((s) => s.toast);
  const reset = useUi((s) => s.reset);
  const stageRef = useRef<HTMLDivElement>(null);
  useActivityPing();

  useEffect(() => {
    const offs = [
      listen<string>("session-locked", (e) => {
        qc.removeQueries({ predicate: (q) => q.queryKey[0] !== "status" });
        qc.invalidateQueries({ queryKey: keys.status });
        reset();
        if (LOCK_REASONS[e.payload]) toast(LOCK_REASONS[e.payload]);
      }),
      listen("clipboard-cleared", () => toast("Clipboard cleared.")),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, [qc, toast, reset]);

  useEffect(() => {
    if (import.meta.env.DEV) return;
    const block = (e: MouseEvent) => {
      const t = e.target as HTMLElement;
      if (!t.closest("input, textarea, .selectable")) e.preventDefault();
    };
    window.addEventListener("contextmenu", block);
    return () => window.removeEventListener("contextmenu", block);
  }, []);

  const stage = status?.stage;

  const [seen, setSeen] = useState<Status | undefined>(status);
  const [sealed, setSealed] = useState<Status | null>(status && status.stage !== "unlocked" ? status : null);
  const [door, setDoor] = useState<"idle" | "opening" | "closing">("idle");
  if (status !== seen) {
    setSeen(status);
    if (status && status.stage !== "unlocked") setSealed(status);
    const was = seen?.stage;
    if (was && stage && was !== stage) {
      if (reduced()) setDoor("idle");
      else if (stage === "unlocked") setDoor("opening");
      else if (was === "unlocked") setDoor("closing");
    }
  }

  useGSAP(
    () => {
      if (door === "idle") return;
      const done = () => setDoor("idle");
      if (door === "closing") {
        gsap
          .timeline({ onComplete: done })
          .fromTo(".door", { rotationY: -64 }, { rotationY: 0, duration: 0.42, ease: "power2.in" })
          .fromTo(".door__bolt-bar", { x: -38 }, { x: 0, duration: 0.14, ease: "power3.in", stagger: (i: number) => Math.floor(i / 2) * 0.05 }, ">-0.02")
          .to(".door__slab", { keyframes: { x: [0, 2, -1, 0] }, duration: 0.22, ease: "none" }, "<");
        return;
      }
      const tl = gsap
        .timeline({
          onComplete: () => {
            done();
            document.querySelector<HTMLElement>(".ledger__list")?.focus({ preventScroll: true });
          },
        })
        .set(".vault-stage", { scale: 0.94, transformOrigin: "50% 50%" })
        .to(".door", { rotationY: -102, duration: 0.95, ease: "power2.inOut" }, 0)
        .to(".jamb__void", { autoAlpha: 0, duration: 0.3, ease: "power1.in" }, 0.08)
        .to(".app-layer--sealed .faceplate", { autoAlpha: 0, duration: 0.2, ease: "none" }, 0)
        .to(".jamb", { scale: 1.16, autoAlpha: 0, duration: 0.5, ease: "power2.in" }, 0.46)
        .to(".vault-stage", { scale: 1, duration: 0.62, ease: "power3.out" }, 0.5);

      const hurry = () => tl.timeScale(6);
      window.addEventListener("keydown", hurry);
      window.addEventListener("pointerdown", hurry);
      return () => {
        window.removeEventListener("keydown", hurry);
        window.removeEventListener("pointerdown", hurry);
      };
    },
    { dependencies: [door], scope: stageRef, revertOnUpdate: true },
  );

  if (!status) {
    return (
      <div className="app-stage">
        <div className="app-frame">
          <Faceplate open={false} title="" />
          <div className="app-loading">
            <Detent size={28} />
          </div>
        </div>
      </div>
    );
  }

  const shown = stage === "unlocked" ? (door === "opening" ? sealed : null) : status;
  return (
    <div ref={stageRef} className="app-stage">
      <div className="app-layer app-layer--vault">{stage === "unlocked" && <Shell status={status} />}</div>
      {shown && (
        <div className="app-layer app-layer--sealed">
          <div className="app-frame">
            <Faceplate open={false} title={shown.vaultLabel ?? ""} />
            {shown.stage === "noVault" && <WelcomeScreen status={shown} />}
            {shown.stage === "locked" && <UnlockScreen key={shown.vaultPath} status={shown} />}
            {shown.stage === "awaitingTotp" && <TotpGate status={shown} />}
            {shown.stage === "needsPasswordReset" && <ResetPasswordScreen status={shown} />}
          </div>
        </div>
      )}
      <Toasts />
    </div>
  );
}

export function App() {
  return (
    <QueryClientProvider client={client}>
      <Root />
    </QueryClientProvider>
  );
}
