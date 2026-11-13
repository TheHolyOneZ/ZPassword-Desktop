import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { gsap, reduced } from "../../lib/motion";
import { api } from "../../lib/ipc";
import type { Folder, Snapshot } from "../../lib/types";
import { keys, useVaultMutation } from "../../app/queries";
import { useUi, type Filter } from "../../app/store";
import { Icon, templateIcon, type IconName } from "../../ui/Icon";
import { Button, Sheet, TextField } from "../../ui/primitives";
import { Menu } from "../../ui/Menu";
import { LABEL_COLORS } from "./labels";

function same(a: Filter, b: Filter) {
  return a.kind === b.kind && ("id" in a ? a.id : null) === ("id" in b ? b.id : null);
}

function Row({
  filter,
  icon,
  label,
  count,
  depth = 0,
  swatch,
  menu,
}: {
  filter: Filter;
  icon?: IconName;
  label: string;
  count?: number;
  depth?: number;
  swatch?: string;
  menu?: React.ReactNode;
}) {
  const current = useUi((s) => s.filter);
  const setFilter = useUi((s) => s.setFilter);
  const active = same(current, filter);
  return (
    <li className={`rail__row ${active ? "is-active" : ""}`} style={{ paddingLeft: 8 + depth * 14 }} data-depth={depth}>
      <button className="rail__btn" onClick={() => setFilter(filter)} aria-current={active ? "page" : undefined}>
        {swatch ? <span className="rail__swatch" style={{ background: swatch }} /> : icon && <Icon name={icon} size={15} />}
        <span className="rail__label">{label}</span>
        {count !== undefined && count > 0 && <span className="rail__count">{count}</span>}
      </button>
      {menu}
    </li>
  );
}

type Editor = { type: "folder"; id: string | null; parent: string | null; name: string } | { type: "label"; id: string | null; name: string; color: string } | null;

