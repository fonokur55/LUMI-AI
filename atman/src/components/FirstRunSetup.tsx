import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { showToast } from "./Toast";
import "./FirstRunSetup.css";

type Props = {
  open: boolean;
  /** Igaz, ha csak a név hiányzik (a születésnap megvan); fals ha mindkettő. */
  onlyName?: boolean;
  /** Igaz, ha csak a születésnap hiányzik; fals ha mindkettő. */
  onlyBirthday?: boolean;
  /**
   * Sikeres setup után. Az `openMemory` jelzi, hogy a felhasználó az
   * onboarding végén "Igen"-t nyomott a memória-feltöltésre - ekkor a
   * szülő (App.tsx) megnyitja a MemoryNotesModal-t.
   */
  onComplete: (name: string, openMemory?: boolean) => void;
};

type Step = "name" | "birthday" | "memory" | "done";

/**
 * First-run wizard - 3 különálló modállépés:
 *
 *  1. NÉV bekérése (hero + sparkle, üdvözlő szöveg)
 *  2. SZÜLETÉSNAP bekérése (hónap + nap)
 *  3. MEMÓRIA upsell - opcionálisan ajánljuk fel a memória-feltöltést.
 *     "Igen" → onComplete(name, true) → App megnyitja a MemoryNotesModal-t
 *     "Talán később" vagy X → onComplete(name, false) → welcome lapra
 *
 * Háttér: blur. Step-váltáskor a modal kis pop+fade animációval cserélődik
 * (a `key={step}` ezt biztosítja).
 *
 * Ha a felhasználó csak az egyik adatot adta meg korábban (onlyName /
 * onlyBirthday), akkor azt a step-et kihagyjuk, de a memória upsell akkor
 * is fut a végén - "ez lenne a kis ceremónia".
 */
