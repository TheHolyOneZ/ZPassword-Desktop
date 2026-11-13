import type { Filter } from "../../app/store";
import type { ItemSummary, Snapshot } from "../../lib/types";

export type SortKey = "title" | "updated" | "created" | "serial";

export function subtree(snap: Snapshot, root: string): Set<string> {
  const out = new Set([root]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const f of snap.folders) {
      if (f.parent && out.has(f.parent) && !out.has(f.id)) {
        out.add(f.id);
        grew = true;
      }
    }
  }
  return out;
}

export function matchesFilter(item: ItemSummary, filter: Filter, snap: Snapshot, folderSet?: Set<string>): boolean {
  if (filter.kind === "trash") return item.trashedAt !== null;
  if (item.trashedAt !== null) return false;
  switch (filter.kind) {
    case "all":
      return true;
    case "favorites":
      return item.favorite;
    case "totp":
      return item.hasTotp;
    case "folder":
      return !!item.folderId && (folderSet ?? subtree(snap, filter.id)).has(item.folderId);
    case "label":
      return item.labels.includes(filter.id);
    case "template":
      return item.template === filter.id;
  }
}

export function matchesQuery(item: ItemSummary, terms: string[], snap: Snapshot): boolean {
  if (terms.length === 0) return true;
  const labelNames = item.labels.map((id) => snap.labels.find((l) => l.id === id)?.name ?? "");
  const folder = snap.folders.find((f) => f.id === item.folderId)?.name ?? "";
  const hay = [item.title, item.subtitle, item.url ?? "", String(item.serial), folder, ...labelNames].join("\n").toLowerCase();
  return terms.every((t) => hay.includes(t));
}

export function score(item: ItemSummary, terms: string[]): number {
  if (terms.length === 0) return 0;
  const t = item.title.toLowerCase();
  let s = 0;
  for (const term of terms) {
    if (t.startsWith(term)) s += 3;
    else if (t.includes(term)) s += 2;
    else s += 1;
  }
  return s;
}

export function visibleItems(snap: Snapshot, filter: Filter, query: string, sort: SortKey): ItemSummary[] {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  const folderSet = filter.kind === "folder" ? subtree(snap, filter.id) : undefined;
  const list = snap.items.filter((i) => matchesFilter(i, filter, snap, folderSet) && matchesQuery(i, terms, snap));
  const cmp: Record<SortKey, (a: ItemSummary, b: ItemSummary) => number> = {
    title: (a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: "base" }),
    updated: (a, b) => b.updatedAt - a.updatedAt,
    created: (a, b) => b.createdAt - a.createdAt,
    serial: (a, b) => a.serial - b.serial,
  };
  list.sort((a, b) => score(b, terms) - score(a, terms) || cmp[sort](a, b));
  return list;
}
