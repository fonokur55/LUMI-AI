import { getCurrentWindow } from "@tauri-apps/api/window";
import "./WindowControls.css";

export function WindowControls() {
  const win = getCurrentWindow();

  return (
    <div className="window-controls" data-tauri-drag-region>
      <button
        type="button"
        className="window-controls__btn"
        aria-label="Minimalizálás"
        onClick={() => win.minimize()}
      >
        −
      </button>
      <button
        type="button"
        className="window-controls__btn"
        aria-label="Bezárás"
        onClick={() => win.close()}
      >
        ×
      </button>
    </div>
  );
}
