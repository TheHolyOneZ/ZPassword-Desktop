import { Fragment, useEffect, useRef, useState } from "react";
import { api } from "../../lib/ipc";
import type { ApiError, FieldView, ItemDetail, Snapshot } from "../../lib/types";
import { date, relative, serial } from "../../lib/format";
import { useItem, useVaultMutation } from "../../app/queries";
import { useUi } from "../../app/store";
import { Icon, fieldIcon, templateIcon } from "../../ui/Icon";
import { Button, EmptyState, IconButton } from "../../ui/primitives";
import { Menu } from "../../ui/Menu";
import { Scramble } from "../../ui/Scramble";
import { gsap, reduced, useGSAP } from "../../lib/motion";

function Flap({ char, delay }: { char: string; delay: number }) {
  const root = useRef<HTMLSpanElement>(null);
  const [shown, setShown] = useState(char);

  useGSAP(
    () => {
      if (char === shown) return;
      if (reduced()) return setShown(char);
      gsap
        .timeline({ delay, onComplete: () => setShown(char) })
        .set(".flap__leaf--bot", { autoAlpha: 0 })
        .fromTo(".flap__leaf--top", { rotationX: 0, autoAlpha: 1 }, { rotationX: -90, duration: 0.13, ease: "power2.in" })
        .set(".flap__leaf--top", { autoAlpha: 0 })
        .fromTo(".flap__leaf--bot", { rotationX: 90, autoAlpha: 1 }, { rotationX: 0, duration: 0.2, ease: "bounce.out" });
    },
    { dependencies: [char], scope: root },
  );

  return (
    <span className="flap" ref={root} aria-hidden>
      <span className="flap__half flap__half--top">
        <span>{char}</span>
      </span>
      <span className="flap__half flap__half--bot">
        <span>{shown}</span>
      </span>
      <span className="flap__half flap__half--top flap__leaf flap__leaf--top">
        <span>{shown}</span>
      </span>
      <span className="flap__half flap__half--bot flap__leaf flap__leaf--bot">
        <span>{char}</span>
      </span>
    </span>
  );
}

