import gsap from "gsap";
import { useGSAP } from "@gsap/react";
import { Flip } from "gsap/Flip";
import { CustomEase } from "gsap/CustomEase";
import { ScrambleTextPlugin } from "gsap/ScrambleTextPlugin";

gsap.registerPlugin(useGSAP, Flip, CustomEase, ScrambleTextPlugin);

CustomEase.create("clunk", "M0,0 C0.5,0 0.62,1.06 0.74,1.035 0.84,1.01 0.9,1 1,1");
CustomEase.create("settle", "M0,0 C0.18,0.72 0.3,1.1 0.5,1.06 0.68,1.02 0.8,0.99 1,1");

const query = typeof window !== "undefined" && "matchMedia" in window ? window.matchMedia("(prefers-reduced-motion: reduce)") : null;

export const reduced = () => query?.matches ?? false;

export const dur = (seconds: number) => (reduced() ? 0 : seconds);

export { gsap, Flip, useGSAP };
