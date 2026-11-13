import { create } from "zustand";

export type Filter =
  | { kind: "all" }
  | { kind: "favorites" }
  | { kind: "totp" }
  | { kind: "trash" }
  | { kind: "folder"; id: string }
  | { kind: "label"; id: string }
  | { kind: "template"; id: string };

export type Panel = "settings" | "security" | "generator" | "health" | "transfer" | "about" | null;

export type Editing = { mode: "new"; template: string } | { mode: "edit"; id: string } | null;

export interface Toast {
  id: number;
  message: string;
  tone: "info" | "ok" | "error";
}

interface UiState {
  filter: Filter;
  query: string;
  selectedId: string | null;
  editing: Editing;
  panel: Panel;
  toasts: Toast[];
  setupFlow: boolean;
  setSetupFlow: (v: boolean) => void;
  setFilter: (f: Filter) => void;
  setQuery: (q: string) => void;
  select: (id: string | null) => void;
  setEditing: (e: Editing) => void;
  openPanel: (p: Panel) => void;
  toast: (message: string, tone?: Toast["tone"]) => void;
  dismiss: (id: number) => void;
  reset: () => void;
}

let toastSeq = 0;

export const useUi = create<UiState>((set) => ({
  filter: { kind: "all" },
  query: "",
  selectedId: null,
  editing: null,
  panel: null,
  toasts: [],
  setupFlow: false,
  setSetupFlow: (setupFlow) => set({ setupFlow }),
  setFilter: (filter) => set({ filter, editing: null }),
  setQuery: (query) => set({ query }),
  select: (selectedId) => set({ selectedId, editing: null }),
  setEditing: (editing) => set({ editing }),
  openPanel: (panel) => set({ panel }),
  toast: (message, tone = "info") => {
    const id = ++toastSeq;
    set((s) => ({ toasts: [...s.toasts.slice(-3), { id, message, tone }] }));
    setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), tone === "error" ? 6000 : 3200);
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  reset: () => set({ filter: { kind: "all" }, query: "", selectedId: null, editing: null, panel: null }),
}));
