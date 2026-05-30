import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  type DlSlot,
  type DownloadDoneEvent,
  type DownloadProgressEvent,
  type SetupStatus,
} from "../lib/api";
import "./BackgroundDownloadStatus.css";

/**
 * v0.2.0 háttér-letöltés státusz pill + expandálható panel.
 *
 * MIKOR JELENIK MEG:
 *   - Az app indulása után, ha legalább egy NEM-bundled expert
 *     (Logika vagy Kód) még nincs telepítve. A főkomponens (App.tsx)
 *     ekkor meghívja `api.startBackgroundDownloads()`-t.
 *   - A jobb alsó sarokban egy kis "pill" jelenik meg: "Letöltés
 *     folyamatban · X% · 2 / 3 expert kész"
 *   - Ráklikkelve egy nagyobb panel ugrik fel részletes per-expert
 *     progresszussal (sebesség, MB, hátralévő idő-becslés).
 *
 * MIKOR TŰNIK EL:
 *   - Amikor mind a 3 expert kész (~3 mp delay után fade-out).
 *   - VAGY ha a user az "X" gombbal becsukja (de a letöltés a háttérben
 *     megy tovább, csak az UI eltűnik).
 */

type CellState = {
  status: "pending" | "downloading" | "extracting" | "done" | "error";
  percent: number;
  downloadedMb: number;
  totalMb: number;
  speedMbps: number;
  error?: string;
};

const EXPERT_LABELS: Record<DlSlot, string> = {
  szoveg: "Szöveg",
  logika: "Logika",
  kod: "Kód",
};

const initialCell = (installed: boolean, sizeGb: number): CellState => ({
  status: installed ? "done" : "pending",
  percent: installed ? 100 : 0,
  downloadedMb: installed ? sizeGb * 1024 : 0,
  totalMb: sizeGb * 1024,
  speedMbps: 0,
});

