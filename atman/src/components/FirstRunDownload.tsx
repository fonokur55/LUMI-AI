import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  type DlSlot,
  type DownloadComponent,
  type DownloadDoneEvent,
  type DownloadProgressEvent,
  type ModelStatus,
  type PerfTier,
  type SetupStatus,
} from "../lib/api";
import "./FirstRunDownload.css";

type Props = {
  /** A backend `check_setup_status`-ából - tartalmazza a recommended_tier-t és a 9 cellát. */
  status: SetupStatus;
  /** Sikeres letöltés után - szülő frissít + bezár. */
  onComplete: () => void;
};

type CellStatus =
  | "pending"
  | "downloading"
  | "extracting"
  | "done"
  | "error";

type CellState = {
  status: CellStatus;
  percent: number;
  downloadedMb: number;
  totalMb: number;
  speedMbps: number;
  error?: string;
};

const idle = (installed: boolean): CellState => ({
  status: installed ? "done" : "pending",
  percent: installed ? 100 : 0,
  downloadedMb: 0,
  totalMb: 0,
  speedMbps: 0,
});

const tierLabelHu = (t: PerfTier): string => {
  switch (t) {
    case "limp":
      return "Light";
    case "standard":
      return "Standard";
    case "pro":
      return "Pro";
    case "blocked":
      return "Nem futtatható";
  }
};

/**
 * v0.1.3+ first-run download wizard.
 *
 * A wizard megnyílik, ha:
 *   - a runtime hiányzik, VAGY
 *   - a recommended_tier 3 modellje közül BÁRMELYIK hiányzik.
 *
 * A user "Letöltés indítása" után automatikusan letölti a hiányzókat:
 *   1. Runtime (ha hiányzik) - egyetlen ZIP, kicsomagolás után kész
 *   2. Recommended tier 3 modellje sorban (Eco → Brain → Creative)
 *
 * Megszakíthatóság: kilépéskor a .part fájl törlődik (Rust oldal), és
 * következő indításkor a wizard megint megnyílik a hiányzó cellákra.
 */
