import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import "./UpdateBanner.css";

/**
 * UpdateBanner - automatikus frissítés-értesítő.
 *
 * Indítás után ~12 mp-cel csendben lekérdezzük a Tauri-updater endpointot
 * (a tauri.conf.json `plugins.updater.endpoints`-ben megadott URL-en a
 * `latest.json`-t). Ha van újabb verzió → fade-in a jobb-alsó téglalap.
 *
 *  - "Letöltés"        → downloadAndInstall() + relaunch()
 *                        közben a gomb folyamat-szöveget mutat ("Letöltés 47%...")
 *  - "Letöltés később" → a banner eltűnik. sessionStorage flag-gel jelöljük,
 *                        hogy ezt a verziót már elutasította a user; amíg újabb
 *                        verzió nem jön, nem zaklatjuk megint.
 *
 * Ha az endpoint nem érhető el (offline gép, vagy a manifest nem létezik),
 * a check() hibát dob, amit elnyelünk - az appot ez NEM blokkolja.
 */
export function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [exiting, setExiting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // 12 mp késleltetés: ne akadályozzuk a UI első renderét, és az AKASHA
    // start-up I/O-t. Csendben pörög a háttérben.
    const t = window.setTimeout(async () => {
      try {
        const u = await check();
        if (!u) return; // nincs frissítés
        const dismissedKey = `lumi.updateDismissed.${u.version}`;
        if (sessionStorage.getItem(dismissedKey) === "1") return;
        setUpdate(u);
      } catch (e) {
        // Network error, no manifest, signature mismatch, stb. - csendben
        // hagyjuk. A user-nek nem kell ezzel találkoznia.
        console.warn("[updater] check failed:", e);
      }
    }, 12000);
    return () => window.clearTimeout(t);
  }, []);

  const dismiss = () => {
    if (update) {
      sessionStorage.setItem(`lumi.updateDismissed.${update.version}`, "1");
    }
    setExiting(true);
    window.setTimeout(() => {
      setUpdate(null);
      setExiting(false);
    }, 200);
  };

  const downloadAndInstall = async () => {
    if (!update) return;
    setError(null);
    setProgress(0);
    let downloaded = 0;
    let total = 0;
    try {
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            total = event.data.contentLength ?? 0;
            setProgress(0);
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            if (total > 0) {
              setProgress(Math.min(100, (downloaded / total) * 100));
            }
            break;
          case "Finished":
            setProgress(100);
            break;
        }
      });
      // A letöltés és telepítés OK - újraindítjuk az appot, hogy az
      // új binárist töltsük be. A `data/` mappához senki nem nyúl.
      await relaunch();
    } catch (e) {
      setError(String(e));
      setProgress(null);
    }
  };

  if (!update) return null;

  return (
    <div
      className={`update-banner ${exiting ? "is-exiting" : ""}`}
      role="status"
      aria-live="polite"
    >
      <div className="update-banner__body">
        <div className="update-banner__title">Új frissítés elérhető</div>
        <div className="update-banner__subtitle">
          LUMI {update.version} — kattints a letöltéshez.
        </div>
        {error && <div className="update-banner__error">{error}</div>}
      </div>

      <div className="update-banner__actions">
        <button
          type="button"
          className="update-banner__primary"
          onClick={downloadAndInstall}
          disabled={progress !== null}
        >
          {progress === null
            ? "Letöltés"
            : progress >= 100
              ? "Telepítés…"
              : `Letöltés ${Math.round(progress)}%`}
        </button>
        <button
          type="button"
          className="update-banner__later"
          onClick={dismiss}
          disabled={progress !== null && progress < 100}
        >
          Letöltés később
        </button>
      </div>
    </div>
  );
}
