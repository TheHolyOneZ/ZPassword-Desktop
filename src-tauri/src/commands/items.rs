use std::time::Duration;

use serde::Serialize;
use tauri::State;
use uuid::Uuid;
use zeroize::Zeroizing;
use zpass_core::model::{Field, FieldKind, Folder, HistoryEntry, Item, Label, Template, VaultSettings};
use zpass_core::totp;
use zpass_core::util::now;

use crate::dto::{ItemDetail, ItemInput, Snapshot};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const MAX_HISTORY_PER_ITEM: usize = 30;

#[tauri::command]
pub fn vault_snapshot(state: State<AppState>) -> ApiResult<Snapshot> {
    let s = state.session();
    Ok(Snapshot::from(&s.open_ref()?.data))
}

#[tauri::command]
pub fn item_get(state: State<AppState>, id: Uuid) -> ApiResult<ItemDetail> {
    let s = state.session();
    let item = s
        .open_ref()?
        .data
        .item(&id)
        .ok_or_else(|| ApiError::new("notFound", "Item not found."))?;
    Ok(ItemDetail::from(item))
}

#[tauri::command]
pub fn item_reveal(state: State<AppState>, id: Uuid, field_id: Uuid) -> ApiResult<String> {
    let s = state.session();
    let item = s
        .open_ref()?
        .data
        .item(&id)
        .ok_or_else(|| ApiError::new("notFound", "Item not found."))?;
    let f = item
        .field(&field_id)
        .ok_or_else(|| ApiError::new("notFound", "Field not found."))?;
    Ok(f.value.clone())
}

