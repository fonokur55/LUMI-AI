import { useEffect, useRef, useState } from "react";
import type { AkashaStatus } from "../../lib/api";
import "./AkashaPill.css";

type Props = {
  status: AkashaStatus;
  activeSlot: string | null;
};

function statusDotColor(status: AkashaStatus): string {
  switch (status) {
    case "ready":
      return "var(--accent-online)";
    case "starting":
      return "var(--accent-warning)";
    case "error":
      return "var(--accent-error)";
    default:
      return "var(--text-dim)";
  }
}

function statusLabel(status: AkashaStatus): string {
  switch (status) {
    case "ready":
      return "Minden rendben - AKASHA online";
    case "starting":
      return "AKASHA indul…";
    case "error":
      return "AKASHA hiba";
    default:
      return "AKASHA leállítva";
  }
}

export function AkashaPill({ status, activeSlot }: Props) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    window.addEventListener("mousedown", onClick);
    return () => window.removeEventListener("mousedown", onClick);
  }, [open]);

  const slotLabel = activeSlot
    ? activeSlot.charAt(0).toUpperCase() + activeSlot.slice(1).toLowerCase()
    : null;

  return (
    <div className="akasha-pill" ref={rootRef}>
      <button
        type="button"
        className="akasha-pill__btn"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <span
          className="akasha-pill__dot"
          style={{ background: statusDotColor(status) }}
          aria-hidden="true"
        />
        <span className="akasha-pill__label">Akasha</span>
        <img
          src="/icons/chevron.png"
          alt=""
          width={10}
          height={10}
          className={`akasha-pill__chev ${open ? "is-open" : ""}`}
        />
      </button>

      {open && (
        <div className="akasha-pill__popover" role="menu">
          <div className="akasha-pill__popover-header">
            <span
              className="akasha-pill__dot"
              style={{ background: statusDotColor(status) }}
              aria-hidden="true"
            />
            <span>{statusLabel(status)}</span>
          </div>
          {slotLabel && (
            <div className="akasha-pill__popover-row">
              <span className="akasha-pill__popover-key">Aktív mód</span>
              <span className="akasha-pill__popover-val">{slotLabel}</span>
            </div>
          )}
          <div className="akasha-pill__popover-foot">
            Később ide érkeznek az új AKASHA módok és preset-ek.
          </div>
        </div>
      )}
    </div>
  );
}
