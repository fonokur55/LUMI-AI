import "./GenerationIndicator.css";

type Props = {
  visible: boolean;
  loadingModel?: string | null;
  elapsedSec: number;
  /** v0.2.4 Kód translation-flow fázis: "generating" / "translating" / null. */
  phase?: string | null;
};

function formatElapsed(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const m = Math.floor(total / 60);
  const s = total % 60;
  if (m > 0) {
    return `${m}:${s.toString().padStart(2, "0")}`;
  }
  return `${s}s`;
}

/**
 * v0.2.3+ Random rotating "gondolkodás" feliratok.
 *
 * A "AKASHA gondolkodik..." statikus felirat helyett 3 másodpercenként
 * vált egy listából, hogy a user élőbbnek érezze az AKASHA-t (és ne
 * érezze úgy hogy "lefagyott" hosszú gondolkodás közben).
 *
 * v0.2.4: a `phase` propon keresztül a Kód translation-flow két fázisához
 * is külön mondatlista jár: "generating" alatt általános gondolkodás-
 * feliratok, "translating" alatt fordítás-specifikus feliratok ("magyarra
 * fordítok", "csiszolom a fordítást" stb.).
 *
 * A rotációhoz az `elapsedSec`-et használjuk (osztva 3-mal és modulo-zva),
 * így determinisztikus + nincs külön timer szükséges.
 */
const THINKING_PHRASES: string[] = [
  "Gondolkodom…",
  "Ezt alaposabban szemügyre veszem…",
  "Csiszolom a választ…",
  "Eltöprengek rajta…",
  "Pillanat, formába öntöm…",
  "Átgondolom a részleteket…",
  "Még egy árnyalat…",
  "Mindjárt megvan…",
  "Hadd lássam…",
  "Egy lépéssel közelebb…",
];

const TRANSLATING_PHRASES: string[] = [
  "Magyarra fordítok…",
  "Csiszolom a magyar szöveget…",
  "Természetes formába öntöm magyarul…",
  "Még egy fordítási árnyalat…",
  "Befejezem a fordítást…",
  "Mindjárt magyarul…",
];

function rotatingPhrase(phrases: string[], elapsedSec: number): string {
  const idx = Math.floor(Math.max(0, elapsedSec) / 3) % phrases.length;
  return phrases[idx];
}

export function GenerationIndicator({
  visible,
  loadingModel,
  elapsedSec,
  phase,
}: Props) {
  if (!visible) return null;

  if (loadingModel) {
    return (
      <div className="gen-indicator" role="status">
        <span className="gen-indicator__pulse" aria-hidden />
        Modell betöltése… <span className="gen-indicator__dim">{loadingModel}</span>
      </div>
    );
  }

  // v0.2.4 - Kód translation-flow: ha "translating" fázis, fordítás-
  // specifikus feliratokat mutatunk.
  const isTranslating = phase === "translating";
  const list = isTranslating ? TRANSLATING_PHRASES : THINKING_PHRASES;

  return (
    <div className="gen-indicator" role="status">
      <span className="gen-indicator__pulse" aria-hidden />
      {rotatingPhrase(list, elapsedSec)}{" "}
      <span className="gen-indicator__time">{formatElapsed(elapsedSec)}</span>
    </div>
  );
}