#[tauri::command]
pub fn history_reveal(state: State<AppState>, id: Uuid, index: usize) -> ApiResult<String> {
    let s = state.session();
    let item = s
        .open_ref()?
        .data
        .item(&id)
        .ok_or_else(|| ApiError::new("notFound", "Item not found."))?;
    item.history
        .get(index)
        .map(|h| h.value.clone())
        .ok_or_else(|| ApiError::new("notFound", "No such entry."))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyResult {
    pub clears_in: Option<u32>,
}

fn copy_secret(state: &AppState, text: Zeroizing<String>, seconds: u32) -> CopyResult {
    let clear = (seconds > 0).then(|| Duration::from_secs(seconds as u64));
    state.clipboard.copy(text, clear);
    CopyResult {
        clears_in: (seconds > 0).then_some(seconds),
    }
}

#[tauri::command]
pub fn item_copy(state: State<AppState>, id: Uuid, field_id: Uuid) -> ApiResult<CopyResult> {
    let (text, secs) = {
        let s = state.session();
        let v = s.open_ref()?;
        let item = v
            .data
            .item(&id)
            .ok_or_else(|| ApiError::new("notFound", "Item not found."))?;
        let f = item
            .field(&field_id)
            .ok_or_else(|| ApiError::new("notFound", "Field not found."))?;
        let text = if f.kind == FieldKind::Totp {
            let cfg = totp::parse(&f.value)?;
            totp::code_at(&cfg, now() as u64)
        } else {
            f.value.clone()
        };
        let secs = if f.concealed || f.kind == FieldKind::Totp {
            v.data.settings.clipboard_clear_seconds
        } else {
            0
        };
        (Zeroizing::new(text), secs)
    };
    Ok(copy_secret(&state, text, secs))
}

#[tauri::command]
pub fn clipboard_copy(state: State<AppState>, text: Zeroizing<String>, sensitive: bool) -> CopyResult {
    let secs = if sensitive {
        state
            .session()
            .open
            .as_ref()
            .map(|o| o.data.settings.clipboard_clear_seconds)
            .unwrap_or(30)
    } else {
        0
    };
    copy_secret(&state, text, secs)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpCode {
    pub code: String,
    pub period: u64,
    pub remaining: u64,
}

#[tauri::command]
pub fn item_totp(state: State<AppState>, id: Uuid, field_id: Uuid) -> ApiResult<TotpCode> {
    let s = state.session();
    let item = s
        .open_ref()?
        .data
        .item(&id)
        .ok_or_else(|| ApiError::new("notFound", "Item not found."))?;
    let f = item
        .field(&field_id)
        .ok_or_else(|| ApiError::new("notFound", "Field not found."))?;
    let cfg = totp::parse(&f.value)?;
    let t = now() as u64;
    Ok(TotpCode {
        code: totp::code_at(&cfg, t),
        period: cfg.period,
        remaining: totp::remaining(&cfg, t),
    })
}

fn validate_input(input: &ItemInput) -> ApiResult<()> {
    if input.title.trim().is_empty() {
        return Err(ApiError::invalid("Give the item a title."));
    }
    if input.fields.len() > 200 {
        return Err(ApiError::invalid("Too many fields."));
    }
    for f in &input.fields {
        if f.kind == FieldKind::Totp
            && let Some(v) = &f.value
            && !v.trim().is_empty()
        {
            totp::parse(v)?;
        }
    }
    Ok(())
}

fn apply_fields(item: &mut Item, input: Vec<crate::dto::FieldInput>) {
    let t = now();
    let mut old: Vec<Field> = std::mem::take(&mut item.fields);
    let mut next = Vec::with_capacity(input.len());
    for fi in input {
        let existing = fi
            .id
            .and_then(|id| old.iter().position(|f| f.id == id))
            .map(|p| old.remove(p));
        let mut field = match existing {
            Some(mut f) => {
                if let Some(v) = fi.value
                    && v != f.value
                {
                    let tracked = f.concealed || matches!(f.kind, FieldKind::Password | FieldKind::Hidden);
                    if tracked && !f.value.is_empty() {
                        item.history.push(HistoryEntry {
                            field_id: f.id,
                            field_name: f.name.clone(),
                            value: std::mem::take(&mut f.value),
                            replaced_at: t,
                        });
                    }
                    f.value = v;
                }
                f
            }
            None => Field::new(fi.name.clone(), fi.kind, fi.value.unwrap_or_default()),
        };
        field.name = fi.name.trim().to_string();
        field.kind = fi.kind;
        field.concealed = fi.concealed;
        if fi.kind == FieldKind::Totp {
            field.value = field.value.trim().to_string();
        }
        next.push(field);
    }
    for mut f in old {
        if (f.concealed || f.kind == FieldKind::Password) && !f.value.is_empty() {
            item.history.push(HistoryEntry {
                field_id: f.id,
                field_name: f.name.clone(),
                value: std::mem::take(&mut f.value),
                replaced_at: t,
            });
        }
    }
    let excess = item.history.len().saturating_sub(MAX_HISTORY_PER_ITEM);
    item.history.drain(..excess);
    item.fields = next;
}

#[tauri::command(async)]
pub fn item_create(state: State<AppState>, input: ItemInput) -> ApiResult<Uuid> {
    validate_input(&input)?;
    let mut s = state.session();
    let v = s.open_mut()?;
    let t = now();
    let serial = v.data.take_serial();
    let mut item = Item {
        id: Uuid::new_v4(),
        serial,
        template: input.template,
        title: input.title.trim().to_string(),
        description: input.description,
        folder_id: input.folder_id,
        labels: input.labels,
        favorite: input.favorite,
        fields: vec![],
        history: vec![],
        created_at: t,
        updated_at: t,
        trashed_at: None,
    };
    apply_fields(&mut item, input.fields);
    item.history.clear();
    let id = item.id;
    v.data.items.push(item);
    s.persist()?;
    Ok(id)
}

#[tauri::command(async)]
pub fn item_update(state: State<AppState>, id: Uuid, input: ItemInput) -> ApiResult<()> {
    validate_input(&input)?;
    let mut s = state.session();
    let v = s.open_mut()?;
    let item = v
        .data
        .item_mut(&id)
        .ok_or_else(|| ApiError::new("notFound", "Item not found."))?;
    item.template = input.template;
    item.title = input.title.trim().to_string();
    item.description = input.description;
    item.folder_id = input.folder_id;
    item.labels = input.labels;
    item.favorite = input.favorite;
    apply_fields(item, input.fields);
    item.updated_at = now();
    s.persist()
}

#[tauri::command(async)]
pub fn item_set_favorite(state: State<AppState>, id: Uuid, favorite: bool) -> ApiResult<()> {
    let mut s = state.session();
    let item = s
        .open_mut()?
        .data
        .item_mut(&id)
        .ok_or_else(|| ApiError::new("notFound", "Item not found."))?;
    item.favorite = favorite;
    s.persist()
}

#[tauri::command(async)]
pub fn items_move(state: State<AppState>, ids: Vec<Uuid>, folder_id: Option<Uuid>) -> ApiResult<()> {
    let mut s = state.session();
    let v = s.open_mut()?;
    if let Some(f) = folder_id
        && !v.data.folders.iter().any(|x| x.id == f)
    {
        return Err(ApiError::new("notFound", "Folder not found."));
    }
    for item in v.data.items.iter_mut().filter(|i| ids.contains(&i.id)) {
        item.folder_id = folder_id;
        item.updated_at = now();
    }
    s.persist()
}

#[tauri::command(async)]
pub fn items_trash(state: State<AppState>, ids: Vec<Uuid>) -> ApiResult<()> {
    let mut s = state.session();
    let t = now();
    for item in s.open_mut()?.data.items.iter_mut().filter(|i| ids.contains(&i.id)) {
        item.trashed_at = Some(t);
    }
    s.persist()
}

#[tauri::command(async)]
pub fn items_restore(state: State<AppState>, ids: Vec<Uuid>) -> ApiResult<()> {
    let mut s = state.session();
    let v = s.open_mut()?;
    let folders: Vec<Uuid> = v.data.folders.iter().map(|f| f.id).collect();
    for item in v.data.items.iter_mut().filter(|i| ids.contains(&i.id)) {
        item.trashed_at = None;
        if item.folder_id.is_some_and(|f| !folders.contains(&f)) {
            item.folder_id = None;
        }
    }
    s.persist()
}

#[tauri::command(async)]
pub fn items_delete_forever(state: State<AppState>, ids: Vec<Uuid>) -> ApiResult<()> {
    let mut s = state.session();
    s.open_mut()?
        .data
        .items
        .retain(|i| !(ids.contains(&i.id) && i.trashed_at.is_some()));
    s.persist()
}

#[tauri::command(async)]
pub fn trash_empty(state: State<AppState>) -> ApiResult<()> {
    let mut s = state.session();
    s.open_mut()?.data.items.retain(|i| i.trashed_at.is_none());
    s.persist()
}

#[tauri::command(async)]
pub fn item_duplicate(state: State<AppState>, id: Uuid) -> ApiResult<Uuid> {
    let mut s = state.session();
    let v = s.open_mut()?;
    let src = v
        .data
        .item(&id)
        .ok_or_else(|| ApiError::new("notFound", "Item not found."))?
        .clone();
    let serial = v.data.take_serial();
    let t = now();
    let mut copy = src;
    copy.id = Uuid::new_v4();
    copy.serial = serial;
    copy.title = format!("{} (copy)", copy.title);
    copy.history.clear();
    copy.created_at = t;
    copy.updated_at = t;
    copy.trashed_at = None;
    for f in copy.fields.iter_mut() {
        f.id = Uuid::new_v4();
    }
    let new_id = copy.id;
    v.data.items.push(copy);
    s.persist()?;
    Ok(new_id)
}

#[tauri::command(async)]
pub fn folder_create(state: State<AppState>, name: String, parent: Option<Uuid>) -> ApiResult<Uuid> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::invalid("Folder names cannot be empty."));
    }
    let mut s = state.session();
    let v = s.open_mut()?;
    if let Some(p) = parent
        && !v.data.folders.iter().any(|f| f.id == p)
    {
        return Err(ApiError::new("notFound", "Parent folder not found."));
    }
    let id = Uuid::new_v4();
    v.data.folders.push(Folder {
        id,
        name,
        parent,
        icon: None,
        created_at: now(),
    });
    s.persist()?;
    Ok(id)
}

