import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { api } from "../../lib/ipc";
import type { ApiError, FieldKind, ItemDetail, ItemInput, Snapshot } from "../../lib/types";
import { serial } from "../../lib/format";
import { keys, useItem } from "../../app/queries";
import { useUi } from "../../app/store";
import { useQueryClient } from "@tanstack/react-query";
import { Icon, fieldIcon, templateIcon } from "../../ui/Icon";
import { Button, IconButton, TextField } from "../../ui/primitives";
import { Menu, Select } from "../../ui/Menu";
import { loadGeneratorOptions } from "../tools/generatorOptions";
import { Flip, reduced } from "../../lib/motion";

const KIND_NAMES: Record<FieldKind, string> = {
  text: "Text",
  username: "Username",
  email: "Email",
  password: "Password",
  url: "Website",
  totp: "One-time code (TOTP)",
  number: "Number",
  date: "Date",
  phone: "Phone",
  multiline: "Long text",
  hidden: "Hidden text",
};
const CONCEALED: FieldKind[] = ["password", "totp", "hidden"];

interface DraftField {
  key: string;
  id: string | null;
  name: string;
  kind: FieldKind;
  concealed: boolean;
  value: string | null;
  show: boolean;
}

let seq = 0;
const k = () => `f${++seq}`;

function fromItem(item: ItemDetail): DraftField[] {
  return item.fields.map((f) => ({ key: k(), id: f.id, name: f.name, kind: f.kind, concealed: f.concealed, value: f.concealed ? (f.empty ? "" : null) : (f.value ?? ""), show: false }));
}

export function Editor({ snap }: { snap: Snapshot }) {
  const editing = useUi((s) => s.editing)!;
  const { data: existing } = useItem(editing.mode === "edit" ? editing.id : null);
  if (editing.mode === "edit" && !existing) return <section className="detail" aria-busy />;
  return <EditorForm key={editing.mode === "edit" ? editing.id : `new-${editing.template}`} snap={snap} existing={editing.mode === "edit" ? existing! : null} />;
}

