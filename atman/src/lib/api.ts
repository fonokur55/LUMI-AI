import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type ChatMessage = { role: string; content: string };

/// AKASHA modell-slot választó:
/// - "auto": a backend `route_prompt` keyword-routere dönti el
/// - "eco" / "brain" / "creative": kényszerített slot a felhasználói dropdown-ból
export type SlotChoice = "auto" | "eco" | "brain" | "creative";

export type AppPaths = {
  launchRoot: string;
  dataDir: string;
  configPath: string;
  chatsDir: string;
  memoryDir: string;
  memoryDocuments: string;
  vectorsDb: string;
  profileDir: string;
  profileDb: string;
  logsDir: string;
  modelsAkasha: string;
  modelsEmbed: string;
  runtimeLlama: string;
};

export type AkashaArsenalConfig = {
  eco: string;
  brain: string;
  creative: string;
};

export type AkashaThrottleConfig = {
  ramWarningMb: number;
  ramCriticalMb: number;
  cpuCriticalPercent: number;
  minThreads: number;
  pollIntervalMs: number;
};

export type AkashaConfig = {
  modelsDir: string;
  modelsMax: number;
  arsenal: AkashaArsenalConfig;
  throttle: AkashaThrottleConfig;
  nThreads: number;
  nCtx: number;
  host: string;
  port: number;
};

export type PerformanceConfig = {
  hardwareProtectionEnabled: boolean;
  /// null | "limp" | "standard" | "pro" - null/üres = AUTO (detektált)
  forcedTier: string | null;
  /// RAM-takarékos mód: a modell minden válasz után kiakasztódik a RAM-ból.
  unloadModelAfterResponse: boolean;
};

export type AppearanceConfig = {
  /// "dark" | "light"
  theme: string;
};

export type AtmanConfig = {
  akasha: AkashaConfig;
  memory: {
    embedModelPath: string;
    chunkSize: number;
    chunkOverlap: number;
    topK: number;
  };
  profile: { displayName: string };
  performance: PerformanceConfig;
  appearance: AppearanceConfig;
};

export type AkashaStatus = "stopped" | "starting" | "ready" | "error";
export type ThrottleLevel = "normal" | "warning" | "critical";

export type HardwareSnapshot = {
  totalRamMb: number;
  availableRamMb: number;
  cpuPercent: number;
  cpuCores: number;
};

/// LUMI Adaptív Védelmi Protokoll szintjei.
export type PerfTier = "blocked" | "limp" | "standard" | "pro";

export type ModelRecommendation = {
  displayName: string;
  sizeGb: number;
  nCtx: number;
  nThreads: number;
};

export type HardwareProfile = {
  detectedTier: PerfTier;
  effectiveTier: PerfTier;
  overrideActive: boolean;
  protectionEnabled: boolean;
  message: string;
  totalRamGb: number;
  availableRamGb: number;
  cpuCores: number;
  cpuHasAvx2: boolean;
  recommendedModel: ModelRecommendation;
};

export type AkashaStatusResponse = {
  status: AkashaStatus;
  port: number;
  baseUrl: string | null;
  error: string | null;
  activeSlot: string | null;
  activeModel: string | null;
  throttleLevel: ThrottleLevel;
  hardware: HardwareSnapshot | null;
};

export type GenStartEvent = {
  slot: string;
  modelId: string;
  estimatedTotalMs: number;
};

export type GenTickEvent = {
  elapsedMs: number;
  remainingMs: number;
  charsSoFar: number;
};

export type ThrottleEvent = {
  level: ThrottleLevel;
  availableRamMb: number;
  cpuPercent: number;
  effectiveThreads: number;
};

export type DocumentInfo = {
  id: string;
  name: string;
  chunkCount: number;
  createdAt: string;
};

export type BadgeInfo = {
  id: string;
  title: string;
  description: string;
  unlocked: boolean;
  unlockedAt: string | null;
};

export type ProfileData = {
  displayName: string;
  domainHours: {
    code: number;
    writing: number;
    analysis: number;
    general: number;
  };
  badges: BadgeInfo[];
  bugsFixed: number;
  messagesSent: number;
  birthMonth: number | null;
  birthDay: number | null;
  avatarPath: string | null;
};

