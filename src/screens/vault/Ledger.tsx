import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { api } from "../../lib/ipc";
import type { Snapshot } from "../../lib/types";
import { serial, hostOf } from "../../lib/format";
import { useVaultMutation } from "../../app/queries";
import { useUi, type Filter } from "../../app/store";
import { Icon, templateIcon } from "../../ui/Icon";
import { Button, EmptyState } from "../../ui/primitives";
import { Menu } from "../../ui/Menu";
import { visibleItems, type SortKey } from "./filtering";
import { Flip, gsap, reduced } from "../../lib/motion";

function filterTitle(f: Filter, snap: Snapshot): string {
  switch (f.kind) {
    case "all":
      return "All items";
    case "favorites":
      return "Favorites";
    case "totp":
      return "One-time codes";
    case "trash":
      return "Trash";
    case "folder":
      return snap.folders.find((x) => x.id === f.id)?.name ?? "Folder";
    case "label":
      return snap.labels.find((x) => x.id === f.id)?.name ?? "Label";
    case "template":
      return snap.templates.find((x) => x.id === f.id)?.name ?? "Kind";
  }
}

const SORTS: { key: SortKey; label: string }[] = [
  { key: "title", label: "Title" },
  { key: "updated", label: "Recently changed" },
  { key: "created", label: "Recently added" },
  { key: "serial", label: "Number" },
];

