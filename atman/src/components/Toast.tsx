import { useEffect, useState } from "react";
import "./Toast.css";

export type ToastKind = "success" | "info" | "error";

type ToastItem = {
  id: number;
  kind: ToastKind;
  text: string;
  /** Ha `true`, már fut az eltűnés animáció - a következő tick-ben a DOM-ból is kikerül. */
  exiting?: boolean;
};

// =====================================================================
//  Toast manager - modul-szintű singleton, hogy bármelyik komponens
//  meghívhassa egy globális helyen megjelenő üzenethez.
// =====================================================================
let _nextId = 1;
const _listeners = new Set<(items: ToastItem[]) => void>();
let _items: ToastItem[] = [];

const EXIT_DURATION_MS = 200;

function emit() {
  for (const cb of _listeners) cb([..._items]);
}

/**
 * Felugró Toast a képernyő tetején. Bárhol az appban hívható.
 * Pl.: `showToast("Mentve ✓")` zöld success toast 2 mp-re.
 */
export function showToast(
  text: string,
  kind: ToastKind = "success",
  durationMs = 2200,
) {
  const id = _nextId++;
  _items = [..._items, { id, kind, text }];
  emit();
  // Megjelölés `exiting` állapotra → CSS exit animáció lefut → tényleges törlés
  window.setTimeout(() => {
    _items = _items.map((it) => (it.id === id ? { ...it, exiting: true } : it));
    emit();
    window.setTimeout(() => {
      _items = _items.filter((it) => it.id !== id);
      emit();
    }, EXIT_DURATION_MS);
  }, durationMs);
}

/**
 * A ToastContainer komponens egyszer rendereljen az App.tsx-ben (root-szinten).
 * Innentől bármelyik komponens `showToast(...)`-tal trigger-elhet.
 */
export function ToastContainer() {
  const [items, setItems] = useState<ToastItem[]>([]);
  useEffect(() => {
    _listeners.add(setItems);
    return () => {
      _listeners.delete(setItems);
    };
  }, []);
  return (
    <div className="toast-stack" role="region" aria-label="Értesítések">
      {items.map((it) => (
        <div
          key={it.id}
          className={`toast toast--${it.kind} ${it.exiting ? "is-exiting" : ""}`}
        >
          {it.kind === "success" && (
            <span className="toast__icon" aria-hidden>
              ✓
            </span>
          )}
          {it.kind === "error" && (
            <span className="toast__icon" aria-hidden>
              !
            </span>
          )}
          <span className="toast__text">{it.text}</span>
        </div>
      ))}
    </div>
  );
}
