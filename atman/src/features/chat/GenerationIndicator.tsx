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
      AKASHA gondolkodik… <span className="gen-indicator__time">{formatElapsed(elapsedSec)}</span>
    </div>
  );
}
