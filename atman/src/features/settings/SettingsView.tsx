import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  type AtmanConfig,
  type DlSlot,
  type DownloadDoneEvent,
  type DownloadProgressEvent,
  type HardwareProfile,
  type ModelStatus,
  type PerfTier,
  type SetupStatus,
} from "../../lib/api";
import { showToast } from "../../components/Toast";
import { MemoryNotesModal } from "../memory/MemoryNotesModal";
import "./SettingsView.css";

/**
 * SettingsView - átláthatóra rendezett, auto-save-vel.
 *
 * Filozófia:
 *  - Nincs "Mentés" gomb az aljon. Minden beállítás-változtatás AZONNAL
 *    perzisztálódik a backend-be (debounce-szal).
 *  - Sikeres mentés után rövid zöld "Mentve ✓" toast a képernyő tetején.
 *  - Szekciók tisztán elválasztva, prioritás szerint sorrendben.
 */
type ModelDownloadState = {
  status: "idle" | "downloading" | "extracting" | "error";
  percent: number;
  downloadedMb: number;
  totalMb: number;
  speedMbps: number;
  error?: string;
};

const idleDownload = (): ModelDownloadState => ({
  status: "idle",
  percent: 0,
  downloadedMb: 0,
  totalMb: 0,
  speedMbps: 0,
});

/** Cella-kulcs a 9-cellás letöltés-állapot map-hez. */
const cellKey = (tier: PerfTier, slot: DlSlot): string => `${tier}-${slot}`;

