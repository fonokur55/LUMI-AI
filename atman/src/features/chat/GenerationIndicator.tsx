import "./GenerationIndicator.css";

type Props = {
  visible: boolean;
  loadingModel?: string | null;
  elapsedSec: number;
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
 * v0.2.3: Random rotating "gondolkodás" feliratok.
 *
 * A "AKASHA gondolkodik..." statikus felirat helyett 3 másodpercenként
 * vált egy listából, hogy a user élőbbnek érezze az AKASHA-t (és ne
 * érezze úgy hogy "lefagyott" hosszú gondolkodás közben).
 *
 * A felirat-listát szándékosan magyar nyelvi árnyalatokkal töltöttük fel,
 * hogy AKASHA személyisége is átsejjen: nem gépi pörgést mutatunk,
 * hanem emberi "elgondolkodok" hangulatot.
 *
 * A rotációhoz az `elapsedSec`-et használjuk (osztva 3-mal és modulo-zva),
 * így determinisztikus + nincs külön timer szükséges.
 */
const THINKING_PHRASES = [
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

function rotatingThinkingPhrase(elapsedSec: number): string {
  // 3 másodpercenként váltunk, a listán körbe-körbe
  const idx = Math.floor(Math.max(0, elapsedSec) / 3) % THINKING_PHRASES.length;
  return THINKING_PHRASES[idx];
}

export function GenerationIndicator({
  visible,
  loadingModel,
  elapsedSec,
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

  return (
    <div className="gen-indicator" role="status">
      <span className="gen-indicator__pulse" aria-hidden />
      {rotatingThinkingPhrase(elapsedSec)}{" "}
      <span className="gen-indicator__time">{formatElapsed(elapsedSec)}</span>
    </div>
  );
}
