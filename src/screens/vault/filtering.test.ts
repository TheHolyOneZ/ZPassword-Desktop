import { describe, expect, it } from "vitest";
import { visibleItems } from "./filtering";
import type { ItemSummary, Snapshot } from "../../lib/types";

const item = (o: Partial<ItemSummary>): ItemSummary => ({
  id: crypto.randomUUID(),
  serial: 1,
  template: "login",
  title: "x",
  subtitle: "",
  url: null,
  folderId: null,
  labels: [],
  favorite: false,
  hasTotp: false,
  hasPassword: true,
  createdAt: 1,
  updatedAt: 1,
  trashedAt: null,
  ...o,
});

const snap = (items: ItemSummary[]): Snapshot => ({
  name: "t",
  items,
  folders: [
    { id: "a", name: "Work", parent: null, icon: null, createdAt: 0 },
    { id: "b", name: "Databases", parent: "a", icon: null, createdAt: 0 },
  ],
  labels: [{ id: "l1", name: "prod", color: "signal" }],
  templates: [],
  settings: {} as Snapshot["settings"],
});

describe("visibleItems", () => {
  it("includes subfolders and excludes trash", () => {
    const s = snap([item({ title: "pg", folderId: "b" }), item({ title: "gone", folderId: "b", trashedAt: 5 }), item({ title: "home" })]);
    expect(visibleItems(s, { kind: "folder", id: "a" }, "", "title").map((i) => i.title)).toEqual(["pg"]);
    expect(visibleItems(s, { kind: "trash" }, "", "title").map((i) => i.title)).toEqual(["gone"]);
  });

  it("searches labels and ranks title prefix first", () => {
    const s = snap([item({ title: "Staging DB", labels: ["l1"] }), item({ title: "Prod DB" }), item({ title: "Mail", subtitle: "prod@x" })]);
    const r = visibleItems(s, { kind: "all" }, "prod", "title").map((i) => i.title);
    expect(r[0]).toBe("Prod DB");
    expect(r).toHaveLength(3);
  });
});