export function BackgroundDownloadStatus({
  initialStatus,
  onAllReady,
}: {
  initialStatus: SetupStatus;
  /** Callback amikor mind a 3 expert telepítve van. A szülő pl. frissítheti
   *  a ChatView slot-választóját, hogy minden expert elérhető legyen. */
  onAllReady?: () => void;
}) {
  // Csak a NEM-bundled expert-eket tartjuk nyilván (Logika, Kód).
  // A Szöveg úgyis bundle-elt / korábban letöltött, nem érintett.
  const tracked = initialStatus.experts.filter((e) => !e.bundled);
  const [cells, setCells] = useState<Record<string, CellState>>(() => {
    const init: Record<string, CellState> = {};
    for (const e of tracked) {
      init[e.slot] = initialCell(e.installed, e.sizeGb);
    }
    return init;
  });
  const [expanded, setExpanded] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const finishedTimer = useRef<number | null>(null);

  // Tauri event-eket figyelünk - per-expert progresszus
  useEffect(() => {
    const unlistens: Array<() => void> = [];
    listen<DownloadProgressEvent>("download-start", (event) => {
      const slot = event.payload.component;
      if (!(slot in cells)) return;
      setCells((c) => ({
        ...c,
        [slot]: { ...c[slot], status: "downloading", percent: 0 },
      }));
    }).then((un) => unlistens.push(un));
    listen<DownloadProgressEvent>("download-progress", (event) => {
      const slot = event.payload.component;
      setCells((c) => {
        if (!(slot in c)) return c;
        return {
          ...c,
          [slot]: {
            ...c[slot],
            status: "downloading",
            percent: event.payload.percent,
            downloadedMb: event.payload.downloadedBytes / 1_048_576,
            totalMb: event.payload.totalBytes / 1_048_576,
            speedMbps: event.payload.speedMbps,
          },
        };
      });
    }).then((un) => unlistens.push(un));
    listen<DownloadDoneEvent>("download-done", (event) => {
      const slot = event.payload.component;
      setCells((c) => {
        if (!(slot in c)) return c;
        return {
          ...c,
          [slot]: { ...c[slot], status: "done", percent: 100 },
        };
      });
    }).then((un) => unlistens.push(un));
    listen<{ slot: string; error: string }>(
      "background-download-error",
      (event) => {
        const slot = event.payload.slot;
        setCells((c) => {
          if (!(slot in c)) return c;
          return {
            ...c,
            [slot]: {
              ...c[slot],
              status: "error",
              error: event.payload.error,
            },
          };
        });
      },
    ).then((un) => unlistens.push(un));
    listen("background-downloads-complete", () => {
      // 3 mp delay-el fade-out, hogy a user lássa a "kész" állapotot
      if (finishedTimer.current) {
        window.clearTimeout(finishedTimer.current);
      }
      finishedTimer.current = window.setTimeout(() => {
        onAllReady?.();
        setDismissed(true);
      }, 3000);
    }).then((un) => unlistens.push(un));
    return () => {
      unlistens.forEach((un) => un());
      if (finishedTimer.current) {
        window.clearTimeout(finishedTimer.current);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Aggregált progresszus számítása
  const totalCells = tracked.length;
  const doneCells = tracked.filter((e) => cells[e.slot]?.status === "done").length;
  const totalSizeGb = tracked.reduce((sum, e) => sum + e.sizeGb, 0);
  const downloadedGb = tracked.reduce((sum, e) => {
    const cell = cells[e.slot];
    if (!cell) return sum;
    if (cell.status === "done") return sum + e.sizeGb;
    return sum + cell.downloadedMb / 1024;
  }, 0);
  const remainingGb = Math.max(0, totalSizeGb - downloadedGb);
  const aggregatePercent =
    totalSizeGb > 0 ? Math.min(100, (downloadedGb / totalSizeGb) * 100) : 0;
  const allDone = totalCells > 0 && doneCells === totalCells;

  // Becsült hátralévő idő összesítve (a leggyorsabb aktív letöltés sebessége alapján)
  const activeSpeedMbps = Math.max(
    ...Object.values(cells).map((c) =>
      c.status === "downloading" ? c.speedMbps : 0,
    ),
    0,
  );
  const etaSec =
    activeSpeedMbps > 0
      ? Math.round((remainingGb * 1024) / activeSpeedMbps)
      : null;

  if (dismissed || tracked.length === 0) {
    return null;
  }

  return (
    <div
      className={`bg-dl ${expanded ? "bg-dl--expanded" : ""} ${
        allDone ? "bg-dl--done" : ""
      }`}
    >
      {!expanded && (
        <button
          type="button"
          className="bg-dl__pill"
          onClick={() => setExpanded(true)}
          aria-label="Háttér-letöltés részletei"
        >
          <span className="bg-dl__pill-spinner" aria-hidden>
            {allDone ? "✓" : "⟳"}
          </span>
          <span className="bg-dl__pill-text">
            {allDone ? (
              <>Minden expert kész</>
            ) : (
              <>
                Háttér-letöltés · <strong>{aggregatePercent.toFixed(0)}%</strong>
                {" "}· {doneCells} / {totalCells} kész
              </>
            )}
          </span>
        </button>
      )}

      {expanded && (
        <div className="bg-dl__panel" role="dialog" aria-modal="false">
          <div className="bg-dl__panel-head">
            <h3>Háttér-letöltés</h3>
            <button
              type="button"
              className="bg-dl__close"
              onClick={() => setExpanded(false)}
              aria-label="Bezárás"
            >
              ×
            </button>
          </div>

          <div className="bg-dl__summary">
            <div className="bg-dl__summary-row">
              <span className="bg-dl__summary-label">Összesen</span>
              <span className="bg-dl__summary-value">
                {downloadedGb.toFixed(2)} / {totalSizeGb.toFixed(2)} GB
              </span>
            </div>
            <div className="bg-dl__summary-row">
              <span className="bg-dl__summary-label">Hátralévő</span>
              <span className="bg-dl__summary-value">
                {allDone ? (
                  "—"
                ) : (
                  <>
                    {remainingGb.toFixed(2)} GB
                    {etaSec !== null && etaSec > 0 && (
                      <span className="bg-dl__eta">
                        {" "}
                        · ~
                        {etaSec < 60
                          ? `${etaSec}s`
                          : `${Math.round(etaSec / 60)}min`}
                      </span>
                    )}
                  </>
                )}
              </span>
            </div>
            <div className="bg-dl__progress">
              <div
                className="bg-dl__progress-fill"
                style={{ width: `${aggregatePercent}%` }}
              />
            </div>
          </div>

          <div className="bg-dl__list">
            {tracked.map((e) => {
              const cell = cells[e.slot] ?? initialCell(false, e.sizeGb);
              return (
                <div key={e.slot} className={`bg-dl__row bg-dl__row--${cell.status}`}>
                  <span className="bg-dl__row-icon" aria-hidden>
                    {cell.status === "done"
                      ? "✓"
                      : cell.status === "error"
                        ? "⚠"
                        : cell.status === "downloading"
                          ? "⟳"
                          : "○"}
                  </span>
                  <div className="bg-dl__row-body">
                    <div className="bg-dl__row-label">
                      {EXPERT_LABELS[e.slot]} <span className="bg-dl__row-name">— {e.displayName}</span>
                    </div>
                    <div className="bg-dl__row-meta">
                      {cell.status === "downloading" ? (
                        <>
                          {cell.downloadedMb.toFixed(0)} / {cell.totalMb.toFixed(0)} MB
                          {cell.speedMbps > 0 && (
                            <> · {cell.speedMbps.toFixed(1)} MB/s</>
                          )}
                        </>
                      ) : cell.status === "done" ? (
                        <>Kész</>
                      ) : cell.status === "error" ? (
                        <>Hiba: {cell.error ?? "ismeretlen"}</>
                      ) : (
                        <>Várakozik…</>
                      )}
                    </div>
                    <div className="bg-dl__row-bar">
                      <div
                        className="bg-dl__row-bar-fill"
                        style={{ width: `${cell.percent}%` }}
                      />
                    </div>
                  </div>
                </div>
              );
            })}
          </div>

          {/* Hibás expert-eknél egy "Újrapróbálás" gomb */}
          {tracked.some((e) => cells[e.slot]?.status === "error") && (
            <div className="bg-dl__retry">
              <button
                type="button"
                className="bg-dl__retry-btn"
                onClick={() => {
                  // Reset az error cells-eknek + relauncholjuk a háttér-letöltést
                  setCells((c) => {
                    const updated = { ...c };
                    for (const slot of Object.keys(updated)) {
                      if (updated[slot].status === "error") {
                        updated[slot] = { ...updated[slot], status: "pending", error: undefined };
                      }
                    }
                    return updated;
                  });
                  api.startBackgroundDownloads().catch(() => {});
                }}
              >
                Sikertelen letöltések újrapróbálása
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
