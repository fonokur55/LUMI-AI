import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  type DownloadDoneEvent,
  type DownloadProgressEvent,
  type SetupStatus,
} from "../lib/api";
import "./FirstRunDownload.css";

type Props = {
  /** A backend `check_setup_status` válasza - csak a runtime + Szöveg
   *  expert állapotát figyeljük (a többi háttérben jön). */
  status: SetupStatus;
  /** Sikeres minimum-letöltés után - szülő frissít + bezár. */
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

/**
 * v0.2.0 first-run download wizard.
 *
 * SZŰKEBB szerep mint a v0.1.x-ben:
 *   - Csak a KÖTELEZŐ MINIMUM: runtime + Szöveg expert
 *   - A Logika és Kód NEM itt töltődik le - azt a háttér-letöltés rendszer
 *     kezeli (`BackgroundDownloadStatus.tsx`)
 *   - Ha a Szöveg expert bundle-elt és a `migrate_eco_model` már átmásolta,
 *     itt csak a runtime-letöltést kell kezelni (vagy semmit)
 *
 * Megnyílás feltétele: `status.minimumReady === false`. Ez kétféle lehet:
 *   - runtime hiányzik (és/vagy Szöveg hiányzik)
 */
export function FirstRunDownload({ status, onComplete }: Props) {
  const szovegExpert = status.experts.find((e) => e.slot === "szoveg");
  const szovegInstalled = szovegExpert?.installed ?? false;
  const szovegSizeGb = szovegExpert?.sizeGb ?? 1.6;

  const [runtime, setRuntime] = useState<CellState>(idle(status.runtimeInstalled));
  const [szoveg, setSzoveg] = useState<CellState>(idle(szovegInstalled));

  const [phase, setPhase] = useState<"intro" | "downloading" | "done">("intro");
  const [error, setError] = useState<string | null>(null);
  const startedRef = useRef(false);

  useEffect(() => {
    const unlistens: Array<() => void> = [];
    const setterFor = (component: string) => {
      if (component === "runtime") return setRuntime;
      if (component === "szoveg") return setSzoveg;
      return null;
    };
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
  }, []);

  const startMinimum = useCallback(async () => {
    if (startedRef.current) return;
    startedRef.current = true;
    setError(null);
    setPhase("downloading");

    try {
      if (!status.runtimeInstalled) {
        await api.downloadRuntime();
      }
      if (!szovegInstalled) {
        await api.downloadExpert("szoveg");
      }
      setPhase("done");
    } catch (e) {
      setError(String(e));
      startedRef.current = false;
    }
  }, [status.runtimeInstalled, szovegInstalled]);

  const allDone = runtime.status === "done" && szoveg.status === "done";

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
                A LUMI első indításához egyetlen alap-modell és az AKASHA motor
                kell. Ez kb. <strong>{(szovegSizeGb + 0.05).toFixed(1)} GB</strong>{" "}
                és pár perc — utána <strong>azonnal beszélgethetsz</strong>.
                A két szakértő-modell (Logika, Kód) később, a háttérben
                érkezik majd, amíg te chat-elsz.
              </p>
            </div>

            <div className="frd-list">
              <DownloadRow
                label="AKASHA motor (llama-server)"
                sublabel="~30–46 MB · kötelező"
                state={runtime}
                required
              />
              <DownloadRow
                label="Szöveg — Gemma 2 2B"
                sublabel={`~${szovegSizeGb.toFixed(1)} GB · általános beszélgetés, kreatív írás · kötelező`}
                state={szoveg}
                required
              />
            </div>

            <p className="frd-hint">
              A <strong>Logika</strong> (~1.0 GB) és <strong>Kód</strong> (~2.0 GB)
              expert-eket a Szöveg-letöltés után automatikusan, csendben
              elindítjuk a háttérben — addig is használhatod a LUMI-t.
            </p>

            <div className="frd-actions">
              <button
                type="button"
                className="frd-btn frd-btn--primary frd-btn--xl"
                onClick={startMinimum}
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
                Ne kapcsold ki az appot. Pár perc múlva tudsz beszélgetni.
              </p>
            </div>

            <div className="frd-list">
              <DownloadRow
                label="AKASHA motor"
                sublabel="~30–46 MB"
                state={runtime}
                required
                showProgress
              />
              <DownloadRow
                label="Szöveg expert (Gemma 2 2B)"
                sublabel={`~${szovegSizeGb.toFixed(1)} GB`}
                state={szoveg}
                required
                showProgress
              />
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
                    startMinimum();
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
                A Szöveg expert kész — most már beszélgethetsz LUMI-val.
                A Logika és Kód expertek <strong>háttérben töltődnek</strong>
                {" "}— a jobb alsó sarokban nyomon tudod követni.
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
          {sublabel && <span className="frd-row__sub">{sublabel}</span>}
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
      {required && state.status !== "done" && (
        <span className="frd-row__required-tag">kötelező</span>
      )}
    </div>
  );
}