export type ProfileSetupStatus = {
  hasName: boolean;
  hasBirthday: boolean;
};

export type BirthdayCheck = {
  isBirthdayToday: boolean;
  needsGreeting: boolean;
  displayName: string;
};

// === Csoportok és beszélgetések ===

export type Group = {
  id: string;
  name: string;
  color: string;
  icon: string;
  sortOrder: number;
  createdAt: string;
};

export type ChatPreview = {
  id: string;
  title: string;
  groupId: string | null;
  pinned: boolean;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
  preview: string | null;
};

export type ChatMessageRecord = {
  id: string;
  role: string;
  content: string;
  createdAt: string;
};

export type ChatFull = {
  preview: ChatPreview;
  messages: ChatMessageRecord[];
};

export type MemoryNote = {
  id: string;
  title: string;
  content: string;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
};

export type DlSlot = "eco" | "brain" | "creative";

/** A 9-modelles tier × slot mátrix egy cellája. */
export type ModelStatus = {
  tier: PerfTier;
  slot: DlSlot;
  installed: boolean;
  displayName: string;
  sizeGb: number;
};

/** Az LUMI setup-állapota - runtime + a 9 modell-cella + ajánlott tier. */
export type SetupStatus = {
  runtimeInstalled: boolean;
  /** A hardware-detektálás alapján ajánlott tier (a Settings forced_tier-rel együtt). */
  recommendedTier: PerfTier;
  /** A 9 cella (3 tier × 3 slot) telepítettsége. */
  models: ModelStatus[];
  /** A KÖTELEZŐ minimum: runtime + a recommended_tier mindhárom modellje. */
  minimumReady: boolean;
};

/** A download-progress event component-mezője most már `<tier>-<slot>`
 *  formátum (pl. "standard-brain"), illetve a runtime-é `"runtime"`.
 */
export type DownloadComponent = string;

export type DownloadProgressEvent = {
  component: DownloadComponent;
  percent: number;
  downloadedBytes: number;
  totalBytes: number;
  speedMbps: number;
};

export type DownloadDoneEvent = {
  component: DownloadComponent;
};