export function FirstRunDownload({ status, onComplete }: Props) {
  const recommendedTier = status.recommendedTier;
  const tierModels: ModelStatus[] = status.models.filter(
    (m) => m.tier === recommendedTier,
  );
  const ecoModel = tierModels.find((m) => m.slot === "eco");
  const brainModel = tierModels.find((m) => m.slot === "brain");
  const creativeModel = tierModels.find((m) => m.slot === "creative");

  const tierTotalGb = tierModels.reduce((sum, m) => sum + m.sizeGb, 0);

  const [runtime, setRuntime] = useState<CellState>(idle(status.runtimeInstalled));
  const [eco, setEco] = useState<CellState>(idle(ecoModel?.installed ?? false));
  const [brain, setBrain] = useState<CellState>(idle(brainModel?.installed ?? false));
  const [creative, setCreative] = useState<CellState>(
    idle(creativeModel?.installed ?? false),
  );

  const [phase, setPhase] = useState<"intro" | "downloading" | "done">("intro");
  const [error, setError] = useState<string | null>(null);
  const startedRef = useRef(false);

  /** A backend event-azonosító megfeleltetése a frontend state-handler-nek. */
  const setterFor = useCallback(
    (component: DownloadComponent): React.Dispatch<React.SetStateAction<CellState>> | null => {
      if (component === "runtime") return setRuntime;
      // A modell-event-ek formátuma: `<tier>-<slot>` (pl. "standard-brain")
      const parts = component.split("-");
      if (parts.length !== 2) return null;
      const slot = parts[1] as DlSlot;
      if (slot === "eco") return setEco;
      if (slot === "brain") return setBrain;
      if (slot === "creative") return setCreative;
      return null;
    },
    [],
  );

  useEffect(() => {
    const unlistens: Array<() => void> = [];
    listen<DownloadProgressEvent>("download-start", (event) => {
      const set = setterFor(event.payload.component);
      if (!set) return;
      set((s) => ({ ...s, status: "downloading", percent: 0 }));
    }).then((un) => unlistens.push(un));
    listen<DownloadProgressEvent>("download-progress", (event) => {
      const set = setterFor(event.payload.component);
      if (!set) return;
      set((s) => ({
        ...s,
        status: "downloading",
        percent: event.payload.percent,
        downloadedMb: event.payload.downloadedBytes / 1_048_576,
        totalMb: event.payload.totalBytes / 1_048_576,
        speedMbps: event.payload.speedMbps,
      }));
    }).then((un) => unlistens.push(un));
    listen<DownloadDoneEvent>("download-extracting", (event) => {
      const set = setterFor(event.payload.component);
      if (!set) return;
      set((s) => ({ ...s, status: "extracting", percent: 99 }));
    }).then((un) => unlistens.push(un));
    listen<DownloadDoneEvent>("download-done", (event) => {
      const set = setterFor(event.payload.component);
      if (!set) return;
      set((s) => ({ ...s, status: "done", percent: 100 }));
    }).then((un) => unlistens.push(un));
    return () => unlistens.forEach((un) => un());
  }, [setterFor]);

  const startMandatory = useCallback(async () => {
    if (startedRef.current) return;
    startedRef.current = true;
    setError(null);
    setPhase("downloading");

    try {
      if (!status.runtimeInstalled) {
        await api.downloadRuntime();
      }
      // A recommended tier 3 modellje sorban
      if (!(ecoModel?.installed ?? false)) {
        await api.downloadTierModel(recommendedTier, "eco");
      }
      if (!(brainModel?.installed ?? false)) {
        await api.downloadTierModel(recommendedTier, "brain");
      }
      if (!(creativeModel?.installed ?? false)) {
        await api.downloadTierModel(recommendedTier, "creative");
      }
      setPhase("done");
    } catch (e) {
      setError(String(e));
      startedRef.current = false;
    }
  }, [
    status.runtimeInstalled,
    ecoModel?.installed,
    brainModel?.installed,
    creativeModel?.installed,
    recommendedTier,
  ]);

  const allDone =
    runtime.status === "done" &&
    eco.status === "done" &&
    brain.status === "done" &&
    creative.status === "done";

  return (
    <div className="frd-backdrop">
      <div className="frd-modal" role="dialog" aria-modal="true">
        {phase === "intro" && (
          <>
            <div className="frd-hero">
              <img
                src="/brand/logo.png"
                alt=""
                className="frd-logo"
                draggable={false}
              />
              <h1>Még egy lépés a chatelésig</h1>
              <p>
                Az LUMI megnézte a géped képességeit és ezt látja:{" "}
                <strong>{tierLabelHu(recommendedTier)} mód</strong>. Ehhez a
                módhoz <strong>3 modellt</strong> töltünk le, hogy AKASHA
                minden témakörre tudjon válaszolni. <strong>Egyszeri művelet</strong>{" "}
                – minden a te gépeden marad, semmilyen adat nem megy ki.
              </p>
            </div>

            <div className="frd-list">
              <DownloadRow label="AKASHA motor (llama-server)" sublabel="~30–46 MB" state={runtime} required />
              {ecoModel && (
                <DownloadRow
                  label={`Eco — ${ecoModel.displayName}`}
                  sublabel={`~${ecoModel.sizeGb.toFixed(1)} GB · gyors általános beszélgetés`}
                  state={eco}
                  required
                />
              )}
              {brainModel && (
                <DownloadRow
                  label={`Brain — ${brainModel.displayName}`}
                  sublabel={`~${brainModel.sizeGb.toFixed(1)} GB · kódolás, matematika`}
                  state={brain}
                  required
                />
              )}
              {creativeModel && (
                <DownloadRow
                  label={`Creative — ${creativeModel.displayName}`}
                  sublabel={`~${creativeModel.sizeGb.toFixed(1)} GB · kreatív írás, történet`}
                  state={creative}
                  required
                />
              )}
            </div>

            <p className="frd-hint">
              Összesen kb. <strong>{tierTotalGb.toFixed(1)} GB</strong>. Más tier
              modelljeit később a <em>Beállítások › Modellek</em> menüből
              letöltheted, ha pl. erősebb gépen futtatnád az LUMI-t.
            </p>

            <div className="frd-actions">
              <button
                type="button"
                className="frd-btn frd-btn--primary frd-btn--xl"
                onClick={startMandatory}
              >
                Letöltés indítása
              </button>
            </div>
          </>
        )}

        {phase === "downloading" && (
          <>
            <div className="frd-hero frd-hero--small">
              <h1>Letöltés folyamatban…</h1>
              <p>
                Ne kapcsold ki az appot. Ha megszakad, a következő indításkor
                a részfájl törlődik és újra kell kezdeni.
              </p>
            </div>

            <div className="frd-list">
              <DownloadRow label="AKASHA motor" sublabel="~30–46 MB" state={runtime} required showProgress />
              {ecoModel && (
                <DownloadRow label="Eco modell" sublabel={`~${ecoModel.sizeGb.toFixed(1)} GB`} state={eco} required showProgress />
              )}
              {brainModel && (
                <DownloadRow label="Brain modell" sublabel={`~${brainModel.sizeGb.toFixed(1)} GB`} state={brain} required showProgress />
              )}
              {creativeModel && (
                <DownloadRow
                  label="Creative modell"
                  sublabel={`~${creativeModel.sizeGb.toFixed(1)} GB`}
                  state={creative}
                  required
                  showProgress
                />
              )}
            </div>

            {error && (
              <div className="frd-error">
                <strong>Hiba a letöltésnél:</strong> {error}
                <p className="frd-error-hint">
                  Ellenőrizd az internetkapcsolatot, és próbáld újra.
                </p>
                <button
                  type="button"
                  className="frd-btn"
                  onClick={() => {
                    setError(null);
                    startMandatory();
                  }}
                >
                  Újrapróbálás
                </button>
              </div>
            )}
          </>
        )}

        {phase === "done" && (
          <>
            <div className="frd-hero frd-hero--small">
              <div className="frd-success">✨</div>
              <h1>Készen áll!</h1>
              <p>
                Minden modell letöltődött. Most már bármilyen témára tudsz
                kérdezni — AKASHA okosan választ a 3 modell közül a kérdés
                alapján.
              </p>
            </div>

            <div className="frd-actions">
              <button
                type="button"
                className="frd-btn frd-btn--primary frd-btn--xl"
                onClick={onComplete}
                disabled={!allDone}
              >
                Vágjunk bele
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function DownloadRow({
  label,
  sublabel,
  state,
  required,
  showProgress,
}: {
  label: string;
  sublabel?: string;
  state: CellState;
  required?: boolean;
  showProgress?: boolean;
}) {
  const statusIcon = (() => {
    if (state.status === "done") return "✓";
    if (state.status === "error") return "⚠";
    if (state.status === "downloading" || state.status === "extracting") return "⟳";
    return "○";
  })();
  return (
    <div className={`frd-row frd-row--${state.status}`}>
      <span className={`frd-row__icon frd-row__icon--${state.status}`} aria-hidden>
        {statusIcon}
      </span>
      <div className="frd-row__body">
        <div className="frd-row__head">
          <span className="frd-row__label">{label}</span>
          {sublabel && (
            <span className="frd-row__sub">
              {sublabel}
              {required && <span className="frd-row__required"> · kötelező</span>}
            </span>
          )}
        </div>
        {showProgress && state.status === "downloading" && (
          <>
            <div className="frd-row__bar">
              <div className="frd-row__bar-fill" style={{ width: `${state.percent}%` }} />
            </div>
            <div className="frd-row__meta">
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
        {showProgress && state.status === "extracting" && (
          <div className="frd-row__meta">Kicsomagolás…</div>
        )}
      </div>
    </div>
  );
}