export function Rail({ snap }: { snap: Snapshot }) {
  const openPanel = useUi((s) => s.openPanel);
  const setFilter = useUi((s) => s.setFilter);
  const [editor, setEditor] = useState<Editor>(null);
  const filter = useUi((s) => s.filter);
  const nav = useRef<HTMLElement>(null);
  const marker = useRef<HTMLSpanElement>(null);
  const placed = useRef(false);

  useLayoutEffect(() => {
    const n = nav.current;
    const m = marker.current;
    const row = n?.querySelector<HTMLElement>(".rail__row.is-active");
    if (!n || !m) return;
    if (!row) return void gsap.set(m, { autoAlpha: 0 });
    const y = row.getBoundingClientRect().top - n.getBoundingClientRect().top + n.scrollTop;
    gsap.to(m, { y, height: row.offsetHeight, autoAlpha: 1, duration: placed.current && !reduced() ? 0.24 : 0, ease: "back.out(1.4)", overwrite: true });
    placed.current = true;
  }, [filter, snap.folders, snap.labels, snap.templates]);

  const live = snap.items.filter((i) => i.trashedAt === null);
  const trashCount = snap.items.length - live.length;

  const counts = useMemo(() => {
    const byFolder = new Map<string, number>();
    const byLabel = new Map<string, number>();
    const byTemplate = new Map<string, number>();
    for (const i of live) {
      if (i.folderId) byFolder.set(i.folderId, (byFolder.get(i.folderId) ?? 0) + 1);
      for (const l of i.labels) byLabel.set(l, (byLabel.get(l) ?? 0) + 1);
      byTemplate.set(i.template, (byTemplate.get(i.template) ?? 0) + 1);
    }
    return { byFolder, byLabel, byTemplate };
  }, [live]);

  const saveFolder = useVaultMutation(
    async (e: { id: string | null; parent: string | null; name: string }) => {
      if (e.id) await api.folderUpdate(e.id, e.name, e.parent);
      else await api.folderCreate(e.name, e.parent);
    },
    { invalidate: [keys.snapshot] },
  );
  const deleteFolder = useVaultMutation((id: string) => api.folderDelete(id), { success: "Folder deleted. Its items moved up a level." });
  const saveLabel = useVaultMutation((e: { id: string | null; name: string; color: string }) => api.labelSave(e.id, e.name, e.color), {
    invalidate: [keys.snapshot],
  });
  const deleteLabel = useVaultMutation((id: string) => api.labelDelete(id), { success: "Label deleted." });

  const folderTree = (parent: string | null, depth: number): React.ReactNode[] =>
    snap.folders
      .filter((f) => f.parent === parent)
      .sort((a, b) => a.name.localeCompare(b.name))
      .flatMap((f: Folder) => [
        <Row
          key={f.id}
          filter={{ kind: "folder", id: f.id }}
          icon="folder"
          label={f.name}
          count={counts.byFolder.get(f.id)}
          depth={depth}
          menu={
            <Menu
              label={`${f.name} actions`}
              align="end"
              trigger={(p) => (
                <button className="rail__more" aria-label={`${f.name} actions`} {...p}>
                  <Icon name="more" size={14} />
                </button>
              )}
              items={[
                { label: "New subfolder", icon: "plus", onSelect: () => setEditor({ type: "folder", id: null, parent: f.id, name: "" }) },
                { label: "Rename", icon: "edit", onSelect: () => setEditor({ type: "folder", id: f.id, parent: f.parent, name: f.name }) },
                "divider",
                {
                  label: "Delete folder",
                  icon: "trash",
                  danger: true,
                  onSelect: () => {
                    deleteFolder.mutate(f.id);
                    setFilter({ kind: "all" });
                  },
                },
              ]}
            />
          }
        />,
        ...folderTree(f.id, depth + 1),
      ]);

  const usedTemplates = snap.templates.filter((t) => counts.byTemplate.get(t.id));

  return (
    <nav className="rail" aria-label="Vault index" ref={nav}>
      <span className="rail__marker" ref={marker} aria-hidden />
      <ul className="rail__group">
        <Row filter={{ kind: "all" }} icon="vault" label="All items" count={live.length} />
        <Row filter={{ kind: "favorites" }} icon="star" label="Favorites" count={live.filter((i) => i.favorite).length} />
        <Row filter={{ kind: "totp" }} icon="clock" label="One-time codes" count={live.filter((i) => i.hasTotp).length} />
      </ul>

      {usedTemplates.length > 0 && (
        <section className="rail__section">
          <h2 className="rail__h">Kinds</h2>
          <ul className="rail__group">
            {usedTemplates.map((t) => (
              <Row key={t.id} filter={{ kind: "template", id: t.id }} icon={templateIcon(t.icon)} label={t.name} count={counts.byTemplate.get(t.id)} />
            ))}
          </ul>
        </section>
      )}

      <section className="rail__section">
        <h2 className="rail__h">
          Folders
          <button className="rail__add" aria-label="New folder" onClick={() => setEditor({ type: "folder", id: null, parent: null, name: "" })}>
            <Icon name="plus" size={13} />
          </button>
        </h2>
        <ul className="rail__group">{snap.folders.length ? folderTree(null, 0) : <li className="rail__hint">Group items however you like.</li>}</ul>
      </section>

      <section className="rail__section">
        <h2 className="rail__h">
          Labels
          <button className="rail__add" aria-label="New label" onClick={() => setEditor({ type: "label", id: null, name: "", color: "signal" })}>
            <Icon name="plus" size={13} />
          </button>
        </h2>
        <ul className="rail__group">
          {snap.labels.length ? (
            snap.labels.map((l) => (
              <Row
                key={l.id}
                filter={{ kind: "label", id: l.id }}
                swatch={`var(--label-${l.color})`}
                label={l.name}
                count={counts.byLabel.get(l.id)}
                menu={
                  <Menu
                    label={`${l.name} actions`}
                    align="end"
                    trigger={(p) => (
                      <button className="rail__more" aria-label={`${l.name} actions`} {...p}>
                        <Icon name="more" size={14} />
                      </button>
                    )}
                    items={[
                      { label: "Edit", icon: "edit", onSelect: () => setEditor({ type: "label", id: l.id, name: l.name, color: l.color }) },
                      {
                        label: "Delete label",
                        icon: "trash",
                        danger: true,
                        onSelect: () => {
                          deleteLabel.mutate(l.id);
                          setFilter({ kind: "all" });
                        },
                      },
                    ]}
                  />
                }
              />
            ))
          ) : (
            <li className="rail__hint">Tag items across folders, like "prod" or "shared".</li>
          )}
        </ul>
      </section>

      <div className="rail__tools">
        <ul className="rail__group">
          <li className="rail__row" style={{ paddingLeft: 8 }}>
            <button className="rail__btn" onClick={() => openPanel("generator")}>
              <Icon name="dice" size={15} />
              <span className="rail__label">Generator</span>
            </button>
          </li>
          <li className="rail__row" style={{ paddingLeft: 8 }}>
            <button className="rail__btn" onClick={() => openPanel("health")}>
              <Icon name="gauge" size={15} />
              <span className="rail__label">Password health</span>
            </button>
          </li>
          <Row filter={{ kind: "trash" }} icon="trash" label="Trash" count={trashCount} />
          <li className="rail__row" style={{ paddingLeft: 8 }}>
            <button className="rail__btn" onClick={() => openPanel("about")}>
              <Icon name="vault" size={15} />
              <span className="rail__label">About ZPassword</span>
            </button>
          </li>
        </ul>
      </div>

      <Sheet open={!!editor} onClose={() => setEditor(null)} title={editor?.type === "label" ? (editor.id ? "Edit label" : "New label") : editor?.id ? "Rename folder" : "New folder"} width={420}>
        {editor && (
          <form
            className="rail__form"
            onSubmit={(e) => {
              e.preventDefault();
              if (!editor.name.trim()) return;
              if (editor.type === "folder") saveFolder.mutate(editor);
              else saveLabel.mutate(editor);
              setEditor(null);
            }}
          >
            <TextField label="Name" value={editor.name} autoFocus onChange={(e) => setEditor({ ...editor, name: e.target.value })} />
            {editor.type === "folder" && !editor.id && editor.parent && (
              <p className="zp-field__hint">Inside {snap.folders.find((f) => f.id === editor.parent)?.name}</p>
            )}
            {editor.type === "label" && (
              <div className="rail__colors" role="radiogroup" aria-label="Colour">
                {LABEL_COLORS.map((c) => (
                  <button
                    type="button"
                    key={c.key}
                    role="radio"
                    aria-checked={editor.color === c.key}
                    aria-label={c.name}
                    className={`rail__color ${editor.color === c.key ? "is-on" : ""}`}
                    style={{ background: `var(--label-${c.key})` }}
                    onClick={() => setEditor({ ...editor, color: c.key })}
                  />
                ))}
              </div>
            )}
            <div className="rail__form-actions">
              <Button type="button" variant="ghost" onClick={() => setEditor(null)}>
                Cancel
              </Button>
              <Button type="submit" variant="primary" disabled={!editor.name.trim()}>
                Save
              </Button>
            </div>
          </form>
        )}
      </Sheet>
    </nav>
  );
}