function TotpReadout({ itemId, fieldId }: { itemId: string; fieldId: string }) {
  const [code, setCode] = useState<{ code: string; period: number; remaining: number } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const fetchedAt = useRef(0);

  useEffect(() => {
    let alive = true;
    api
      .itemTotp(itemId, fieldId)
      .then((c) => {
        if (!alive) return;
        fetchedAt.current = Date.now();
        setCode(c);
        setErr(null);
      })
      .catch((e) => alive && setErr((e as ApiError).message));
    let t: number | undefined;
    const stop = () => {
      window.clearInterval(t);
      t = undefined;
    };
    const start = () => {
      stop();
      setNow(Date.now());
      t = window.setInterval(() => setNow(Date.now()), 1000);
    };
    const onVisibility = () => (document.hidden ? stop() : start());
    start();
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      alive = false;
      stop();
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [itemId, fieldId]);

  const remaining = code ? code.remaining - (now - fetchedAt.current) / 1000 : 0;
  useEffect(() => {
    if (code && remaining <= 0) {
      api.itemTotp(itemId, fieldId).then((c) => {
        fetchedAt.current = Date.now();
        setCode(c);
      });
    }
  }, [remaining, code, itemId, fieldId]);

  if (err) return <span className="detail__err">{err}</span>;
  if (!code) return <span className="detail__muted">…</span>;
  const left = Math.max(0, Math.min(code.period, Math.ceil(remaining)));
  const half = Math.ceil(code.code.length / 2);
  return (
    <span className="totp">
      <span className="totp__code code" role="text" aria-label={`${code.code.slice(0, half)} ${code.code.slice(half)}`}>
        {[...code.code].map((ch, i) => (
          <Fragment key={i}>
            {i === half && <span className="totp__gap" />}
            <Flap char={ch} delay={i * 0.035} />
          </Fragment>
        ))}
      </span>
      <span
        className={`totp__fuse ${left <= 5 ? "is-low" : ""}`}
        role="timer"
        aria-label={`${left} seconds left`}
        style={{ ["--left" as string]: left, ["--period" as string]: code.period }}
      >
        <i />
      </span>
    </span>
  );
}

function FieldRow({ item, f }: { item: ItemDetail; f: FieldView }) {
  const [revealed, setRevealed] = useState<string | null>(null);
  const [stamp, setStamp] = useState<{ n: number; clearsIn: number | null } | null>(null);
  const stampEl = useRef<HTMLSpanElement>(null);
  const toast = useUi((s) => s.toast);

  useEffect(() => setRevealed(null), [item.id, item.updatedAt]);

  const toggle = async () => {
    if (revealed !== null) return setRevealed(null);
    try {
      setRevealed(await api.itemReveal(item.id, f.id));
    } catch (e) {
      toast((e as ApiError).message, "error");
    }
  };

  const copy = async () => {
    try {
      const r = await api.itemCopy(item.id, f.id);
      setStamp((s) => ({ n: (s?.n ?? 0) + 1, clearsIn: r.clearsIn }));
    } catch (e) {
      toast((e as ApiError).message, "error");
    }
  };

  useGSAP(
    () => {
      const el = stampEl.current;
      if (!stamp || !el) return;
      const tl = gsap.timeline({ onComplete: () => setStamp(null) });
      if (reduced()) return void tl.to(el, { duration: 1.8 });
      tl.fromTo(el, { scale: 1.6, rotation: -10, autoAlpha: 0 }, { scale: 1, rotation: -3, autoAlpha: 1, duration: 0.1, ease: "power4.in" })
        .fromTo(el.closest(".detail__field"), { y: 0 }, { keyframes: { y: [0, 1.5, 0] }, duration: 0.12, ease: "none" })
        .to(el, { autoAlpha: 0, duration: 0.28, ease: "power1.in" }, "+=1.35");
    },
    { dependencies: [stamp?.n] },
  );

  if (f.empty) return null;
  const isTotp = f.kind === "totp";
  const multiline = f.kind === "multiline" || (revealed?.includes("\n") ?? false);

  let value: React.ReactNode;
  if (isTotp) value = <TotpReadout itemId={item.id} fieldId={f.id} />;
  else if (f.concealed && revealed === null) value = <span className="detail__mask" role="img" aria-label="Hidden" />;
  else if (f.concealed)
    value = multiline ? (
      <pre className="detail__pre code selectable">{revealed}</pre>
    ) : (
      <Scramble text={revealed!} className="selectable" />
    );
  else if (f.kind === "multiline") value = <pre className="detail__pre selectable">{f.value}</pre>;
  else value = <span className={`selectable ${["url", "number"].includes(f.kind) ? "code" : ""}`}>{f.value}</span>;

  return (
    <div className={`detail__field ${multiline ? "is-multiline" : ""}`}>
      <dt className="detail__fname">
        <Icon name={fieldIcon[f.kind] ?? "text"} size={13} />
        {f.name}
      </dt>
      <dd className="detail__fval">{value}</dd>
      <dd className="detail__factions">
        {stamp && (
          <span className="stamp" ref={stampEl} role="status">
            <b className="plate-type">Copied</b>
            {stamp.clearsIn ? <span>clears in {stamp.clearsIn}s</span> : null}
            <span className="sr-only">{f.name} copied.</span>
          </span>
        )}
        {f.concealed && !isTotp && <IconButton icon={revealed !== null ? "eyeOff" : "eye"} label={revealed !== null ? `Hide ${f.name}` : `Show ${f.name}`} onClick={toggle} />}
        <IconButton icon="copy" label={`Copy ${f.name}`} onClick={copy} />
      </dd>
    </div>
  );
}

function History({ item }: { item: ItemDetail }) {
  const [open, setOpen] = useState(false);
  const [shown, setShown] = useState<Record<number, string>>({});
  const toast = useUi((s) => s.toast);
  useEffect(() => {
    setShown({});
    setOpen(false);
  }, [item.id]);
  if (!item.history.length) return null;
  return (
    <section className="detail__history">
      <button className="detail__disclose" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <Icon name={open ? "chevronDown" : "chevronRight"} size={14} />
        Previous values <span className="detail__muted">({item.history.length})</span>
      </button>
      {open && (
        <ul>
          {item.history.map((h) => (
            <li key={h.index}>
              <span className="detail__hname">{h.fieldName}</span>
              <span className="code detail__hval selectable">{shown[h.index] ?? "•".repeat(10)}</span>
              <span className="detail__muted">replaced {relative(h.replacedAt)}</span>
              <IconButton
                icon={shown[h.index] !== undefined ? "eyeOff" : "eye"}
                label="Show previous value"
                onClick={async () => {
                  if (shown[h.index] !== undefined) {
                    const { [h.index]: _drop, ...rest } = shown;
                    void _drop;
                    return setShown(rest);
                  }
                  try {
                    const v = await api.historyReveal(item.id, h.index);
                    setShown((s) => ({ ...s, [h.index]: v }));
                  } catch (e) {
                    toast((e as ApiError).message, "error");
                  }
                }}
              />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function Detail({ snap }: { snap: Snapshot }) {
  const { selectedId, setEditing, select } = useUi();
  const { data: item, isLoading } = useItem(selectedId);
  const fav = useVaultMutation((a: { id: string; v: boolean }) => api.itemSetFavorite(a.id, a.v));
  const trash = useVaultMutation((id: string) => api.itemsTrash([id]), { success: "Moved to trash." });
  const restore = useVaultMutation((id: string) => api.itemsRestore([id]), { success: "Restored." });
  const destroy = useVaultMutation((id: string) => api.itemsDeleteForever([id]), { success: "Deleted for good." });
  const duplicate = useVaultMutation((id: string) => api.itemDuplicate(id), { success: "Duplicated." });
  const moveTo = useVaultMutation((a: { id: string; folder: string | null }) => api.itemsMove([a.id], a.folder), { success: "Moved." });
  const body = useRef<HTMLDivElement>(null);

  useGSAP(
    () => {
      if (!body.current || reduced()) return;
      gsap.fromTo(
        body.current.querySelectorAll(".detail__field, .detail__notes, .detail__history, .detail__meta, .zp-empty"),
        { x: -18, opacity: 0 },
        { x: 0, opacity: 1, duration: 0.26, ease: "power3.out", stagger: 0.028, clearProps: "transform,opacity" },
      );
    },
    { dependencies: [item?.id] },
  );

  if (!selectedId)
    return (
      <section className="detail detail--empty">
        <p className="detail__vacant">
          <Icon name="vault" size={22} />
          <span>
            Nothing selected. Pick an item from the list, or press <kbd className="zp-kbd">Ctrl</kbd> <kbd className="zp-kbd">N</kbd> to add one.
          </span>
        </p>
      </section>
    );
  if (isLoading || !item) return <section className="detail" aria-busy />;

  const template = snap.templates.find((t) => t.id === item.template);
  const folderPath: string[] = [];
  let fid = item.folderId;
  while (fid) {
    const f = snap.folders.find((x) => x.id === fid);
    if (!f) break;
    folderPath.unshift(f.name);
    fid = f.parent;
  }
  const trashed = item.trashedAt !== null;

  return (
    <section className="detail" aria-label={item.title}>
      <header className="detail__head">
        <div className="detail__stamp">
          <span className="code detail__serial">{serial(item.serial)}</span>
          <span className="detail__kind">
            <Icon name={templateIcon(template?.icon ?? "file")} size={13} /> {template?.name ?? "Item"}
          </span>
          {folderPath.length > 0 && (
            <span className="detail__kind">
              <Icon name="folder" size={13} /> {folderPath.join(" / ")}
            </span>
          )}
        </div>
        <div className="detail__titlebar">
          <h2 className="detail__title plate-type selectable">{item.title}</h2>
          <div className="detail__actions">
            {trashed ? (
              <>
                <Button size="sm" icon="restore" onClick={() => restore.mutate(item.id)}>
                  Restore
                </Button>
                <Button size="sm" variant="danger" icon="trash" onClick={() => destroy.mutate(item.id)}>
                  Delete for good
                </Button>
              </>
            ) : (
              <>
                <IconButton icon="star" label={item.favorite ? "Remove from favorites" : "Add to favorites"} active={item.favorite} onClick={() => fav.mutate({ id: item.id, v: !item.favorite })} />
                <Button size="sm" icon="edit" onClick={() => setEditing({ mode: "edit", id: item.id })}>
                  Edit
                </Button>
                <Menu
                  label="More actions"
                  align="end"
                  trigger={(p) => (
                    <button className="zp-iconbtn" aria-label="More actions" title="More actions" {...p}>
                      <Icon name="more" />
                    </button>
                  )}
                  items={[
                    { label: "Duplicate", icon: "copy", onSelect: () => duplicate.mutate(item.id, { onSuccess: (id) => select(id) }) },
                    { label: "Move to top level", icon: "folder", disabled: !item.folderId, onSelect: () => moveTo.mutate({ id: item.id, folder: null }) },
                    ...snap.folders
                      .filter((f) => f.id !== item.folderId)
                      .slice(0, 12)
                      .map((f) => ({ label: `Move to ${f.name}`, icon: "folder" as const, onSelect: () => moveTo.mutate({ id: item.id, folder: f.id }) })),
                    "divider",
                    { label: "Move to trash", icon: "trash", danger: true, onSelect: () => trash.mutate(item.id) },
                  ]}
                />
              </>
            )}
          </div>
        </div>
        {item.labels.length > 0 && (
          <ul className="detail__labels">
            {item.labels.map((l) => {
              const label = snap.labels.find((x) => x.id === l);
              return label ? (
                <li key={l} style={{ ["--c" as string]: `var(--label-${label.color})` }}>
                  {label.name}
                </li>
              ) : null;
            })}
          </ul>
        )}
      </header>

      <div className="detail__body" ref={body}>
        {item.fields.some((f) => !f.empty) ? (
          <dl className="detail__fields">
            {item.fields.map((f) => (
              <FieldRow key={f.id} item={item} f={f} />
            ))}
          </dl>
        ) : (
          !item.description && <EmptyState icon="note" title="No fields filled in" />
        )}

        {item.description && (
          <section className="detail__notes">
            <h3 className="detail__h3">Notes</h3>
            <p className="selectable">{item.description}</p>
          </section>
        )}

        <History item={item} />

        <footer className="detail__meta">
          <span>Added {date(item.createdAt)}</span>
          <span>Changed {relative(item.updatedAt)}</span>
          {trashed && <span>Trashed {relative(item.trashedAt!)}</span>}
        </footer>
      </div>
    </section>
  );
}