function EditorForm({ snap, existing }: { snap: Snapshot; existing: ItemDetail | null }) {
  const { editing, setEditing, select, toast, filter } = useUi();
  const qc = useQueryClient();
  const initialTemplate = editing?.mode === "new" ? editing.template : existing!.template;
  const [template, setTemplate] = useState(initialTemplate);
  const tpl = snap.templates.find((t) => t.id === template);
  const [title, setTitle] = useState(existing?.title ?? "");
  const [description, setDescription] = useState(existing?.description ?? "");
  const [folderId, setFolderId] = useState<string | null>(existing ? existing.folderId : filter.kind === "folder" ? filter.id : null);
  const [labels, setLabels] = useState<string[]>(existing ? existing.labels : filter.kind === "label" ? [filter.id] : []);
  const [favorite, setFavorite] = useState(existing?.favorite ?? filter.kind === "favorites");
  const [fields, setFields] = useState<DraftField[]>(() =>
    existing
      ? fromItem(existing)
      : (snap.templates.find((t) => t.id === initialTemplate)?.fields ?? []).map((f) => ({
          key: k(),
          id: null,
          name: f.name,
          kind: f.kind,
          concealed: CONCEALED.includes(f.kind),
          value: "",
          show: false,
        })),
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const titleRef = useRef<HTMLInputElement>(null);

  useEffect(() => titleRef.current?.focus(), []);

  const switchTemplate = (id: string) => {
    setTemplate(id);
    const t = snap.templates.find((x) => x.id === id);
    if (!t) return;
    const typed = fields.filter((f) => f.value);
    const fresh = t.fields
      .filter((tf) => !typed.some((f) => f.name === tf.name))
      .map((f) => ({ key: k(), id: null, name: f.name, kind: f.kind, concealed: CONCEALED.includes(f.kind), value: "", show: false }));
    setFields([...typed, ...fresh]);
  };

  const update = (key: string, patch: Partial<DraftField>) => setFields((fs) => fs.map((f) => (f.key === key ? { ...f, ...patch } : f)));
  const remove = (key: string) => setFields((fs) => fs.filter((f) => f.key !== key));
  const fieldList = useRef<HTMLOListElement>(null);
  const swap = useRef<ReturnType<typeof Flip.getState> | null>(null);
  useLayoutEffect(() => {
    const state = swap.current;
    swap.current = null;
    if (state && fieldList.current) Flip.from(state, { targets: fieldList.current.children, duration: 0.24, ease: "power3.inOut", simple: true });
  }, [fields]);
  const moveField = (key: string, d: number) => {
    if (fieldList.current && !reduced()) swap.current = Flip.getState(fieldList.current.children);
    setFields((fs) => {
      const i = fs.findIndex((f) => f.key === key);
      const j = i + d;
      if (j < 0 || j >= fs.length) return fs;
      const next = [...fs];
      [next[i], next[j]] = [next[j], next[i]];
      return next;
    });
  };
  const add = (kind: FieldKind) =>
    setFields((fs) => [...fs, { key: k(), id: null, name: KIND_NAMES[kind].replace(/ \(.*\)/, ""), kind, concealed: CONCEALED.includes(kind), value: "", show: kind !== "password" }]);

  const reveal = async (f: DraftField) => {
    if (f.value !== null) return update(f.key, { show: !f.show });
    try {
      const v = await api.itemReveal(existing!.id, f.id!);
      update(f.key, { value: v, show: true });
    } catch (e) {
      toast((e as ApiError).message, "error");
    }
  };

  const generate = async (f: DraftField) => {
    try {
      const g = await api.generate(loadGeneratorOptions());
      update(f.key, { value: g.value, show: true });
    } catch (e) {
      toast((e as ApiError).message, "error");
    }
  };

  const save = async () => {
    if (!title.trim()) {
      setError("Give the item a title.");
      titleRef.current?.focus();
      return;
    }
    setBusy(true);
    setError(null);
    const input: ItemInput = {
      template,
      title,
      description,
      folderId,
      labels,
      favorite,
      fields: fields.filter((f) => f.name.trim() || f.value).map((f) => ({ id: f.id, name: f.name.trim() || KIND_NAMES[f.kind], kind: f.kind, concealed: f.concealed, value: f.value })),
    };
    try {
      if (existing) {
        await api.itemUpdate(existing.id, input);
        await qc.invalidateQueries({ queryKey: keys.item(existing.id) });
        setEditing(null);
        toast("Saved.", "ok");
      } else {
        const id = await api.itemCreate(input);
        select(id);
        toast("Saved to the vault.", "ok");
      }
      await qc.invalidateQueries({ queryKey: keys.snapshot });
    } catch (e) {
      setError((e as ApiError).message);
    } finally {
      setBusy(false);
    }
  };

  const folderOptions = useMemo(() => {
    const out: { id: string; label: string; depth: number }[] = [];
    const walk = (parent: string | null, depth: number) =>
      snap.folders
        .filter((f) => f.parent === parent)
        .sort((a, b) => a.name.localeCompare(b.name))
        .forEach((f) => {
          out.push({ id: f.id, label: f.name, depth });
          walk(f.id, depth + 1);
        });
    walk(null, 0);
    return out;
  }, [snap.folders]);

  return (
    <section
      className="detail editor"
      aria-label={existing ? `Edit ${existing.title}` : "New item"}
      onKeyDown={(e) => {
        if ((e.ctrlKey || e.metaKey) && e.key === "s") {
          e.preventDefault();
          save();
        }
        if (e.key === "Escape") setEditing(null);
      }}
    >
      <header className="detail__head">
        <div className="detail__stamp">
          <span className="code detail__serial">{existing ? serial(existing.serial) : "New"}</span>
          {existing ? (
            <span className="detail__kind">
              <Icon name={templateIcon(tpl?.icon ?? "file")} size={13} /> {tpl?.name}
            </span>
          ) : (
            <Menu
              label="Kind"
              trigger={(p) => (
                <button className="detail__kind is-button" {...p}>
                  <Icon name={templateIcon(tpl?.icon ?? "file")} size={13} /> {tpl?.name ?? "Item"} <Icon name="chevronDown" size={12} />
                </button>
              )}
              items={snap.templates.map((t) => ({ label: t.name, icon: templateIcon(t.icon), onSelect: () => switchTemplate(t.id) }))}
            />
          )}
        </div>
        <input
          ref={titleRef}
          className="editor__title plate-type"
          placeholder="Title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          aria-label="Title"
        />
      </header>

      <div className="detail__body editor__body">
        <div className="editor__meta">
          <div className="editor__metafield">
            <span>Folder</span>
            <Select
              label="Folder"
              icon="folder"
              value={folderId ?? ""}
              onChange={(v) => setFolderId(v || null)}
              options={[{ value: "", label: "No folder" }, ...folderOptions.map((o) => ({ value: o.id, label: o.label, depth: o.depth }))]}
            />
          </div>
          <button className={`editor__fav ${favorite ? "is-on" : ""}`} aria-pressed={favorite} onClick={() => setFavorite(!favorite)}>
            <Icon name="star" size={14} /> Favorite
          </button>
        </div>
        {snap.labels.length > 0 && (
          <div className="editor__labels" role="group" aria-label="Labels">
            {snap.labels.map((l) => {
              const on = labels.includes(l.id);
              return (
                <button
                  key={l.id}
                  className={`editor__label ${on ? "is-on" : ""}`}
                  aria-pressed={on}
                  style={{ ["--c" as string]: `var(--label-${l.color})` }}
                  onClick={() => setLabels(on ? labels.filter((x) => x !== l.id) : [...labels, l.id])}
                >
                  {l.name}
                </button>
              );
            })}
          </div>
        )}

        <ol className="editor__fields" ref={fieldList}>
          {fields.map((f, i) => (
            <li key={f.key} className="editor__field" data-flip-id={f.key}>
              <div className="editor__fieldhead">
                <Icon name={fieldIcon[f.kind]} size={13} />
                <input className="editor__fname" value={f.name} aria-label="Field name" onChange={(e) => update(f.key, { name: e.target.value })} />
                <Menu
                  label="Field type"
                  align="end"
                  trigger={(p) => (
                    <button className="editor__kind" {...p}>
                      {KIND_NAMES[f.kind]} <Icon name="chevronDown" size={11} />
                    </button>
                  )}
                  items={(Object.keys(KIND_NAMES) as FieldKind[]).map((kind) => ({
                    label: KIND_NAMES[kind],
                    icon: fieldIcon[kind],
                    onSelect: () => update(f.key, { kind, concealed: CONCEALED.includes(kind) }),
                  }))}
                />
                <IconButton icon="arrowUp" label="Move up" size={13} disabled={i === 0} onClick={() => moveField(f.key, -1)} />
                <IconButton icon="arrowDown" label="Move down" size={13} disabled={i === fields.length - 1} onClick={() => moveField(f.key, 1)} />
                <IconButton icon="close" label={`Remove ${f.name || "field"}`} size={13} onClick={() => remove(f.key)} />
              </div>
              {f.kind === "multiline" ? (
                <TextField multiline rows={4} value={f.value ?? ""} onChange={(e) => update(f.key, { value: e.target.value })} aria-label={f.name} />
              ) : (
                <TextField
                  aria-label={f.name}
                  type={f.concealed && !f.show ? "password" : "text"}
                  mono={f.concealed || f.kind === "totp" || f.kind === "url"}
                  value={f.value ?? ""}
                  placeholder={f.value === null ? "Unchanged. Type to replace." : f.kind === "totp" ? "Secret key or otpauth:// link" : undefined}
                  onChange={(e) => update(f.key, { value: e.target.value })}
                  trailing={
                    <>
                      {f.kind === "password" && <IconButton icon="dice" label="Generate password" onClick={() => generate(f)} />}
                      {f.concealed && <IconButton icon={f.show ? "eyeOff" : "eye"} label={f.show ? "Hide" : "Show"} onClick={() => reveal(f)} />}
                    </>
                  }
                />
              )}
            </li>
          ))}
        </ol>

        <Menu
          label="Add field"
          trigger={(p) => (
            <Button variant="ghost" size="sm" icon="plus" {...p}>
              Add field
            </Button>
          )}
          items={(Object.keys(KIND_NAMES) as FieldKind[]).map((kind) => ({ label: KIND_NAMES[kind], icon: fieldIcon[kind], onSelect: () => add(kind) }))}
        />

        <TextField multiline rows={5} label="Notes" value={description} onChange={(e) => setDescription(e.target.value)} />

        {error && (
          <p className="zp-field__error" role="alert">
            {error}
          </p>
        )}
      </div>

      <footer className="editor__foot">
        <span className="editor__hint">
          <kbd className="zp-kbd">Ctrl</kbd> <kbd className="zp-kbd">S</kbd> to save
        </span>
        <Button variant="ghost" onClick={() => setEditing(null)} disabled={busy}>
          Cancel
        </Button>
        <Button variant="primary" icon="check" onClick={save} busy={busy}>
          {existing ? "Save changes" : "Save item"}
        </Button>
      </footer>
    </section>
  );
}
