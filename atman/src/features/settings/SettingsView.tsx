import { useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  type AtmanConfig,
  type HardwareProfile,
  type PerfTier,
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
export function SettingsView() {
  const [config, setConfig] = useState<AtmanConfig | null>(null);
  const [profile, setProfile] = useState<HardwareProfile | null>(null);
  const [memoryModalOpen, setMemoryModalOpen] = useState(false);

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
  }, []);

  useEffect(() => {
    load();
  }, [load]);

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
          onChange={(v) =>
            updatePerf({ forcedTier: v === "auto" ? null : v })
          }
          options={[
            { value: "auto", label: "AUTO (detektált alapján)" },
            { value: "limp", label: "Light mód - gyenge gépre" },
            { value: "standard", label: "Standard mód - átlagos gépre" },
            { value: "pro", label: "Pro mód - erős gépre" },
          ]}
          hint="⚠️ Csak akkor használd, ha tudod, mit csinálsz — gyengébb gépen a Pro mód lassú lesz, és a többi program is akadhat."
        />

        <ContextSlider
          value={config.akasha.nCtx}
          onChange={(v) => updateAkasha({ nCtx: v })}
        />
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