export function SettingsView() {
  const [config, setConfig] = useState<AtmanConfig | null>(null);
  const [profile, setProfile] = useState<HardwareProfile | null>(null);
  const [memoryModalOpen, setMemoryModalOpen] = useState(false);
  // Setup-status + 9 cellás letöltés-állapot map
  const [setupStatus, setSetupStatus] = useState<SetupStatus | null>(null);
  const [downloads, setDownloads] = useState<Record<string, ModelDownloadState>>({});

  // Auto-save debounce: az utolsó setConfig hívás után 500ms-mal mentünk.
  const saveTimerRef = useRef<number | null>(null);
  const lastSavedRef = useRef<string>("");

  const load = useCallback(async () => {
    setConfig(await api.config());
    try {
      setProfile(await api.getHardwareProfile());
    } catch (e) {
      console.error("profile load failed", e);
    }
    try {
      setSetupStatus(await api.checkSetupStatus());
    } catch (e) {
      console.error("setup status failed", e);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  // Modell-letöltés progress event-ek a Beállítások szekcióhoz - a 9
  // cellás map alapján frissít. A komponens-azonosító `<tier>-<slot>`
  // formátumú a backend event-ekben, pl. `standard-brain`.
  useEffect(() => {
    const unlistens: Array<() => void> = [];
    listen<DownloadProgressEvent>("download-progress", (event) => {
      const key = event.payload.component;
      if (key === "runtime") return;
      setDownloads((prev) => ({
        ...prev,
        [key]: {
          status: "downloading",
          percent: event.payload.percent,
          downloadedMb: event.payload.downloadedBytes / 1_048_576,
          totalMb: event.payload.totalBytes / 1_048_576,
          speedMbps: event.payload.speedMbps,
        },
      }));
    }).then((un) => unlistens.push(un));
    listen<DownloadDoneEvent>("download-done", (event) => {
      const key = event.payload.component;
      if (key === "runtime") return;
      setDownloads((prev) => {
        const next = { ...prev };
        delete next[key];
        return next;
      });
      api.checkSetupStatus().then(setSetupStatus).catch(() => {});
      showToast("Modell letöltése kész");
    }).then((un) => unlistens.push(un));
    return () => unlistens.forEach((un) => un());
  }, []);

  const startModelDownload = async (tier: PerfTier, slot: DlSlot) => {
    const key = cellKey(tier, slot);
    setDownloads((prev) => ({
      ...prev,
      [key]: { ...idleDownload(), status: "downloading" },
    }));
    try {
      await api.downloadTierModel(tier, slot);
    } catch (e) {
      setDownloads((prev) => ({
        ...prev,
        [key]: { ...idleDownload(), status: "error", error: String(e) },
      }));
      showToast(`Letöltés sikertelen: ${e}`, "error", 4000);
    }
  };

  const startTierPackDownload = async (tier: PerfTier) => {
    try {
      await api.downloadTierPack(tier);
      showToast(`${tier} tier modelljei letöltve`);
    } catch (e) {
      showToast(`Letöltés sikertelen: ${e}`, "error", 4000);
    }
  };

  // Auto-save: amikor a config változik (felhasználói interakció miatt),
  // 500ms múlva elmentjük. Ha közben újra változik, a timer reset-elődik.
  useEffect(() => {
    if (!config) return;
    const serialized = JSON.stringify(config);
    if (serialized === lastSavedRef.current) return; // nincs változás
    if (lastSavedRef.current === "") {
      // Első load - ne mentsünk azonnal, csak rögzítsük a baseline-t.
      lastSavedRef.current = serialized;
      return;
    }
    if (saveTimerRef.current) {
      window.clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = window.setTimeout(async () => {
      try {
        await api.saveConfig(config);
        lastSavedRef.current = serialized;
        showToast("Beállítás mentve");
        // A hardware profil is változhatott (forced_tier-rel) - frissítsük
        try {
          setProfile(await api.getHardwareProfile());
        } catch {
          /* ignore */
        }
      } catch (e) {
        console.error("save config failed", e);
        showToast(`Mentés hiba: ${e}`, "error", 4000);
      }
    }, 500);
    return () => {
      if (saveTimerRef.current) {
        window.clearTimeout(saveTimerRef.current);
      }
    };
  }, [config]);

  if (!config) return <div className="settings-view">Betöltés…</div>;

  // ---- Helperek a config-frissítéshez ----
  const updatePerf = (
    patch: Partial<NonNullable<AtmanConfig["performance"]>>,
  ) => {
    const defaults = {
      hardwareProtectionEnabled: true,
      forcedTier: null,
      unloadModelAfterResponse: true,
    };
    setConfig({
      ...config,
      performance: {
        ...defaults,
        ...(config.performance ?? {}),
        ...patch,
      },
    });
  };
  const updateAkasha = (patch: Partial<AtmanConfig["akasha"]>) =>
    setConfig({ ...config, akasha: { ...config.akasha, ...patch } });
  const updateMemory = (patch: Partial<AtmanConfig["memory"]>) =>
    setConfig({ ...config, memory: { ...config.memory, ...patch } });

  const perf = config.performance ?? {
    hardwareProtectionEnabled: true,
    forcedTier: null,
    unloadModelAfterResponse: true,
  };

  return (
    <div className="settings-view">
      <h1>Beállítások</h1>
      <p className="settings-view__intro">
        A változtatások <strong>automatikusan mentődnek</strong> — felül egy
        zöld jelzés mutatja, amikor elmentődött.
      </p>

      {/* ===== MEGJELENÉS (téma) ===== */}
      <section className="settings-card">
        <h2>Megjelenés</h2>
        <div className="theme-picker">
          <ThemeOption
            value="light"
            current={config.appearance?.theme ?? "light"}
            label="Világos"
            onChange={(v) => {
              setConfig({
                ...config,
                appearance: { theme: v },
              });
              document.documentElement.setAttribute("data-theme", v);
            }}
          />
          <ThemeOption
            value="dark"
            current={config.appearance?.theme ?? "light"}
            label="Sötét"
            onChange={(v) => {
              setConfig({
                ...config,
                appearance: { theme: v },
              });
              document.documentElement.setAttribute("data-theme", v);
            }}
          />
        </div>
      </section>

      {/* ===== TELJESÍTMÉNY ===== */}
      <section className="settings-card">
        <h2>Teljesítmény</h2>

        {profile && (
          <div className={`perf-tier perf-tier--${profile.effectiveTier}`}>
            <div className="perf-tier__head">
              <span className="perf-tier__dot" aria-hidden />
              <span className="perf-tier__label">
                {tierLabel(profile.effectiveTier)}
              </span>
              {profile.overrideActive && (
                <span className="perf-tier__badge">kézi felülírás</span>
              )}
              {!profile.protectionEnabled && (
                <span className="perf-tier__badge perf-tier__badge--warn">
                  védelem kikapcsolva
                </span>
              )}
            </div>
            <p className="perf-tier__msg">{profile.message}</p>
            <div className="perf-tier__meta">
              <span>
                Detektált:{" "}
                <strong>{tierLabel(profile.detectedTier)}</strong>
              </span>
              <span>
                RAM:{" "}
                <strong>
                  {profile.availableRamGb.toFixed(1)} /{" "}
                  {profile.totalRamGb.toFixed(1)} GB szabad
                </strong>
              </span>
              <span>
                CPU magok: <strong>{profile.cpuCores}</strong>
              </span>
              <span>
                AVX2: <strong>{profile.cpuHasAvx2 ? "✓" : "✗"}</strong>
              </span>
            </div>
          </div>
        )}

        <Toggle
          checked={perf.hardwareProtectionEnabled}
          onChange={(v) => updatePerf({ hardwareProtectionEnabled: v })}
          title="Védelmi protokoll bekapcsolva"
          description="Ha bekapcsolva, AKASHA automatikusan kíméletes módra vált, ha más program memóriát kér, és sosem fagyaszt le más programot. Ajánlott bekapcsolva tartani."
        />

        <Toggle
          checked={perf.unloadModelAfterResponse}
          onChange={(v) => updatePerf({ unloadModelAfterResponse: v })}
          title="RAM-takarékos mód"
          description="Ha bekapcsolva (alap), AKASHA minden válasz után kiakasztja a modellt a memóriából, így 0 GB RAM-ot foglal, amíg nem chatelsz. A következő üzenetnél újratölti (~10–15 mp). Ha kikapcsolod, a modell a RAM-ban marad — gyorsabb a következő válasz, de állandóan 5+ GB foglalt."
        />

        <SelectField
          label="Mód kézi felülírása"
          value={perf.forcedTier ?? "auto"}
          onChange={(v) => {
            const newTier = v === "auto" ? null : v;
            updatePerf({ forcedTier: newTier });
            // Ha a user új tier-re vált és annak modelljei hiányoznak,
            // figyelmeztessük a Beállítások › Modellek menüre.
            if (newTier && setupStatus) {
              const tierModels = setupStatus.models.filter(
                (m) => m.tier === newTier,
              );
              const missing = tierModels.filter((m) => !m.installed);
              if (missing.length > 0) {
                const missingGb = missing.reduce((sum, m) => sum + m.sizeGb, 0);
                showToast(
                  `Az új mód ${missing.length} modellje hiányzik (~${missingGb.toFixed(1)} GB). Töltsd le a Modellek szekcióban.`,
                  "info",
                  6000,
                );
              }
            }
          }}
          options={[
            { value: "auto", label: "AUTO (detektált alapján)" },
            { value: "limp", label: "Light mód - gyenge gépre" },
            { value: "standard", label: "Standard mód - átlagos gépre" },
            { value: "pro", label: "Pro mód - erős gépre" },
          ]}
          hint="⚠️ Csak akkor használd, ha tudod, mit csinálsz — gyengébb gépen a Pro mód lassú lesz, és a többi program is akadhat. Ha új módot választasz és annak modelljei hiányoznak, a Modellek szekcióban töltheted le őket."
        />

        <ContextSlider
          value={config.akasha.nCtx}
          onChange={(v) => updateAkasha({ nCtx: v })}
        />
      </section>

      {/* ===== MODELLEK (3 tier × 3 slot mátrix) ===== */}
      <section className="settings-card">
        <h2>Modellek</h2>
        <p className="settings-view__hint">
          A LUMI az AKASHA motort 3 mód (<strong>Light/Standard/Pro</strong>) és
          3 témakör (<strong>Eco/Brain/Creative</strong>) szerint csoportosítja.
          Az appod a géped képességei alapján az ajánlott módot használja, de
          itt bármelyik mód modelljeit letöltheted. A <strong>Brain</strong>{" "}
          modellek a kódolás-specifikus Qwen Coder családból jönnek, az{" "}
          <strong>Eco/Creative</strong> a magyar nyelven erős Gemma családból.
        </p>

        {setupStatus && (
          <>
            <ModelTierBlock
              tier="limp"
              label="Light mód — gyenge gépre, ~4 GB összesen"
              models={setupStatus.models.filter((m) => m.tier === "limp")}
              recommended={setupStatus.recommendedTier === "limp"}
              downloads={downloads}
              onDownload={startModelDownload}
              onPack={() => startTierPackDownload("limp")}
            />
            <ModelTierBlock
              tier="standard"
              label="Standard mód — átlagos gépre, ~14 GB összesen"
              models={setupStatus.models.filter((m) => m.tier === "standard")}
              recommended={setupStatus.recommendedTier === "standard"}
              downloads={downloads}
              onDownload={startModelDownload}
              onPack={() => startTierPackDownload("standard")}
            />
            <ModelTierBlock
              tier="pro"
              label="Pro mód — erős gépre, ~22 GB összesen"
              models={setupStatus.models.filter((m) => m.tier === "pro")}
              recommended={setupStatus.recommendedTier === "pro"}
              downloads={downloads}
              onDownload={startModelDownload}
              onPack={() => startTierPackDownload("pro")}
            />
          </>
        )}
      </section>

      {/* ===== MEMÓRIA-KÁRTYÁK (Gemini-stílus) ===== */}
      <section className="settings-card">
        <h2>Memória</h2>
        <p className="settings-view__hint">
          Add meg AKASHA-nak, hogy mit tudjon rólad, vagy milyen
          személyiséggel beszéljen veled.
        </p>
        <button
          type="button"
          className="settings-view__btn"
          onClick={() => setMemoryModalOpen(true)}
        >
          Memória kezelése
        </button>
      </section>

      {/* ===== DOKUMENTUM-MEMÓRIA finomhangolás (RAG) ===== */}
      <section className="settings-card">
        <h2>Dokumentum-memória (haladó)</h2>
        <NumberField
          label="Chunk méret (token)"
          value={config.memory.chunkSize}
          onChange={(v) => updateMemory({ chunkSize: v })}
          hint="Mekkora darabokban tördelje a dokumentumokat az embedding-eléshez."
        />
        <NumberField
          label="Top-K (releváns találatok száma RAG-hoz)"
          value={config.memory.topK}
          onChange={(v) => updateMemory({ topK: v })}
        />
      </section>

      <MemoryNotesModal
        open={memoryModalOpen}
        onClose={() => setMemoryModalOpen(false)}
      />
    </div>
  );
}

// =====================================================================
//  Apró újrahasznosítható mező-komponensek
// =====================================================================

function Toggle({
  checked,
  onChange,
  title,
  description,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  title: string;
  description: string;
}) {
  return (
    <label className="settings-view__toggle">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>
        <strong>{title}</strong>
        <span className="settings-view__toggle-desc">{description}</span>
      </span>
    </label>
  );
}

function NumberField({
  label,
  value,
  onChange,
  hint,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  hint?: string;
}) {
  return (
    <label className="settings-view__field">
      <span className="settings-view__field-label">{label}</span>
      <input
        type="number"
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      {hint && <span className="settings-view__field-hint">{hint}</span>}
    </label>
  );
}

function SelectField({
  label,
  value,
  onChange,
  options,
  hint,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
  hint?: string;
}) {
  return (
    <label className="settings-view__field">
      <span className="settings-view__field-label">{label}</span>
      <select value={value} onChange={(e) => onChange(e.target.value)}>
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      {hint && <span className="settings-view__field-hint">{hint}</span>}
    </label>
  );
}

/**
 * Context-méret csúszka: 4k, 8k, 12k, ... 128k (4k lépésközzel).
 * A tényleges érték tokenben tárolódik (4096, 8192, ...). Új érték a
 * következő AKASHA-újraindításnál (vagy app újraindításnál) lép életbe.
 */
function ContextSlider({
  value,
  onChange,
}: {
  value: number;
  onChange: (v: number) => void;
}) {
  // 1 = 4k token (4096), 32 = 128k token (131072)
  const STEP_TOKENS = 4096;
  const MIN_STEP = 1;
  const MAX_STEP = 32;
  // A mentett érték lehet bármi - kerekítsük a legközelebbi lépésre.
  const currentStep = Math.max(
    MIN_STEP,
    Math.min(MAX_STEP, Math.round(value / STEP_TOKENS) || MIN_STEP),
  );
  const labelK = currentStep * 4;
  return (
    <label className="settings-view__field settings-view__field--ctx">
      <span className="settings-view__field-label">
        Context-méret · <strong>{labelK}k token</strong>
      </span>
      <input
        type="range"
        min={MIN_STEP}
        max={MAX_STEP}
        step={1}
        value={currentStep}
        onChange={(e) => onChange(Number(e.target.value) * STEP_TOKENS)}
      />
      <div className="settings-view__ctx-scale" aria-hidden>
        <span>4k</span>
        <span>32k</span>
        <span>64k</span>
        <span>96k</span>
        <span>128k</span>
      </div>
      <span className="settings-view__field-hint">
        Nagyobb context = AKASHA több korábbi üzenetre emlékszik egy
        beszélgetésen belül, de több RAM-ot foglal, és lassabb a válasz.
        Az új érték a következő AKASHA-indításnál lép életbe.
      </span>
    </label>
  );
}

function ThemeOption({
  value,
  current,
  label,
  onChange,
}: {
  value: string;
  current: string;
  label: string;
  onChange: (v: string) => void;
}) {
  const selected = current === value;
  return (
    <button
      type="button"
      className={`theme-option theme-option--${value} ${selected ? "is-active" : ""}`}
      onClick={() => onChange(value)}
      aria-pressed={selected}
    >
      <div className="theme-option__preview" aria-hidden>
        <span className="theme-option__preview-sidebar" />
        <span className="theme-option__preview-main">
          <span className="theme-option__preview-bubble" />
          <span className="theme-option__preview-bubble theme-option__preview-bubble--user" />
        </span>
      </div>
      <div className="theme-option__body">
        <strong>
          {label}
          {selected && <span className="theme-option__check"> ✓</span>}
        </strong>
      </div>
    </button>
  );
}

function ModelTierBlock({
  tier,
  label,
  models,
  recommended,
  downloads,
  onDownload,
  onPack,
}: {
  tier: PerfTier;
  label: string;
  models: ModelStatus[];
  recommended: boolean;
  downloads: Record<string, ModelDownloadState>;
  onDownload: (tier: PerfTier, slot: DlSlot) => void;
  onPack: () => void;
}) {
  const eco = models.find((m) => m.slot === "eco");
  const brain = models.find((m) => m.slot === "brain");
  const creative = models.find((m) => m.slot === "creative");
  const allInstalled = eco?.installed && brain?.installed && creative?.installed;
  return (
    <div className={`settings-tier-block ${recommended ? "is-recommended" : ""}`}>
      <div className="settings-tier-block__head">
        <h3 className="settings-tier-block__title">
          {label}
          {recommended && (
            <span className="settings-tier-block__badge">Ajánlott a gépedhez</span>
          )}
        </h3>
        {!allInstalled && (
          <button
            type="button"
            className="settings-view__btn"
            onClick={onPack}
          >
            Az egész {tier === "limp" ? "Light" : tier === "pro" ? "Pro" : "Standard"} letöltése
          </button>
        )}
      </div>
      {eco && (
        <ModelCell
          model={eco}
          dl={downloads[`${tier}-eco`]}
          onDownload={() => onDownload(tier, "eco")}
        />
      )}
      {brain && (
        <ModelCell
          model={brain}
          dl={downloads[`${tier}-brain`]}
          onDownload={() => onDownload(tier, "brain")}
        />
      )}
      {creative && (
        <ModelCell
          model={creative}
          dl={downloads[`${tier}-creative`]}
          onDownload={() => onDownload(tier, "creative")}
        />
      )}
    </div>
  );
}

function ModelCell({
  model,
  dl,
  onDownload,
}: {
  model: ModelStatus;
  dl?: ModelDownloadState;
  onDownload: () => void;
}) {
  const downloading = dl?.status === "downloading";
  const slotLabel = {
    eco: "Eco",
    brain: "Brain",
    creative: "Creative",
  }[model.slot];
  return (
    <div className={`settings-model-row ${model.installed ? "is-installed" : ""}`}>
      <span className={`settings-model-row__icon ${model.installed ? "is-ok" : ""}`}>
        {model.installed ? "✓" : downloading ? "⟳" : "○"}
      </span>
      <div className="settings-model-row__body">
        <div className="settings-model-row__label">
          <span>
            <strong>{slotLabel}</strong> · {model.displayName}
          </span>
        </div>
        <div className="settings-model-row__meta">~{model.sizeGb.toFixed(1)} GB</div>
        {downloading && dl && (
          <>
            <div className="settings-model-row__bar">
              <div
                className="settings-model-row__bar-fill"
                style={{ width: `${dl.percent}%` }}
              />
            </div>
            <div className="settings-model-row__meta">
              {dl.totalMb > 0 ? (
                <>
                  {dl.downloadedMb.toFixed(0)} / {dl.totalMb.toFixed(0)} MB
                </>
              ) : (
                <>{dl.downloadedMb.toFixed(0)} MB</>
              )}
              {dl.speedMbps > 0 && <> · {dl.speedMbps.toFixed(1)} MB/s</>}
            </div>
          </>
        )}
        {dl?.status === "error" && (
          <div className="settings-model-row__error">Hiba: {dl.error}</div>
        )}
      </div>
      {!model.installed && !downloading && (
        <button
          type="button"
          className="settings-view__btn settings-model-row__btn"
          onClick={onDownload}
        >
          Letöltés
        </button>
      )}
      {model.installed && (
        <span className="settings-model-row__status">Telepítve</span>
      )}
    </div>
  );
}

function tierLabel(t: PerfTier): string {
  switch (t) {
    case "blocked":
      return "Nem futtatható";
    case "limp":
      return "Light mód";
    case "standard":
      return "Standard mód";
    case "pro":
      return "Pro mód";
  }
}
