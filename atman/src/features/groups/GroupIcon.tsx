/**
 * GroupIcon - egységes csoport-ikon renderelő.
 *
 * A csoport `icon` mezője az új csoportoknál `ic1`...`ic12` (a public/icons
 * mappa PNG-jeire mutat). Régebbi csoportoknál még unicode emoji is lehet
 * (pl. "📁") - ezekre fallback-elve a <span>-be tesszük be a karaktert,
 * hogy ne törjön semmi.
 */
type Props = {
  value: string;
  /** Kontextus: a méretezést a használt CSS osztály adja meg külön. */
  size?: number;
  className?: string;
};

const IC_PATTERN = /^ic([1-9]|1[0-2])$/;

export function GroupIcon({ value, size = 18, className }: Props) {
  if (IC_PATTERN.test(value)) {
    return (
      <img
        src={`/icons/${value}.png`}
        alt=""
        width={size}
        height={size}
        className={className}
        style={{ objectFit: "contain" }}
      />
    );
  }
  // Legacy emoji fallback
  return <span className={className}>{value}</span>;
}
