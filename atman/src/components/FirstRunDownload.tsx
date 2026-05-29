import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  type DownloadComponent,
  type DownloadProgressEvent,
  type DownloadDoneEvent,
  type SetupStatus,
} from "../lib/api";
import "./FirstRunDownload.css";

type Props = {
  /** Az állapot a Tauri backend `check_setup_status`-ából. */
  status: SetupStatus;
  /** Sikeres letöltés után - a szülő frissíti az állapotát és bezárja. */
  onComplete: () => void;
};

type ComponentStatus =
  | "pending"   // még semmi nem indult
  | "downloading"
  | "extracting" // csak a runtime-nál
  | "done"
  | "error";

type ComponentState = {
  status: ComponentStatus;
  percent: number;
  downloadedMb: number;
  totalMb: number;
  speedMbps: number;
  error?: string;
};

const initialState = (installed: boolean): ComponentState => ({
  status: installed ? "done" : "pending",
  percent: installed ? 100 : 0,
  downloadedMb: 0,
  totalMb: 0,
  speedMbps: 0,
});

/**
 * First-run modellek + runtime letöltő.
 *
 * Áron specifikáció:
 *  - Eco modell + llama-server runtime KÖTELEZŐ. Ezekkel kezdődik.
 *  - Brain és Creative OPCIONÁLIS. A user kihagyhatja, később
 *    a Beállítások › Modellek szekcióban letöltheti.
 *  - Ha nincs net: hibajelzés, az app nem indul el (az App.tsx kezeli).
 *  - Megszakítható: ha a user kilép, a .part fájl törlődik (Rust oldal),
 *    következő indításkor 0-ról kezdjük.
 */
