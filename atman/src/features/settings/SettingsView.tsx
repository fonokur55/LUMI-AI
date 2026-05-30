import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  type AtmanConfig,
  type DlSlot,
  type DownloadDoneEvent,
  type DownloadProgressEvent,
  type ExpertStatus,
  type HardwareProfile,
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

// v0.2.0: a letöltés-állapot map kulcsa egyszerűen a slot ("szoveg" /
// "logika" / "kod" / "runtime"), mert az 1 expert = 1 fájl.

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

  const startExpertDownload = async (slot: DlSlot) => {
    setDownloads((prev) => ({
      ...prev,
      [slot]: { ...idleDownload(), status: "downloading" },
    }));
    try {
      await api.downloadExpert(slot);
    } catch (e) {
      setDownloads((prev) => ({
        ...prev,
        [slot]: { ...idleDownload(), status: "error", error: String(e) },
      }));
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
          label="Védelmi szint kézi felülírása"
          value={perf.forcedTier ?? "auto"}
          onChange={(v) => updatePerf({ forcedTier: v === "auto" ? null : v })}
          options={[
            { value: "auto", label: "AUTO (detektált alapján)" },
            { value: "limp", label: "Light — gyengébb gépre, óvatosabb" },
            { value: "standard", label: "Standard — átlagos gép" },
            { value: "pro", label: "Pro — erős gép, kevesebb throttling" },
          ]}
          hint="A v0.2.0-tól ugyanaz a 3 expert mindenkin fut — a mód csak azt szabályozza, hogy AKASHA mennyire óvatos a RAM/CPU használattal."
        />

        <ContextSlider
          value={config.akasha.nCtx}
          onChange={(v) => updateAkasha({ nCtx: v })}
        />
      </section>

      {/* ===== MODELLEK (v0.2.0: 3 specializált expert) ===== */}
      <section className="settings-card">
        <h2>Modellek</h2>
        <p className="settings-view__hint">
          A LUMI 3 specializált expertet használ: <strong>Szöveg</strong>{" "}
          (Gemma 2 2B), <strong>Logika</strong> (Qwen 2.5 Math 1.5B) és{" "}
          <strong>Kód</strong> (Qwen 2.5 Coder 3B). A Szöveg az alap (a
          telepítőben jön); a Logika és Kód az első indítás után automatikusan
          letöltődik a háttérben. Itt manuálisan újratöltheted bármelyiket.
        </p>

        {setupStatus && (
          <div className="settings-experts">
            {setupStatus.experts.map((e) => (
              <ExpertCard
                key={e.slot}
                expert={e}
                state={downloads[e.slot] ?? idleDownload()}
                onDownload={() => startExpertDownload(e.slot)}
              />
            ))}
          </div>
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

function ExpertCard({
  expert,
  state,
  onDownload,
}: {
  expert: ExpertStatus;
  state: ModelDownloadState;
  onDownload: () => void;
}) {
  const downloading = state.status === "downloading";
  return (
    <div className={`settings-model-row ${expert.installed ? "is-installed" : ""}`}>
      <span className={`settings-model-row__icon ${expert.installed ? "is-ok" : ""}`}>
        {expert.installed ? "✓" : downloading ? "⟳" : "○"}
      </span>
      <div className="settings-model-row__body">
        <div className="settings-model-row__label">
          <strong>{expert.displayName}</strong>
          {expert.bundled && (
            <span className="settings-model-row__bundled-tag">Telepítőből</span>
          )}
        </div>
        <div className="settings-model-row__meta">
          {expert.description} · ~{expert.sizeGb.toFixed(1)} GB
        </div>
        {downloading && (
          <>
            <div className="settings-model-row__bar">
              <div
                className="settings-model-row__bar-fill"
                style={{ width: `${state.percent}%` }}
              />
            </div>
            <div className="settings-model-row__meta">
              {state.totalMb > 0 ? (
                <>
                  {state.downloadedMb.toFixed(0)} / {state.totalMb.toFixed(0)} MB
                </>
              ) : (
                <>{state.downloadedMb.toFixed(0)} MB</>
              )}
              {state.speedMbps > 0 && <> · {state.speedMbps.toFixed(1)} MB/s</>}
            </div>
          </>
        )}
        {state.status === "error" && (
          <div className="settings-model-row__error">Hiba: {state.error}</div>
        )}
      </div>
      {!expert.installed && !downloading && (
        <button
          type="button"
          className="settings-view__btn settings-model-row__btn"
          onClick={onDownload}
        >
          Letöltés
        </button>
      )}
      {expert.installed && (
        <span className="settings-model-row__status">Telepítve</span>
      )}
    </div>
  );
}

// Hardware-tier címke. v0.2.0-ban a tier már csak a védelmi szintet
// szabályozza, nem a modell-választást — de a profil/diagnosztika UI-ban
// még megjelenik.
function tierLabel(t: import("../../lib/api").PerfTier): string {
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
