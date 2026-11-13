export type Uuid = string;

export type Stage = "noVault" | "locked" | "awaitingTotp" | "needsPasswordReset" | "unlocked";

export type FieldKind =
  | "text"
  | "username"
  | "email"
  | "password"
  | "url"
  | "totp"
  | "number"
  | "date"
  | "phone"
  | "multiline"
  | "hidden";

export interface RecentVault {
  path: string;
  name: string;
  exists: boolean;
}

export interface YubiKeyRequirement {
  slot: number;
  serial: number | null;
}

export interface UnlockInfo {
  keyfile: boolean;
  yubikey: YubiKeyRequirement | null;
  recoveryCodes: number;
  recoveryQuestions: string[] | null;
  deviceUnlock: string | null;
  pinAttempts: number | null;
}

export interface Status {
  stage: Stage;
  vaultPath: string | null;
  vaultLabel: string | null;
  defaultVaultPath: string;
  unlock: UnlockInfo | null;
  mustReset: boolean;
  recent: RecentVault[];
  platform: string;
  deviceUnlockBackend: "windowsHello" | "secretService" | null;
}

export interface ItemSummary {
  id: Uuid;
  serial: number;
  template: string;
  title: string;
  subtitle: string;
  url: string | null;
  folderId: Uuid | null;
  labels: Uuid[];
  favorite: boolean;
  hasTotp: boolean;
  hasPassword: boolean;
  createdAt: number;
  updatedAt: number;
  trashedAt: number | null;
}

export interface FieldView {
  id: Uuid;
  name: string;
  kind: FieldKind;
  concealed: boolean;
  value: string | null;
  empty: boolean;
}

export interface HistoryView {
  index: number;
  fieldId: Uuid;
  fieldName: string;
  replacedAt: number;
}

export interface ItemDetail {
  id: Uuid;
  serial: number;
  template: string;
  title: string;
  description: string;
  folderId: Uuid | null;
  labels: Uuid[];
  favorite: boolean;
  fields: FieldView[];
  history: HistoryView[];
  createdAt: number;
  updatedAt: number;
  trashedAt: number | null;
}

export interface Folder {
  id: Uuid;
  name: string;
  parent: Uuid | null;
  icon: string | null;
  createdAt: number;
}

export interface Label {
  id: Uuid;
  name: string;
  color: string;
}

export interface TemplateField {
  name: string;
  kind: FieldKind;
}

export interface Template {
  id: string;
  name: string;
  icon: string;
  fields: TemplateField[];
  builtin: boolean;
}

export interface VaultSettings {
  autoLockMinutes: number;
  clipboardClearSeconds: number;
  lockOnOsLock: boolean;
  lockOnMinimize: boolean;
  backupsKeep: number;
  trashRetentionDays: number;
  passwordMaxAgeDays: number;
}

export interface Snapshot {
  name: string;
  items: ItemSummary[];
  folders: Folder[];
  labels: Label[];
  templates: Template[];
  settings: VaultSettings;
}

export interface FieldInput {
  id: Uuid | null;
  name: string;
  kind: FieldKind;
  concealed: boolean;
  value: string | null;
}

export interface ItemInput {
  template: string;
  title: string;
  description: string;
  folderId: Uuid | null;
  labels: Uuid[];
  favorite: boolean;
  fields: FieldInput[];
}

export interface SlotView {
  id: Uuid;
  kind: "password" | "recoveryCode" | "recoveryQuestions" | "osKeystore";
  createdAt: number;
  detail: string | null;
}

export interface SecurityOverview {
  keyfile: boolean;
  yubikey: YubiKeyRequirement | null;
  recoveryCodes: number;
  recoveryQuestions: string[] | null;
  totp: boolean;
  pin: boolean;
  deviceUnlock: string | null;
  deviceUnlockAvailable: boolean;
  deviceUnlockBackend: string | null;
  slots: SlotView[];
  kdfMemoryMib: number;
  kdfPasses: number;
}

export interface TotpEnrollment {
  secret: string;
  uri: string;
  qrSvg: string;
}

export interface TotpCode {
  code: string;
  period: number;
  remaining: number;
}

export interface CopyResult {
  clearsIn: number | null;
}

export type GeneratorMode = "characters" | "passphrase" | "pin";

export interface GeneratorOptions {
  mode: GeneratorMode;
  length: number;
  uppercase: boolean;
  lowercase: boolean;
  digits: boolean;
  symbols: boolean;
  avoidAmbiguous: boolean;
  words: number;
  separator: string;
  capitalize: boolean;
  appendNumber: boolean;
}

export interface Generated {
  value: string;
  entropyBits: number;
}

export interface Strength {
  score: 0 | 1 | 2 | 3 | 4;
  guessesLog10: number;
  crackTime: string;
  warning: string | null;
  suggestions: string[];
}

export interface HealthIssue {
  itemId: Uuid;
  fieldId: Uuid;
  fieldName: string;
  weak: boolean;
  score: number;
  reusedWith: number;
  ageDays: number;
  old: boolean;
}

export interface HealthReport {
  totalPasswords: number;
  weak: number;
  reused: number;
  old: number;
  score: number;
  issues: HealthIssue[];
}

export interface BackupInfo {
  path: string;
  modified: number;
  size: number;
}

export interface YubiKeyDevice {
  name: string | null;
  serial: number | null;
}

export interface YubiKeyChoice {
  slot: number;
  serial: number | null;
}

export interface ApiError {
  code: string;
  message: string;
}

export type ImportSource =
  | "bitwarden"
  | "chrome"
  | "firefox"
  | "safari"
  | "onePassword"
  | "lastPass"
  | "bitwardenCsv"
  | "genericCsv"
  | "keePass"
  | "zPassword";

export interface ImportSummary {
  source: ImportSource | null;
  items: number;
  folders: number;
  withTotp: number;
  duplicates: number;
  sample: string[];
  warnings: string[];
}

export interface ImportReport {
  added: number;
  skipped: number;
  foldersCreated: number;
}

export type ExportFormat = "zpassword" | "bitwardenJson" | "csv";
