import { invoke } from "@tauri-apps/api/core";
import type * as T from "./types";

export function toApiError(e: unknown): T.ApiError {
  if (e && typeof e === "object" && "code" in e && "message" in e) return e as T.ApiError;
  return { code: "internal", message: typeof e === "string" ? e : "Something went wrong." };
}

async function call<R>(cmd: string, args?: Record<string, unknown>): Promise<R> {
  try {
    return await invoke<R>(cmd, args);
  } catch (e) {
    throw toApiError(e);
  }
}

export const api = {
  status: () => call<T.Status>("app_status"),
  activity: () => call<void>("activity"),
  selectVault: (path: string) => call<T.Status>("vault_select", { path }),
  closeVault: () => call<T.Status>("vault_close"),
  forgetRecent: (path: string) => call<T.Status>("vault_forget_recent", { path }),
  createVault: (a: { path: string; name: string; password: string; keyfile: string | null; yubikey: T.YubiKeyChoice | null }) =>
    call<T.Status>("vault_create", a),
  unlockPassword: (password: string, keyfile: string | null) => call<T.Status>("unlock_password", { password, keyfile }),
  unlockRecoveryCode: (code: string) => call<T.Status>("unlock_recovery_code", { code }),
  unlockRecoveryQuestions: (answers: string[]) => call<T.Status>("unlock_recovery_questions", { answers }),
  unlockDevice: () => call<T.Status>("unlock_device"),
  unlockPin: (pin: string) => call<T.Status>("unlock_pin", { pin }),
  verifyTotp: (code: string) => call<T.Status>("verify_totp", { code }),
  lock: () => call<T.Status>("lock"),
  setMasterPassword: (a: {
    current: string | null;
    password: string;
    keyfile: string | null;
    currentKeyfile: string | null;
    yubikey: T.YubiKeyChoice | null;
  }) => call<T.Status>("master_password_set", a),

  securityOverview: () => call<T.SecurityOverview>("security_overview"),
  recoveryCodesGenerate: () => call<string[]>("recovery_codes_generate"),
  recoveryCodesClear: () => call<void>("recovery_codes_clear"),
  recoveryQuestionsSet: (questions: string[], answers: string[]) => call<void>("recovery_questions_set", { questions, answers }),
  recoveryQuestionsClear: () => call<void>("recovery_questions_clear"),
  totpEnrollBegin: () => call<T.TotpEnrollment>("totp_enroll_begin"),
  totpEnrollConfirm: (code: string) => call<void>("totp_enroll_confirm", { code }),
  totpDisable: (code: string) => call<void>("totp_disable", { code }),
  pinSet: (pin: string) => call<void>("pin_set", { pin }),
  pinClear: () => call<void>("pin_clear"),
  deviceUnlockEnable: () => call<void>("device_unlock_enable"),
  deviceUnlockDisable: () => call<void>("device_unlock_disable"),
  slotRemove: (slotId: string) => call<void>("slot_remove", { slotId }),
  yubikeyList: () => call<T.YubiKeyDevice[]>("yubikey_list"),
  keyfileGenerate: (path: string) => call<void>("keyfile_generate", { path }),

  snapshot: () => call<T.Snapshot>("vault_snapshot"),
  itemGet: (id: string) => call<T.ItemDetail>("item_get", { id }),
  itemReveal: (id: string, fieldId: string) => call<string>("item_reveal", { id, fieldId }),
  historyReveal: (id: string, index: number) => call<string>("history_reveal", { id, index }),
  itemCopy: (id: string, fieldId: string) => call<T.CopyResult>("item_copy", { id, fieldId }),
  clipboardCopy: (text: string, sensitive: boolean) => call<T.CopyResult>("clipboard_copy", { text, sensitive }),
  itemTotp: (id: string, fieldId: string) => call<T.TotpCode>("item_totp", { id, fieldId }),
  itemCreate: (input: T.ItemInput) => call<string>("item_create", { input }),
  itemUpdate: (id: string, input: T.ItemInput) => call<void>("item_update", { id, input }),
  itemSetFavorite: (id: string, favorite: boolean) => call<void>("item_set_favorite", { id, favorite }),
  itemsMove: (ids: string[], folderId: string | null) => call<void>("items_move", { ids, folderId }),
  itemsTrash: (ids: string[]) => call<void>("items_trash", { ids }),
  itemsRestore: (ids: string[]) => call<void>("items_restore", { ids }),
  itemsDeleteForever: (ids: string[]) => call<void>("items_delete_forever", { ids }),
  trashEmpty: () => call<void>("trash_empty"),
  itemDuplicate: (id: string) => call<string>("item_duplicate", { id }),
  folderCreate: (name: string, parent: string | null) => call<string>("folder_create", { name, parent }),
  folderUpdate: (id: string, name: string, parent: string | null) => call<void>("folder_update", { id, name, parent }),
  folderDelete: (id: string) => call<void>("folder_delete", { id }),
  labelSave: (id: string | null, name: string, color: string) => call<string>("label_save", { id, name, color }),
  labelDelete: (id: string) => call<void>("label_delete", { id }),
  templateSave: (template: T.Template) => call<string>("template_save", { template }),
  templateDelete: (id: string) => call<void>("template_delete", { id }),
  settingsUpdate: (settings: T.VaultSettings, name: string | null) => call<void>("settings_update", { settings, name }),

  generate: (options: T.GeneratorOptions) => call<T.Generated>("generate_password", { options }),
  strength: (password: string) => call<T.Strength>("password_strength", { password }),
  health: () => call<T.HealthReport>("health_report"),
  backupsList: () => call<T.BackupInfo[]>("backups_list"),
  backupRestore: (backup: string) => call<void>("backup_restore", { backup }),
  exportText: (path: string, text: string) => call<void>("export_text", { path, text }),
  importPreview: (path: string, password: string | null, keyfile: string | null) => call<T.ImportSummary>("import_preview", { path, password, keyfile }),
  importCommit: (skipDuplicates: boolean, intoFolder: string | null) => call<T.ImportReport>("import_commit", { skipDuplicates, intoFolder }),
  importCancel: () => call<void>("import_cancel"),
  exportVault: (path: string, format: T.ExportFormat, password: string | null) => call<void>("export_vault", { path, format, password }),
};
