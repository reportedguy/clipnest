// Typed wrappers around the Rust commands (src-tauri/src/commands.rs, library.rs).
// Keep these in sync with the Rust structs (serde uses camelCase).
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export interface Category {
  id: number;
  name: string;
  hint: string | null;
  color: string;
  icon: string | null;
  sortOrder: number;
  isSystem: boolean;
  itemCount: number;
  latestThumb: string | null;
}

export type CardStatus = "queued" | "processing" | "done" | "failed";

export interface Card {
  id: number;
  categoryId: number;
  categoryName: string;
  categoryColor: string;
  imagePath: string;
  thumbPath: string;
  width: number;
  height: number;
  userNote: string;
  ocrText: string | null;
  title: string | null;
  summary: string | null;
  tags: string[];
  urls: string[];
  mode: "text" | "image";
  autoUpgraded: boolean;
  categoryLocked: boolean;
  status: CardStatus;
  error: string | null;
  deadlineAt: number | null;
  deadlineLabel: string | null;
  deadlineDone: boolean;
  /** Shown in the strip and reminded (note shows intent, or set by hand). */
  deadlineImportant: boolean;
  createdAt: number;
  /** Every category the card is in, primary (best match) first. */
  categories: CardCategory[];
}

export interface CardCategory {
  id: number;
  name: string;
  color: string;
}

/** Feed filters: every selected category and tag must match; text is the header search. */
export interface FeedQuery {
  categoryIds: number[];
  tags: string[];
  text: string;
}

export interface FeedPage {
  cards: Card[];
  total: number;
}

export interface TagCount {
  tag: string;
  count: number;
}

export interface FilterOptions {
  categories: Category[];
  tags: TagCount[];
}

export type HotkeyStatus =
  | { kind: "active"; accelerator: string }
  | { kind: "usingFallback"; accelerator: string; wanted: string; reason: string }
  | { kind: "failed"; wanted: string; reason: string };

export interface AppInfo {
  version: string;
  dataDir: string;
  schemaVersion: number;
  /** Installed Windows OCR languages; empty means OCR can't run. */
  ocrLanguages: string[];
  hotkey: HotkeyStatus;
  /** One-time message, e.g. the library was recovered from a backup. */
  notice: string | null;
}

export type ThemePref = "system" | "light" | "dark";

export interface Settings {
  hotkey: string;
  startWithWindows: boolean;
  closeToTray: boolean;
  theme: ThemePref;
  deadlineLeadHours: number;
  notifyHour: number;
  aiPaused: boolean;
  onboardingDone: boolean;
  aiProfile: string;
  aiDownloadConsent: boolean;
  aiVisionConsent: boolean;
  pauseOnBatterySaver: boolean;
  /** Show each card's summary in the feed (off: title only, summary on click). */
  showSummary: boolean;
}

/** One downloadable AI choice (a `models.json` profile). */
export interface AiProfile {
  id: string;
  modelName: string;
  /** Engine plus text model. */
  downloadBytes: number;
  minRamGb: number;
}

export interface AiProfiles {
  /** Installed memory in GB (0 if unknown). */
  totalRamGb: number;
  recommended: string;
  /** Smallest first. */
  profiles: AiProfile[];
}

export interface AiStatus {
  state: "notInstalled" | "downloading" | "paused" | "error" | "ready";
  modelName: string;
  totalBytes: number;
  downloadedBytes: number;
  error: string | null;
  engineRunning: boolean;
  visionInstalled: boolean;
  visionBytes: number;
  visionDownloading: boolean;
}

export interface QueueStatus {
  /** Queued or processing. */
  pending: number;
  /** Of `pending`: already read, waiting only for the AI. */
  waitingForAi: number;
  /** Of `waitingForAi`: Image-mode items, which need the image model. */
  waitingForVision: number;
  failed: number;
  paused: boolean;
  /** Paused because Windows battery saver is on. */
  batterySaver: boolean;
  aiReady: boolean;
}

export interface CategoryInput {
  name?: string;
  hint?: string;
  color?: string;
}

/** Starting categories for a profession (config/category-presets.json). */
export interface Preset {
  id: string;
  name: string;
  description: string;
  icon: string;
  categories: { name: string; hint: string }[];
}

