import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type DriveInfo = {
  letter: string;
  root: string;
  label: string;
  driveType: "removable" | "fixed" | "unknown";
  freeBytes: number;
  totalBytes: number;
};

type Stage = "picker" | "installing" | "success" | "error";

type Progress = {
  percent: number;
  currentFile: string;
};

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

function driveTypeLabel(t: string): string {
  switch (t) {
    case "removable":
      return "Pendrive / SD-kártya";
    case "fixed":
      return "Külső / belső meghajtó";
    default:
      return "Meghajtó";
  }
}

export default function App() {
  const [stage, setStage] = useState<Stage>("picker");
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [selectedRoot, setSelectedRoot] = useState<string | null>(null);
  const [progress, setProgress] = useState<Progress>({
    percent: 0,
    currentFile: "",
  });
  const [installPath, setInstallPath] = useState<string>("");
  const [error, setError] = useState<string>("");
  const [loadingDrives, setLoadingDrives] = useState(true);

  const refreshDrives = async () => {
    setLoadingDrives(true);
    try {
      const list = await invoke<DriveInfo[]>("list_drives");
      setDrives(list);
      // Az első removable-t automatikusan kiválasztjuk
      const firstRemovable = list.find((d) => d.driveType === "removable");
      if (firstRemovable) {
        setSelectedRoot(firstRemovable.root);
      } else if (list.length > 0) {
        setSelectedRoot(list[0].root);
      }
    } catch (e) {
      console.error("list_drives failed", e);
    } finally {
      setLoadingDrives(false);
    }
  };

  useEffect(() => {
    refreshDrives();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen<Progress>("install-progress", (event) => {
      setProgress(event.payload);
    }).then((un) => {
      unlisten = un;
    });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const startInstall = async () => {
    if (!selectedRoot) return;
    setStage("installing");
    setProgress({ percent: 0, currentFile: "" });
    setError("");
    try {
      const result = await invoke<{ installPath: string }>(
        "install_to_drive",
        { driveRoot: selectedRoot },
      );
      setInstallPath(result.installPath);
      setStage("success");
    } catch (e) {
      setError(String(e));
      setStage("error");
    }
  };

  const reveal = async () => {
    try {
      await invoke("reveal_in_explorer", { path: installPath });
    } catch (e) {
      console.error(e);
    }
  };

  const launch = async () => {
    try {
      await invoke("launch_lumi", { installPath });
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="installer">
      <header className="installer__header">
        <img
          src="/brand/logo.png"
          alt=""
          className="installer__logo"
          draggable={false}
        />
        <h1>LUMI USB Telepítő</h1>
        <p className="installer__subtitle">
          Hordozható LUMI telepítés pendrive-ra vagy külső meghajtóra
        </p>
      </header>

      {stage === "picker" && (
        <section className="installer__body installer__body--picker">
          <h2 className="installer__section-title">
            Hová telepítsem a LUMI-t?
          </h2>

          {loadingDrives ? (
            <p className="installer__loading">Meghajtók keresése…</p>
          ) : drives.length === 0 ? (
            <div className="installer__empty">
              <p>
                Nem találtam egyetlen meghajtót sem. Csatlakoztass egy
                pendrive-ot vagy külső lemezt.
              </p>
              <button
                type="button"
                className="installer__btn-link"
                onClick={refreshDrives}
              >
                Újra keresés
              </button>
            </div>
          ) : (
            <>
              <ul className="installer__drive-list">
                {drives.map((d) => {
                  const selected = d.root === selectedRoot;
                  const enoughSpace = d.freeBytes >= 100 * 1024 * 1024; // 100 MB
                  return (
                    <li key={d.root}>
                      <button
                        type="button"
                        className={`installer__drive ${
                          selected ? "is-selected" : ""
                        } ${enoughSpace ? "" : "is-disabled"}`}
                        onClick={() => enoughSpace && setSelectedRoot(d.root)}
                        disabled={!enoughSpace}
                      >
                        <span className="installer__drive-letter">
                          {d.letter}
                        </span>
                        <span className="installer__drive-info">
                          <span className="installer__drive-name">
                            {d.label || driveTypeLabel(d.driveType)}
                          </span>
                          <span className="installer__drive-sub">
                            {formatBytes(d.freeBytes)} szabad ·{" "}
                            {formatBytes(d.totalBytes)} teljes ·{" "}
                            {driveTypeLabel(d.driveType)}
                          </span>
                        </span>
                        {selected && (
                          <span
                            className="installer__drive-check"
                            aria-hidden
                          >
                            ✓
                          </span>
                        )}
                      </button>
                    </li>
                  );
                })}
              </ul>
              <button
                type="button"
                className="installer__btn-link installer__refresh"
                onClick={refreshDrives}
              >
                ↻ Frissítés
              </button>
            </>
          )}

          <div className="installer__actions">
            <button
              type="button"
              className="installer__btn-primary"
              onClick={startInstall}
              disabled={!selectedRoot || drives.length === 0}
            >
              Telepítés ▶
            </button>
          </div>
        </section>
      )}

      {stage === "installing" && (
        <section className="installer__body installer__body--progress">
          <h2 className="installer__section-title">
            Telepítés folyamatban…
          </h2>
          <p className="installer__target">Cél: {selectedRoot}LUMI</p>
          <div className="installer__progress-bar">
            <div
              className="installer__progress-fill"
              style={{ width: `${progress.percent}%` }}
            />
          </div>
          <p className="installer__progress-text">
            {Math.round(progress.percent)}%
            {progress.currentFile && (
              <span className="installer__progress-file">
                {" — "}
                {progress.currentFile}
              </span>
            )}
          </p>
        </section>
      )}

      {stage === "success" && (
        <section className="installer__body installer__body--success">
          <div className="installer__success-mark" aria-hidden>
            ✨
          </div>
          <h2 className="installer__section-title">Sikerült!</h2>
          <p className="installer__success-text">
            LUMI a következő helyre került:
          </p>
          <p className="installer__path">{installPath}</p>
          <div className="installer__actions">
            <button
              type="button"
              className="installer__btn-secondary"
              onClick={reveal}
            >
              Megnyitás az Intézőben
            </button>
            <button
              type="button"
              className="installer__btn-primary"
              onClick={launch}
            >
              LUMI indítása
            </button>
          </div>
        </section>
      )}

      {stage === "error" && (
        <section className="installer__body installer__body--error">
          <div className="installer__error-mark" aria-hidden>
            ⚠
          </div>
          <h2 className="installer__section-title">Valami félrement</h2>
          <p className="installer__error-text">{error}</p>
          <div className="installer__actions">
            <button
              type="button"
              className="installer__btn-primary"
              onClick={() => setStage("picker")}
            >
              Vissza
            </button>
          </div>
        </section>
      )}
    </div>
  );
}