export const api = {
  version: () => invoke<string>("app_version"),
  paths: () => invoke<AppPaths>("get_app_paths"),
  config: () => invoke<AtmanConfig>("get_config"),
  saveConfig: (config: AtmanConfig) => invoke<void>("save_app_config", { config }),
  akashaStatus: () => invoke<AkashaStatusResponse>("akasha_status"),
  akashaStart: () => invoke<AkashaStatusResponse>("akasha_start"),
  akashaStop: () => invoke<void>("akasha_stop"),
  akashaCancelGeneration: () =>
    invoke<void>("akasha_cancel_generation"),
  akashaHardware: () => invoke<HardwareSnapshot>("akasha_hardware"),
  getHardwareProfile: () => invoke<HardwareProfile>("get_hardware_profile"),
  akashaChat: (
    messages: ChatMessage[],
    useMemory: boolean,
    opts: {
      chatId?: string | null;
      chatTitle?: string | null;
      useWeb?: boolean;
      /// "eco" | "brain" | "creative" | null (null = AUTO router-alapú)
      forceSlot?: SlotChoice;
    } = {},
  ) =>
    invoke<string>("akasha_chat", {
      payload: {
        messages,
        useMemory,
        useWeb: opts.useWeb ?? false,
        forceSlot: opts.forceSlot && opts.forceSlot !== "auto" ? opts.forceSlot : null,
        chatId: opts.chatId ?? null,
        chatTitle: opts.chatTitle ?? null,
      },
    }),
  memoryImport: (filePath: string) =>
    invoke<DocumentInfo>("memory_import", { filePath }),
  memoryList: () => invoke<DocumentInfo[]>("memory_list"),
  memoryDelete: (docId: string) => invoke<void>("memory_delete", { docId }),

  // Memória-kártyák (Gemini-stílus): a felhasználó személyes infói +
  // személyiség-beállítások, amik minden chat előtt a system promptba
  // kerülnek tömör formában.
  memoryNotesList: () => invoke<MemoryNote[]>("memory_notes_list"),
  memoryNotesCreate: (title: string, content: string) =>
    invoke<MemoryNote>("memory_notes_create", { args: { title, content } }),
  memoryNotesUpdate: (id: string, title: string, content: string) =>
    invoke<void>("memory_notes_update", { args: { id, title, content } }),
  memoryNotesToggle: (id: string, enabled: boolean) =>
    invoke<void>("memory_notes_toggle", { args: { id, enabled } }),
  memoryNotesDelete: (id: string) =>
    invoke<void>("memory_notes_delete", { id }),

  // Első indítási letöltő + Beállítások › Modellek (9-cellás)
  checkSetupStatus: () => invoke<SetupStatus>("check_setup_status"),
  checkOnline: () => invoke<boolean>("check_online"),
  /** A llama-server runtime letöltése (~30-46 MB). */
  downloadRuntime: () => invoke<void>("download_runtime"),
  /** Egy konkrét tier × slot modell letöltése (~1-9 GB). */
  downloadTierModel: (tier: PerfTier, slot: DlSlot) =>
    invoke<void>("download_tier_model", { tier, slot }),
  /** Egy egész tier 3 modellje (Eco + Brain + Creative) egymás után. */
  downloadTierPack: (tier: PerfTier) =>
    invoke<void>("download_tier_pack", { tier }),
  profileGet: () => invoke<ProfileData>("profile_get"),
  profileUpdateName: (name: string) => invoke<void>("profile_update_name", { name }),
  profileRecordEvent: (kind: string) => invoke<void>("profile_record_event", { kind }),
  profileSetBirthday: (month: number, day: number) =>
    invoke<void>("profile_set_birthday", { month, day }),
  profileGetSetupStatus: () =>
    invoke<ProfileSetupStatus>("profile_get_setup_status"),
  profileCheckBirthday: () =>
    invoke<BirthdayCheck>("profile_check_birthday"),
  profileMarkBirthdayGreeted: () =>
    invoke<void>("profile_mark_birthday_greeted"),
  profileSaveAvatar: (pngBase64: string) =>
    invoke<string>("profile_save_avatar", { pngBase64 }),
  profileClearAvatar: () => invoke<void>("profile_clear_avatar"),
  /// Beolvas egy képfájlt a megadott elérési útról és visszaadja data URL-ként.
  readImageDataUrl: (path: string) =>
    invoke<string>("read_image_data_url", { path }),
  /// A mentett avatart adja vissza data URL-ként (vagy null ha nincs).
  profileGetAvatarDataUrl: () =>
    invoke<string | null>("profile_get_avatar_data_url"),

  // Beszélgetések
  chatsList: () => invoke<ChatPreview[]>("chats_list"),
  chatGet: (chatId: string) => invoke<ChatFull>("chat_get", { chatId }),
  chatCreate: (chatId: string, title: string) =>
    invoke<void>("chat_create", { args: { chatId, title } }),
  chatRename: (chatId: string, title: string) =>
    invoke<void>("chat_rename", { args: { chatId, title } }),
  chatDelete: (chatId: string) => invoke<void>("chat_delete", { chatId }),
  chatPin: (chatId: string, pinned: boolean) =>
    invoke<void>("chat_pin", { args: { chatId, pinned } }),
  chatSetGroup: (chatId: string, groupId: string | null) =>
    invoke<void>("chat_set_group", { args: { chatId, groupId } }),
  chatSearch: (query: string) =>
    invoke<ChatPreview[]>("chat_search", { query }),

  // Csoportok
  groupsList: () => invoke<Group[]>("groups_list"),
  groupCreate: (name: string, color: string, icon: string) =>
    invoke<Group>("group_create", { args: { name, color, icon } }),
  groupUpdate: (
    groupId: string,
    patch: { name?: string; color?: string; icon?: string },
  ) =>
    invoke<void>("group_update", {
      args: {
        groupId,
        name: patch.name ?? null,
        color: patch.color ?? null,
        icon: patch.icon ?? null,
      },
    }),
  groupDelete: (groupId: string) => invoke<void>("group_delete", { groupId }),
};

