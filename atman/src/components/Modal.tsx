import { useEffect, useState } from "react";
import "./Modal.css";

type Props = {
  open: boolean;
  title: string;
  onClose: () => void;
  children: React.ReactNode;
  maxWidth?: number;
};

/**
 * Generic Modal - fade-in és fade-out animációval. A bezárás (Escape /
 * backdrop / X) NEM egyből unmountolja a panel-t: előbb lefutik egy
 * "closing" fázis a CSS exit-animációval (~150ms), aztán szól az onClose-t
 * a szülőnek hogy állítsa `open=false`-ra. Így az eltűnés is animált.
 */
const EXIT_DURATION_MS = 160;

export function Modal({ open, title, onClose, children, maxWidth = 420 }: Props) {
  // `mounted`: tényleg a DOM-ban van-e (open=true VAGY closing fázisban)
  const [mounted, setMounted] = useState(open);
  // `closing`: most fut az exit animáció
  const [closing, setClosing] = useState(false);

  useEffect(() => {
    if (open) {
      setMounted(true);
      setClosing(false);
    } else if (mounted) {
      // A szülő open=false-ra állította → exit animáció indul, majd unmount.
      setClosing(true);
      const t = window.setTimeout(() => {
        setMounted(false);
        setClosing(false);
      }, EXIT_DURATION_MS);
      return () => window.clearTimeout(t);
    }
  }, [open, mounted]);

  useEffect(() => {
    if (!mounted || closing) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [mounted, closing, onClose]);

  if (!mounted) return null;

  return (
    <div
      className={`modal__backdrop ${closing ? "is-closing" : ""}`}
      onMouseDown={onClose}
    >
      <div
        className={`modal__panel ${closing ? "is-closing" : ""}`}
        style={{ maxWidth }}
        onMouseDown={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <header className="modal__header">
          <h2>{title}</h2>
          <button
            type="button"
            className="modal__close"
            aria-label="Bezárás"
            onClick={onClose}
          >
            ×
          </button>
        </header>
        <div className="modal__body">{children}</div>
      </div>
    </div>
  );
}