#[tauri::command(async)]
pub fn folder_update(state: State<AppState>, id: Uuid, name: String, parent: Option<Uuid>) -> ApiResult<()> {
    let mut s = state.session();
    let v = s.open_mut()?;
    if let Some(p) = parent
        && v.data.folder_subtree(&id).contains(&p)
    {
        return Err(ApiError::invalid("A folder cannot be moved inside itself."));
    }
    let f = v
        .data
        .folders
        .iter_mut()
        .find(|f| f.id == id)
        .ok_or_else(|| ApiError::new("notFound", "Folder not found."))?;
    if !name.trim().is_empty() {
        f.name = name.trim().to_string();
    }
    f.parent = parent;
    s.persist()
}

#[tauri::command(async)]
pub fn folder_delete(state: State<AppState>, id: Uuid) -> ApiResult<()> {
    let mut s = state.session();
    let v = s.open_mut()?;
    let parent = v
        .data
        .folders
        .iter()
        .find(|f| f.id == id)
        .ok_or_else(|| ApiError::new("notFound", "Folder not found."))?
        .parent;
    let subtree = v.data.folder_subtree(&id);
    for item in v
        .data
        .items
        .iter_mut()
        .filter(|i| i.folder_id.is_some_and(|f| subtree.contains(&f)))
    {
        item.folder_id = parent;
    }
    v.data.folders.retain(|f| !subtree.contains(&f.id));
    s.persist()
}