export function onAkashaToken(cb: (token: string) => void) {
  return listen<string>("akasha-token", (e) => cb(e.payload));
}

/// Reasoning-modellek (DeepSeek-R1 distill, stb.) belső gondolkodási
/// tokenjei - a UI külön panelben jeleníti meg, nem keverve a végleges válasszal.
export function onAkashaThinkingToken(cb: (token: string) => void) {
  return listen<string>("akasha-thinking-token", (e) => cb(e.payload));
}

// =====================================================================
//  GLOBÁLIS, EGY-PÉLDÁNYÚ AKASHA STREAM-LISTENER
// =====================================================================
//
// A komponens-szintű `listen()` regisztráció bizonytalan: React StrictMode,
// hot-reload, vagy egyszerű remount esetén többször is bejöhetett,
// minden egyes listener pedig külön-külön kapja a token-eseményeket → a
// streamBuf-ba minden token N-szer kerül be ("MaMaMaMa" vagy a teljes
// válasz 4× a buborékban). Egy `active` flag csak részben véd ez ellen.
//
// Az alábbi singleton REGISZTRÁLJA A TAURI LISTENER-T PONTOSAN EGYSZER az
// app teljes életciklusára, és egy ref-erra-alapuló dispatcher-en keresztül
// hívja az aktuális handler-eket. Komponensek `setAkashaStreamHandlers()`-szel
// regisztrálnak, és cleanup-kor lenullázzák a handler-t. Garantáltan egy
// token = egy meghívás.
export type WebSearchResult = {
  title: string;
  snippet: string;
  url: string;
};

export type AkashaStreamHandlers = {
  onToken?: (t: string) => void;
  onThinkingToken?: (t: string) => void;
  onDone?: () => void;
  onError?: (e: string) => void;
  onModelLoading?: (id: string) => void;
  onModelReady?: (id: string) => void;
  onGenStart?: (ev: GenStartEvent) => void;
  onGenTick?: (ev: GenTickEvent) => void;
  onThrottle?: (ev: ThrottleEvent) => void;
  /// Web-keresés státusz: a backend most küldte ki a queryt DDG-re.
  onWebSearching?: (query: string) => void;
  /// A DDG válaszolt - itt vannak a találatok.
  onWebResults?: (results: WebSearchResult[]) => void;
  /// A web-keresés hibára futott (network, parsing). A chat továbbmegy
  /// keresési eredmények nélkül.
  onWebError?: (message: string) => void;
  /// A backend új hardware-profilt küldött (pl. chat-küldés előtti
  /// pre-flight tier recheck után). A UI banner-rel jelezheti a usernek
  /// ha lecsökkent a tier.
  onPerfProfile?: (profile: HardwareProfile) => void;
};

let _akashaHandlers: AkashaStreamHandlers = {};
let _akashaListenersInit: Promise<void> | null = null;

function ensureAkashaListeners(): Promise<void> {
  if (_akashaListenersInit) return _akashaListenersInit;
  _akashaListenersInit = (async () => {
    await listen<string>("akasha-token", (e) =>
      _akashaHandlers.onToken?.(e.payload),
    );
    await listen<string>("akasha-thinking-token", (e) =>
      _akashaHandlers.onThinkingToken?.(e.payload),
    );
    await listen("akasha-done", () => _akashaHandlers.onDone?.());
    await listen<string>("akasha-error", (e) =>
      _akashaHandlers.onError?.(e.payload),
    );
    await listen<{ modelId: string }>("akasha-model-loading", (e) =>
      _akashaHandlers.onModelLoading?.(e.payload.modelId),
    );
    await listen<{ modelId: string }>("akasha-model-ready", (e) =>
      _akashaHandlers.onModelReady?.(e.payload.modelId),
    );
    await listen<GenStartEvent>("akasha-gen-start", (e) =>
      _akashaHandlers.onGenStart?.(e.payload),
    );
    await listen<GenTickEvent>("akasha-gen-tick", (e) =>
      _akashaHandlers.onGenTick?.(e.payload),
    );
    await listen<ThrottleEvent>("akasha-throttle", (e) =>
      _akashaHandlers.onThrottle?.(e.payload),
    );
    await listen<string>("akasha-web-searching", (e) =>
      _akashaHandlers.onWebSearching?.(e.payload),
    );
    await listen<WebSearchResult[]>("akasha-web-results", (e) =>
      _akashaHandlers.onWebResults?.(e.payload),
    );
    await listen<string>("akasha-web-error", (e) =>
      _akashaHandlers.onWebError?.(e.payload),
    );
    await listen<HardwareProfile>("akasha-perf-profile", (e) => {
      _akashaHandlers.onPerfProfile?.(e.payload);
      _perfProfileBannerHandler?.(e.payload);
    });
  })();
  return _akashaListenersInit;
}