export function Ledger({ snap }: { snap: Snapshot }) {
  const { filter, query, selectedId, select, setEditing } = useUi();
  const [sort, setSort] = useState<SortKey>(() => {
    try {
      return (localStorage.getItem("zp.sort") as SortKey) || "title";
    } catch {
      return "title";
    }
  });
  const list = useMemo(() => visibleItems(snap, filter, query, sort), [snap, filter, query, sort]);
  const listRef = useRef<HTMLUListElement>(null);
  const pointer = useRef<HTMLLIElement>(null);

  const order = `${filter.kind}:${"id" in filter ? filter.id : ""}|${sort}|${query}`;
  const flip = useRef<{ order: string; state: ReturnType<typeof Flip.getState> | null }>({ order, state: null });
  if (flip.current.order !== order && !flip.current.state && listRef.current && list.length <= 120 && !reduced()) {
    flip.current.state = Flip.getState(listRef.current.querySelectorAll(".ledger__row"));
  }
  useLayoutEffect(() => {
    const f = flip.current;
    if (f.order === order) return;
    f.order = order;
    const state = f.state;
    f.state = null;
    if (!state || !listRef.current) return;
    Flip.from(state, {
      targets: listRef.current.querySelectorAll(".ledger__row"),
      duration: 0.3,
      ease: "power3.inOut",
      simple: true,
      onEnter: (els) => gsap.fromTo(els, { opacity: 0 }, { opacity: 1, duration: 0.2, ease: "none" }),
    });
  }, [order]);

  const lastSel = useRef<string | null>(null);
  useLayoutEffect(() => {
    const p = pointer.current;
    const row = selectedId ? listRef.current?.querySelector<HTMLElement>(`[data-id="${selectedId}"]`) : null;
    if (!p) return;
    if (!row) return void gsap.set(p, { autoAlpha: 0 });
    const slide = lastSel.current !== null && lastSel.current !== selectedId && !reduced();
    lastSel.current = selectedId;
    gsap.to(p, { y: row.offsetTop, height: row.offsetHeight, autoAlpha: 1, duration: slide ? 0.2 : 0, ease: "back.out(1.5)", overwrite: true });
  }, [selectedId, list]);
  const emptyTrash = useVaultMutation(() => api.trashEmpty(), { success: "Trash emptied." });

  useEffect(() => {
    try {
      localStorage.setItem("zp.sort", sort);
    } catch {
    }
  }, [sort]);

  useEffect(() => {
    if (useUi.getState().editing) return;
    if (!list.some((i) => i.id === selectedId)) select(list[0]?.id ?? null);
  }, [list, selectedId, select]);

  const move = (delta: number) => {
    const idx = list.findIndex((i) => i.id === selectedId);
    const next = list[Math.min(list.length - 1, Math.max(0, idx + delta))];
    if (next) {
      select(next.id);
      listRef.current?.querySelector<HTMLElement>(`[data-id="${next.id}"]`)?.scrollIntoView({ block: "nearest" });
    }
  };

  const newItems = snap.templates.map((t) => ({ label: t.name, icon: templateIcon(t.icon), onSelect: () => setEditing({ mode: "new", template: t.id }) }));
  const title = query ? `Results in ${filterTitle(filter, snap).toLowerCase()}` : filterTitle(filter, snap);

  return (
    <section className="ledger" aria-label="Items">
      <header className="ledger__head">
        <div>
          <h1 className="ledger__title plate-type">{title}</h1>
          <p className="ledger__count">
            {list.length} {list.length === 1 ? "item" : "items"}
          </p>
        </div>
        <div className="ledger__tools">
          <Menu
            label="Sort"
            align="end"
            trigger={(p) => (
              <button className="zp-iconbtn" aria-label={`Sort by ${SORTS.find((s) => s.key === sort)?.label}`} title="Sort" {...p}>
                <Icon name={sort === "title" ? "text" : sort === "serial" ? "hash" : "clock"} />
              </button>
            )}
            items={SORTS.map((s) => ({ label: s.label, selected: s.key === sort, onSelect: () => setSort(s.key) }))}
          />
          {filter.kind === "trash" ? (
            <Button variant="danger" size="sm" disabled={!list.length} onClick={() => emptyTrash.mutate(undefined)}>
              Empty trash
            </Button>
          ) : (
            <Menu
              label="New item"
              align="end"
              trigger={(p) => (
                <Button variant="primary" size="sm" icon="plus" {...p}>
                  New
                </Button>
              )}
              items={newItems}
            />
          )}
        </div>
      </header>

      {list.length === 0 ? (
        query ? (
          <EmptyState icon="search" title="Nothing matches">
            Try fewer words, or search by username, website or label.
          </EmptyState>
        ) : filter.kind === "trash" ? (
          <EmptyState icon="trash" title="Trash is empty">
            Deleted items wait here for {snap.settings.trashRetentionDays || "∞"} days before they're removed for good.
          </EmptyState>
        ) : (
          <EmptyState
            icon="key"
            title={filter.kind === "all" ? "Store your first login" : "Nothing here yet"}
            action={
              <Button variant="primary" icon="plus" onClick={() => setEditing({ mode: "new", template: filter.kind === "template" ? filter.id : "login" })}>
                New {filter.kind === "template" ? (snap.templates.find((t) => t.id === filter.id)?.name ?? "item").toLowerCase() : "login"}
              </Button>
            }
          >
            {filter.kind === "all" ? "Logins, database credentials, API keys, cards and notes all live in this one encrypted file." : undefined}
          </EmptyState>
        )
      ) : (
        <ul
          className="ledger__list"
          ref={listRef}
          role="listbox"
          aria-label={title}
          tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") {
              e.preventDefault();
              move(1);
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              move(-1);
            }
          }}
        >
          <li ref={pointer} className="ledger__pointer" role="presentation" aria-hidden />
          {list.map((i) => {
            const host = hostOf(i.url);
            return (
              <li
                key={i.id}
                data-id={i.id}
                data-flip-id={i.id}
                role="option"
                aria-selected={i.id === selectedId}
                className={`ledger__row ${i.id === selectedId ? "is-selected" : ""}`}
                onClick={() => select(i.id)}
              >
                <span className="ledger__serial code">{serial(i.serial)}</span>
                <span className="ledger__glyph">
                  <Icon name={templateIcon(snap.templates.find((t) => t.id === i.template)?.icon ?? "file")} size={15} />
                </span>
                <span className="ledger__main">
                  <span className="ledger__name">{i.title}</span>
                  <span className="ledger__sub">
                    {i.subtitle || (host ? "" : "\u00a0")}
                    {host && <span className={`ledger__host ${i.subtitle ? "has-gap" : ""}`}>{host}</span>}
                  </span>
                </span>
                <span className="ledger__marks">
                  {i.labels.slice(0, 3).map((l) => {
                    const label = snap.labels.find((x) => x.id === l);
                    return label ? <span key={l} className="ledger__dot" title={label.name} style={{ background: `var(--label-${label.color})` }} /> : null;
                  })}
                  {i.hasTotp && <Icon name="clock" size={12} className="ledger__mark" title="Has one-time code" />}
                  {i.favorite && <Icon name="star" size={12} className="ledger__mark is-fav" title="Favorite" />}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