export function FirstRunSetup({
  open,
  onlyName,
  onlyBirthday,
  onComplete,
}: Props) {
  const askName = !onlyBirthday;
  const askBirthday = !onlyName;

  // A wizard belépő step-je: az ELSŐ még szükséges adatkérő step. Ha csak
  // a születésnap hiányzik, mindjárt onnan indulunk; ha minden hiányzik,
  // a "name"-mel kezdünk.
  const initialStep: Step = askName ? "name" : "birthday";
  const [step, setStep] = useState<Step>(initialStep);

  const [name, setName] = useState("");
  const [month, setMonth] = useState<number>(1);
  const [day, setDay] = useState<number>(1);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Új nyitás → reset
  useEffect(() => {
    if (open) {
      setStep(initialStep);
      setName("");
      setMonth(1);
      setDay(1);
      setError(null);
      setSaving(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  if (!open) return null;

  // --- Step kezelők ---

  const submitName = async () => {
    setError(null);
    const finalName = name.trim();
    if (!finalName) {
      setError("Kérlek, add meg a neved.");
      return;
    }
    setSaving(true);
    try {
      await api.profileUpdateName(finalName);
      // Tovább a következő lépésre
      if (askBirthday) {
        setStep("birthday");
      } else {
        setStep("memory");
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const submitBirthday = async () => {
    setError(null);
    setSaving(true);
    try {
      await api.profileSetBirthday(month, day);
      setStep("memory");
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const finishWith = (openMemory: boolean) => {
    if (openMemory) {
      // Csak akkor toaszt, ha mindkettő (vagy név) megtörtént - amikor
      // tényleg "befejeztük" a beállítást. Ha csak X-szel zárta a memory
      // modalt és átment a welcome-ra, a toaszt felesleges.
      // (Itt mindig megjelenik, mert ez a setup happy-path vége.)
    } else {
      showToast("Köszönöm! Üdvözöllek a LUMI-ban 🌟", "success", 3000);
    }
    setStep("done");
    onComplete(name.trim(), openMemory);
  };

  // Hónap-választó opciók
  const months = [
    "Január", "Február", "Március", "Április", "Május", "Június",
    "Július", "Augusztus", "Szeptember", "Október", "November", "December",
  ];
  const maxDay = daysInMonth(month);

  return (
    <div className="first-run-backdrop">
      {/* A key={step} biztosítja a step-váltáskor a friss bejövő animációt. */}
      <div
        key={step}
        className="first-run-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Első indítás"
      >
        {step === "memory" && (
          <button
            type="button"
            className="first-run-modal__close"
            aria-label="Bezárás"
            onClick={() => finishWith(false)}
          >
            ×
          </button>
        )}

        {/* ====== STEP 1: NÉV ====== */}
        {step === "name" && (
          <>
            <div className="first-run-modal__hero">
              <div className="first-run-modal__sparkle" aria-hidden>
                ✦
              </div>
              <h1>Üdvözöllek a LUMI-ban!</h1>
              <p>
                Egy gyors kérdéssel kezdjük, hogy AKASHA személyesen tudjon
                szólítani. Minden adat <strong>csak a te gépeden</strong>{" "}
                marad — sosem megy ki az internetre.
              </p>
            </div>

            <div className="first-run-modal__form">
              <label className="first-run-modal__field">
                <span>Hogy szólítsalak?</span>
                <input
                  type="text"
                  placeholder="pl. Áron"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  maxLength={40}
                  autoFocus
                  onKeyDown={(e) => {
                    if (e.key === "Enter") submitName();
                  }}
                />
              </label>
              {error && <p className="first-run-modal__error">{error}</p>}
            </div>

            <div className="first-run-modal__actions">
              <button
                type="button"
                className="first-run-modal__primary"
                onClick={submitName}
                disabled={saving}
              >
                {saving ? "Mentés…" : "Tovább"}
              </button>
            </div>
          </>
        )}

        {/* ====== STEP 2: SZÜLETÉSNAP ====== */}
        {step === "birthday" && (
          <>
            <div className="first-run-modal__hero first-run-modal__hero--small">
              <h1>Mikor van a születésnapod?</h1>
              <p>A születésnapodon LUMI köszönt — konfettivel. 🎉</p>
            </div>

            <div className="first-run-modal__form">
              <div className="first-run-modal__field">
                <span>Hónap és nap</span>
                <div className="first-run-modal__bday">
                  <select
                    value={month}
                    onChange={(e) => {
                      const m = Number(e.target.value);
                      setMonth(m);
                      if (day > daysInMonth(m)) setDay(daysInMonth(m));
                    }}
                    autoFocus
                  >
                    {months.map((m, i) => (
                      <option key={i} value={i + 1}>
                        {m}
                      </option>
                    ))}
                  </select>
                  <select
                    value={day}
                    onChange={(e) => setDay(Number(e.target.value))}
                  >
                    {Array.from({ length: maxDay }, (_, i) => i + 1).map(
                      (d) => (
                        <option key={d} value={d}>
                          {d}.
                        </option>
                      ),
                    )}
                  </select>
                </div>
              </div>
              {error && <p className="first-run-modal__error">{error}</p>}
            </div>

            <div className="first-run-modal__actions">
              <button
                type="button"
                className="first-run-modal__primary"
                onClick={submitBirthday}
                disabled={saving}
              >
                {saving ? "Mentés…" : "Tovább"}
              </button>
            </div>
          </>
        )}

        {/* ====== STEP 3: MEMÓRIA UPSELL ====== */}
        {step === "memory" && (
          <>
            <div className="first-run-modal__hero first-run-modal__hero--small">
              <h1>Mesélsz magadról?</h1>
              <p>
                Most opcionálisan feltölthetsz pár dolgot magadról, vagy
                megmondhatod, milyen <strong>személyiséggel</strong>{" "}
                beszéljen veled AKASHA. Bármikor megteheted később is a
                Beállítások &rsaquo; Memória menüben.
              </p>
            </div>

            <div className="first-run-modal__memory-actions">
              <button
                type="button"
                className="first-run-modal__primary first-run-modal__primary--xl"
                onClick={() => finishWith(true)}
                autoFocus
              >
                Igen, vágjunk bele
              </button>
              <button
                type="button"
                className="first-run-modal__later"
                onClick={() => finishWith(false)}
              >
                Talán később
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function daysInMonth(month: number): number {
  // Egyszerűsített: nem tudjuk az évet, ezért a hosszabbat vesszük.
  // Február: 29-et engedünk (szökőévi születéseseknek).
  const lengths = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return lengths[month - 1] ?? 31;
}