/// Állítsd be az aktuális AKASHA stream handler-eket. A globális Tauri
/// listener egyszer regisztrálódik az app életidejében, és minden token
/// pontosan EGYSZER hívja a current handler-t.
export async function setAkashaStreamHandlers(
  handlers: AkashaStreamHandlers,
): Promise<void> {
  await ensureAkashaListeners();
  _akashaHandlers = handlers;
}

/// Lenullázza a handler-eket - komponens-cleanup-kor érdemes hívni,
/// hogy a leszakadt komponens callback-jei már ne tüzeljenek.
export function clearAkashaStreamHandlers(): void {
  _akashaHandlers = {};
}

/// Külön perf-profile handler - a ChatView és az AppShell egyaránt
/// érdekelt ebben (a ChatView a streaming, az AppShell a banner miatt),
/// és nem akarjuk hogy a stream-handler reset-elgesse egymást.
/// Ez egy AppShell-specifikus csatorna: a `set` overwrite-ol, mindig
/// legfeljebb 1 handler hallgat. AppShell mount-kor regisztrálja,
/// unmount-kor nullázza.
let _perfProfileBannerHandler:
  | ((p: HardwareProfile) => void)
  | null = null;
export function setAkashaPerfProfileBannerHandler(
  h: ((p: HardwareProfile) => void) | null,
): void {
  _perfProfileBannerHandler = h;
}

export function onAkashaDone(cb: () => void) {
  return listen("akasha-done", () => cb());
}

export function onAkashaError(cb: (err: string) => void) {
  return listen<string>("akasha-error", (e) => cb(e.payload));
}

export function onAkashaGenStart(cb: (ev: GenStartEvent) => void) {
  return listen<GenStartEvent>("akasha-gen-start", (e) => cb(e.payload));
}

export function onAkashaGenTick(cb: (ev: GenTickEvent) => void) {
  return listen<GenTickEvent>("akasha-gen-tick", (e) => cb(e.payload));
}

export function onAkashaThrottle(cb: (ev: ThrottleEvent) => void) {
  return listen<ThrottleEvent>("akasha-throttle", (e) => cb(e.payload));
}

type ModelLoadEvent = { modelId: string };

export function onAkashaModelLoading(cb: (modelId: string) => void) {
  return listen<ModelLoadEvent>("akasha-model-loading", (e) => cb(e.payload.modelId));
}

export function onAkashaModelReady(cb: (modelId: string) => void) {
  return listen<ModelLoadEvent>("akasha-model-ready", (e) => cb(e.payload.modelId));
}

export function onBadgeUnlocked(cb: (id: string) => void) {
  return listen<string>("badge-unlocked", (e) => cb(e.payload));
}

export function formatSlotLabel(slot: string | null | undefined): string {
  if (!slot) return "Akasha";
  const map: Record<string, string> = {
    eco: "Eco",
    brain: "Brain",
    creative: "Creative",
  };
  return `Akasha · ${map[slot.toLowerCase()] ?? slot}`;
}

export function formatMs(seconds: number): string {
  if (seconds < 1) return "<1s";
  return `${Math.round(seconds)}s`;
}