export function FirstRunDownload({ status, onComplete }: Props) {
  const [runtime, setRuntime] = useState<ComponentState>(
    initialState(status.runtimeInstalled),
  );
  const [eco, setEco] = useState<ComponentState>(
    initialState(status.ecoInstalled),
  );
  const [brain, setBrain] = useState<ComponentState>(
    initialState(status.brainInstalled),
  );
  const [creative, setCreative] = useState<ComponentState>(
    initialState(status.creativeInstalled),
  );

  // Mit tölt épp - a "Letöltés indítása" után kötelezőek után végigfut.
  // A `phase` "intro" → "downloading" → "optional" → "done".
  const [phase, setPhase] = useState<"intro" | "downloading" | "done">(
    "intro",
  );
  const [error, setError] = useState<string | null>(null);
  const startedRef = useRef(false);

  const setterFor = (c: DownloadComponent) => {
    if (c === "runtime") return setRuntime;
    if (c === "eco") return setEco;
    if (c === "brain") return setBrain;
    return setCreative;
  };

  // Eseményfigyelők beállítása (egyszer)
  useEffect(() => {
    const unlistens: Array<() => void> = [];
    listen<DownloadProgressEvent>("download-start", (event) => {
      const set = setterFor(event.payload.component);
      set((s) => ({ ...s, status: "downloading", percent: 0 }));
    }).then((un) => unlistens.push(un));
    listen<DownloadProgressEvent>("download-progress", (event) => {
      const set = setterFor(event.payload.component);
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
      set((s) => ({ ...s, status: "extracting", percent: 99 }));
    }).then((un) => unlistens.push(un));
    listen<DownloadDoneEvent>("download-done", (event) => {
      const set = setterFor(event.payload.component);
      set((s) => ({ ...s, status: "done", percent: 100 }));
    }).then((un) => unlistens.push(un));

    return () => {
      unlistens.forEach((un) => un());
    };
  }, []);

  /** Megpróbálja letölteni a komponenst; hiba esetén `error` állapotot ad. */
  const downloadOne = useCallback(
    async (component: DownloadComponent): Promise<boolean> => {
      try {
        await api.downloadComponent(component);
        return true;
      } catch (e) {
        const setter = setterFor(component);
        setter((s) => ({
          ...s,
          status: "error",
          error: String(e),
        }));
        setError(String(e));
        return false;
      }
    },
    [],
  );

  /** Kötelező letöltés: runtime + eco (ha hiányoznak). */
  const startMandatory = useCallback(async () => {
    if (startedRef.current) return;
    startedRef.current = true;
    setError(null);
    setPhase("downloading");

    if (!status.runtimeInstalled) {
      const ok = await downloadOne("runtime");
      if (!ok) return;
    }
    if (!status.ecoInstalled) {
      const ok = await downloadOne("eco");
      if (!ok) return;
    }
    // A kötelezők megvannak - a "done" fázis enged tovább az appba.
    setPhase("done");
  }, [downloadOne, status.ecoInstalled, status.runtimeInstalled]);

  // Opcionális modellek letöltése a "done" fázisból
  const downloadOptional = async (component: DownloadComponent) => {
    await downloadOne(component);
  };

  const allMandatoryDone =
    runtime.status === "done" && eco.status === "done";

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
                A LUMI motorjához (AKASHA) le kell töltenünk pár dolgot.
                <strong> Egyszeri művelet</strong>, és minden a te gépeden
                marad — semmilyen adat nem megy ki később.
              </p>
            </div>

            <div className="frd-list">
              <DownloadRow
                label="AKASHA motor (llama-server)"
                sublabel="~30–46 MB"
                state={runtime}
                required
              />
              <DownloadRow
                label="Eco modell — gyors általános beszélgetés"
                sublabel="~2 GB"
                state={eco}
                required
              />
              <DownloadRow
                label="Brain modell — kódolás, matematika"
                sublabel="~4.7 GB · opcionális"
                state={brain}
              />
              <DownloadRow
                label="Creative modell — kreatív írás, történet"
                sublabel="~4.9 GB · opcionális"
                state={creative}
              />
            </div>

            <p className="frd-hint">
              <strong>Ajánlott:</strong> az összes modell letöltése a teljes
              élményhez. A Brain és Creative bármikor letölthető később a
              Beállítások › Modellek menüből is.
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
                Ne kapcsold ki az appot. Ha megszakad, a következő
                indításkor újra kell kezdeni a részfájlt.
              </p>
            </div>

            <div className="frd-list">
              <DownloadRow
                label="AKASHA motor (llama-server)"
                sublabel="~30–46 MB"
                state={runtime}
                required
                showProgress
              />
              <DownloadRow
                label="Eco modell"
                sublabel="~2 GB"
                state={eco}
                required
                showProgress
              />
            </div>

            {error && (
              <div className="frd-error">
                <strong>Hiba:</strong> {error}
                <button
                  type="button"
                  className="frd-btn"
                  onClick={() => {
                    startedRef.current = false;
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
                Az alap modell megvan, már chatelhetsz. Ha szeretnéd a Brain
                vagy Creative modellt is letölteni, megteheted most, vagy
                bármikor később a Beállítások menüből.
              </p>
            </div>

            <div className="frd-list">
              <DownloadRow
                label="Brain modell — kódolás, matematika"
                sublabel="~4.7 GB"
                state={brain}
                actionLabel={
                  brain.status === "pending" ? "Letöltés" : undefined
                }
                onAction={() => downloadOptional("brain")}
                showProgress={brain.status !== "pending"}
              />
              <DownloadRow
                label="Creative modell — kreatív írás"
                sublabel="~4.9 GB"
                state={creative}
                actionLabel={
                  creative.status === "pending" ? "Letöltés" : undefined
                }
                onAction={() => downloadOptional("creative")}
                showProgress={creative.status !== "pending"}
              />
            </div>

            <div className="frd-actions">
              <button
                type="button"
                className="frd-btn frd-btn--primary frd-btn--xl"
                onClick={onComplete}
                disabled={!allMandatoryDone}
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

/**
 * Egyetlen letöltő-sor: ikon + label + sublabel + progress (ha kell) +
 * akció (opcionális modelleknél a "Letöltés" gomb).
 */
function DownloadRow({
  label,
  sublabel,
  state,
  required,
  showProgress,
  actionLabel,
  onAction,
}: {
  label: string;
  sublabel?: string;
  state: ComponentState;
  required?: boolean;
  showProgress?: boolean;
  actionLabel?: string;
  onAction?: () => void;
}) {
  const statusIcon = (() => {
    if (state.status === "done") return "✓";
    if (state.status === "error") return "⚠";
    if (state.status === "downloading" || state.status === "extracting")
      return "⟳";
    return "○";
  })();

  const statusClass = `frd-row__icon frd-row__icon--${state.status}`;

  return (
    <div className={`frd-row frd-row--${state.status}`}>
      <span className={statusClass} aria-hidden>
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
              <div
                className="frd-row__bar-fill"
                style={{ width: `${state.percent}%` }}
              />
            </div>
            <div className="frd-row__meta">
              {state.totalMb > 0 ? (
                <>
                  {state.downloadedMb.toFixed(0)} / {state.totalMb.toFixed(0)} MB
                </>
              ) : (
                <>{state.downloadedMb.toFixed(0)} MB</>
              )}
              {state.speedMbps > 0 && (
                <> · {state.speedMbps.toFixed(1)} MB/s</>
              )}
            </div>
          </>
        )}
        {showProgress && state.status === "extracting" && (
          <div className="frd-row__meta">Kicsomagolás…</div>
        )}
      </div>
      {actionLabel && state.status === "pending" && (
        <button
          type="button"
          className="frd-btn frd-btn--small"
          onClick={onAction}
        >
          {actionLabel}
        </button>
      )}
    </div>
  );
}
