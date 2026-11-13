import type { GeneratorOptions } from "../../lib/types";

export const DEFAULT_GENERATOR: GeneratorOptions = {
  mode: "characters",
  length: 24,
  uppercase: true,
  lowercase: true,
  digits: true,
  symbols: true,
  avoidAmbiguous: false,
  words: 5,
  separator: "-",
  capitalize: false,
  appendNumber: false,
};

const KEY = "zp.generator";

export function loadGeneratorOptions(): GeneratorOptions {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? { ...DEFAULT_GENERATOR, ...JSON.parse(raw) } : DEFAULT_GENERATOR;
  } catch {
    return DEFAULT_GENERATOR;
  }
}

export function saveGeneratorOptions(o: GeneratorOptions) {
  try {
    localStorage.setItem(KEY, JSON.stringify(o));
  } catch {
  }
}
