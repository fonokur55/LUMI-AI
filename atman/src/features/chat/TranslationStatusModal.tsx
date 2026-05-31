import { useEffect, useState } from "react";
import "./TranslationStatusModal.css";

/**
 * v0.2.6 - TranslationStatusModal
 *
 * A Kód mód translation-flow átmeneti pillanataira jelenik meg:
 * a Coder befejezte az angol választ, a Gemma 2B még betöltődik
 * vagy fordít, és még nincs első magyar token. Ez a "2-3 mp üres
 * képernyő"-időszak megijesztheti a felhasználót ("lefagyott?
 * kattintsak ide-oda?"), ezért egy diszkrét lebegő doboz mutatja
 * hogy AKASHA dolgozik.
 *
 * Mikor látszik:
 *   `phase === "translating"` ÉS a magyar válasz még nem
 *   kezdődött el a buborékban (streamBuf üres).
 *
 * Mikor tűnik el:
 *   Amint az első magyar chunk megérkezik (streamBuf nem üres) —
 *   ettől a pillanattól a buborék veszi át a vizuális szerepet.
 */

type Props = {
  visible: boolean;
};

const REVIEW_PHRASES = [
  "AKASHA átnézi a választ, hogy minden rendben van-e…",
  "Egy pillanat, csiszolom a magyar fordítást…",
  "Még egy utolsó simítás…",
  "Mindjárt kész — átnézem az utolsó részleteket…",
  "Befejezem a válaszodat…",
];

export function TranslationStatusModal({ visible }: Props) {
  const [phraseIdx, setPhraseIdx] = useState(0);

  useEffect(() => {
    if (!visible) {
      setPhraseIdx(0);
      return;
    }
    // 2.5 mp-enként rotálunk a feliratokon
    const tick = window.setInterval(() => {
      setPhraseIdx((i) => (i + 1) % REVIEW_PHRASES.length);
    }, 2500);
    return () => window.clearInterval(tick);
  }, [visible]);

  if (!visible) return null;

  return (
    <div className="translation-modal" role="status" aria-live="polite">
      <div className="translation-modal__inner">
        <div className="translation-modal__icon" aria-hidden>
          <img
            src="/brand/logo.png"
            alt=""
            className="translation-modal__logo"
            draggable={false}
          />
          <span className="translation-modal__sparkle" aria-hidden>
            ✨
          </span>
        </div>
        <div className="translation-modal__body">
          <div className="translation-modal__title">
            Egy pillanat, kész is van…
          </div>
          <div className="translation-modal__subtitle">
            {REVIEW_PHRASES[phraseIdx]}
          </div>
        </div>
      </div>
    </div>
  );
}
