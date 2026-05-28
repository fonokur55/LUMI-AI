import { useEffect, useState } from "react";

/**
 * Apró gondolkodás-ikon: a `gondol1.png` és `gondol2.png` képeket
 * váltogatja 1 mp-enként. Ott jelenik meg az assistant buborékban,
 * ahol egyébként a villogó kurzor lenne - amíg AKASHA még nem küldte
 * az első tokent (gondolkodik / első chunk-ra vár).
 *
 * A frame-váltáskor egy gyors cross-fade megy le a CSS-en keresztül
 * (a `key` prop a frame-mel változik, így az <img> remountolódik).
 */
type Props = {
  /** Méret pixelben - default 28. */
  size?: number;
  className?: string;
};

export function ThinkingIcon({ size = 28, className }: Props) {
  const [frame, setFrame] = useState<1 | 2>(1);
  useEffect(() => {
    const id = window.setInterval(() => {
      setFrame((f) => (f === 1 ? 2 : 1));
    }, 1000);
    return () => window.clearInterval(id);
  }, []);
  return (
    <img
      key={frame}
      src={frame === 1 ? "/brand/gondol1.png" : "/brand/gondol2.png"}
      alt=""
      aria-hidden
      draggable={false}
      width={size}
      height={size}
      className={`thinking-icon ${className ?? ""}`}
    />
  );
}