export interface SetupOutcome {
  added: number;
  removed: number;
  /** Screenshots queued to be filed again into the new categories. */
  queued: number;
}

export interface StorageInfo {
  dataDir: string;
  bytes: number;
  screenshots: number;
}

/** Shows a Tauri accelerator the way Windows users read it ("Super" is the Win key). */
export function formatHotkey(accelerator: string): string {
  return accelerator.replace(/\bSuper\b/g, "Win");
}

/** URL for a saved image or thumbnail (relative path as stored in the DB). */
export function mediaUrl(relativePath: string): string {
  return convertFileSrc(`lib/${relativePath}`, "rsmedia");
}

export const ipc = {
  appInfo: () => invoke<AppInfo>("app_info"),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (patch: Partial<Settings>) => invoke<Settings>("update_settings", { patch }),

  listCategories: () => invoke<Category[]>("list_categories"),
  createCategory: (input: CategoryInput) => invoke<number>("create_category", { input }),
  updateCategory: (id: number, input: CategoryInput) => invoke<void>("update_category", { id, input }),
  reorderCategories: (ids: number[]) => invoke<void>("reorder_categories", { ids }),
  deleteCategory: (id: number) => invoke<number>("delete_category", { id }),
  categoryPresets: () => invoke<Preset[]>("category_presets"),
  /** `replace`: the list becomes all the categories (setup). Otherwise only missing ones are added. */
  applyCategorySetup: (categories: CategoryInput[], replace: boolean) =>
    invoke<SetupOutcome>("apply_category_setup", { categories, replace }),

  feed: (query: FeedQuery, offset: number, limit: number) => invoke<FeedPage>("feed", { query, offset, limit }),
  filterOptions: () => invoke<FilterOptions>("filter_options"),
  /** Tags on screenshots in every selected category (all tags when none). */
  tagOptions: (categoryIds: number[]) => invoke<TagCount[]>("tag_options", { categoryIds }),
  /** Sets a card's categories by hand (the AI won't change them afterwards). */
  setCardCategories: (id: number, categoryIds: number[]) => invoke<void>("set_card_categories", { id, categoryIds }),
  updateCardText: (id: number, title: string, summary: string) => invoke<void>("update_card_text", { id, title, summary }),
  updateCardNote: (id: number, note: string) => invoke<void>("update_card_note", { id, note }),
  deleteCard: (id: number) => invoke<void>("delete_card", { id }),
  restoreCard: (id: number) => invoke<void>("restore_card", { id }),
  retryCard: (id: number) => invoke<void>("retry_card", { id }),
  copyCardImage: (id: number) => invoke<void>("copy_card_image", { id }),
  openCardImage: (id: number) => invoke<void>("open_card_image", { id }),
  /** Opens an http(s) link in the default browser. */
  openUrl: (url: string) => invoke<void>("open_url", { url }),

  queueStatus: () => invoke<QueueStatus>("queue_status"),
  aiStatus: () => invoke<AiStatus>("ai_status"),
  aiProfiles: () => invoke<AiProfiles>("ai_profiles"),
  aiStartSetup: () => invoke<void>("ai_start_setup"),
  aiStartVisionSetup: () => invoke<void>("ai_start_vision_setup"),
  aiPauseDownload: () => invoke<void>("ai_pause_download"),
  aiDelete: () => invoke<void>("ai_delete"),
  /** Asks the AI to add newly created categories to existing screenshots where they fit. */
  recheckCategories: () => invoke<number>("recheck_categories"),
  /** Re-runs the AI on every screenshot (keeps hand-picked categories and edited titles). */
  refileAll: () => invoke<number>("refile_all"),
  upcomingDeadlines: () => invoke<Card[]>("upcoming_deadlines"),
  /** `date` is YYYY-MM-DD, or null to clear. */
  setDeadline: (id: number, date: string | null, label: string) => invoke<void>("set_deadline", { id, date, label }),
  markDeadlineDone: (id: number) => invoke<void>("mark_deadline_done", { id }),
  storageInfo: () => invoke<StorageInfo>("storage_info"),
  /** Writes a zip to Downloads and shows it in Explorer; returns its path. */
  exportAll: () => invoke<string>("export_all"),
  devFakeAi: () => invoke<number>("dev_fake_ai"),
};