#[tauri::command(async)]
pub fn label_save(state: State<AppState>, id: Option<Uuid>, name: String, color: String) -> ApiResult<Uuid> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::invalid("Label names cannot be empty."));
    }
    let mut s = state.session();
    let v = s.open_mut()?;
    let id = match id.and_then(|id| v.data.labels.iter_mut().find(|l| l.id == id)) {
        Some(l) => {
            l.name = name;
            l.color = color;
            l.id
        }
        None => {
            let id = Uuid::new_v4();
            v.data.labels.push(Label { id, name, color });
            id
        }
    };
    s.persist()?;
    Ok(id)
}

#[tauri::command(async)]
pub fn label_delete(state: State<AppState>, id: Uuid) -> ApiResult<()> {
    let mut s = state.session();
    let v = s.open_mut()?;
    v.data.labels.retain(|l| l.id != id);
    for item in v.data.items.iter_mut() {
        item.labels.retain(|l| *l != id);
    }
    s.persist()
}

#[tauri::command(async)]
pub fn template_save(state: State<AppState>, template: Template) -> ApiResult<String> {
    if template.name.trim().is_empty() {
        return Err(ApiError::invalid("Templates need a name."));
    }
    let mut s = state.session();
    let v = s.open_mut()?;
    let mut t = template;
    t.builtin = false;
    if t.id.is_empty() || zpass_core::templates::builtin().iter().any(|b| b.id == t.id) {
        t.id = format!("user:{}", Uuid::new_v4());
    }
    let id = t.id.clone();
    match v.data.templates.iter_mut().find(|x| x.id == t.id) {
        Some(existing) => *existing = t,
        None => v.data.templates.push(t),
    }
    s.persist()?;
    Ok(id)
}

#[tauri::command(async)]
pub fn template_delete(state: State<AppState>, id: String) -> ApiResult<()> {
    let mut s = state.session();
    s.open_mut()?.data.templates.retain(|t| t.id != id);
    s.persist()
}

#[tauri::command(async)]
pub fn settings_update(state: State<AppState>, settings: VaultSettings, name: Option<String>) -> ApiResult<()> {
    let mut s = state.session();
    let v = s.open_mut()?;
    let mut st = settings;
    st.backups_keep = st.backups_keep.min(100);
    st.clipboard_clear_seconds = st.clipboard_clear_seconds.min(600);
    st.auto_lock_minutes = st.auto_lock_minutes.min(24 * 60);
    v.data.settings = st;
    if let Some(n) = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
        v.data.name = n;
    }
    s.persist()
}
