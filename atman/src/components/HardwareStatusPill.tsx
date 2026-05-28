import { useEffect, useState } from "react";
import { api, type HardwareProfile, type PerfTier } from "../lib/api";
import "./HardwareStatusPill.css";

/**
 * Apró pill a titlebar-on, ami mutatja az aktuális tier-szintet és a
 * szabad RAM-ot. 5 mp-enként frissül. Kattintásra → Settings (de azt
 * a parent kezeli, itt csak megjelenítjük).
 */
type Props = {
  onClick?: () => void;
};

export function HardwareStatusPill({ onClick }: Props) {
  const [profile, setProfile] = useState<HardwareProfile | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const p = await api.getHardwareProfile();
        if (!cancelled) setProfile(p);
      } catch (e) {
        console.error("hw profile failed", e);
      }
    };
    load();
    const id = window.setInterval(load, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, []);

  if (!profile) return null;

  // RAM-küszöb szerinti szín-szint, FÜGGETLENÜL a tier-től:
  //  - critical (<3 GB szabad): piros pulzáló
  //  - warning  (3-6 GB szabad): sárga
  //  - ok       (>6 GB): tier-szerinti normál szín
  const ramLevel: "critical" | "warning" | "ok" =
    profile.availableRamGb < 3
      ? "critical"
      : profile.availableRamGb < 6
        ? "warning"
        : "ok";

  return (
    <button
      type="button"
      className={`hw-pill hw-pill--${profile.effectiveTier} hw-pill--ram-${ramLevel}`}
      onClick={onClick}
      title={`${tierLabel(profile.effectiveTier)} · ${profile.availableRamGb.toFixed(1)} GB szabad - kattints a részletekért`}
    >
      <span className="hw-pill__dot" aria-hidden />
      <span className="hw-pill__text">
        {profile.availableRamGb.toFixed(1)} GB
      </span>
    </button>
  );
}

function tierLabel(t: PerfTier): string {
  switch (t) {
    case "blocked":
      return "Blokkolva";
    case "limp":
      return "Light mód";
    case "standard":
      return "Standard";
    case "pro":
      return "Pro mód";
  }
}
