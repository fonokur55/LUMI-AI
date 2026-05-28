import { useEffect, useRef, useState } from "react";
import confetti from "canvas-confetti";
import { AppShell } from "./app/AppShell";
import { FirstRunSetup } from "./components/FirstRunSetup";
import { ToastContainer } from "./components/Toast";
import { UpdateBanner } from "./components/UpdateBanner";
import { MemoryNotesModal } from "./features/memory/MemoryNotesModal";
import { api } from "./lib/api";

export default function App() {
  const [displayName, setDisplayName] = useState("");
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);
  const [theme, setTheme] = useState<string>("light");
  const [ready, setReady] = useState(false);
  // First-run modal állapot
  const [setupOpen, setSetupOpen] = useState(false);
  const [setupOnlyName, setSetupOnlyName] = useState(false);
  const [setupOnlyBirthday, setSetupOnlyBirthday] = useState(false);
  // Születésnap-cím (ha ma van a user szülinapja)
  const [birthdayGreeting, setBirthdayGreeting] = useState<string | null>(null);
  const birthdayCheckedRef = useRef(false);
  // A first-run wizard 3. step-jén ("Mesélsz magadról?") "Igen" → ez a
  // state nyitja meg a memória-modalt onboarding közvetlen folytatásaként.
  const [memoryFromOnboarding, setMemoryFromOnboarding] = useState(false);

  // Avatar (re)frissítés - a ProfileView ezt hívja meg mentés/törlés után,
  // hogy a sidebar avatarja is azonnal kövesse a változást.
  const refreshAvatar = async () => {
    try {
      const url = await api.profileGetAvatarDataUrl();
      setAvatarUrl(url);
    } catch (e) {
      console.error("avatar refresh failed", e);
    }
  };

  // Globális drag-letiltás: a felhasználó NE tudjon az appból kihúzni
  // képet/ikont (sem fájlrendszerre, sem másik ablakba, sem mentésre).
  // Plusz: ha valaki kívülről RÁHUZ valamit az ablakra, a böngésző alap-
  // viselkedése a fájl megnyitása lenne - ezt is letiltjuk, hogy ne
  // kerülje meg a saját file-picker UI-t.
  useEffect(() => {
    const blockImgDrag = (e: DragEvent) => {
      const t = e.target as HTMLElement | null;
      if (!t) return;
      const tag = t.tagName;
      if (tag === "IMG" || tag === "PICTURE" || tag === "SVG" || tag === "CANVAS") {
        e.preventDefault();
      }
    };
    const blockWindowDrop = (e: DragEvent) => {
      // Külső fájl-drop az ablakon - a böngésző különben megnyitná.
      e.preventDefault();
    };
    window.addEventListener("dragstart", blockImgDrag, { capture: true });
    window.addEventListener("dragover", blockWindowDrop);
    window.addEventListener("drop", blockWindowDrop);
    return () => {
      window.removeEventListener("dragstart", blockImgDrag, { capture: true } as EventListenerOptions);
      window.removeEventListener("dragover", blockWindowDrop);
      window.removeEventListener("drop", blockWindowDrop);
    };
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const profile = await api.profileGet();
        setDisplayName(profile.displayName ?? "");
      } catch (e) {
        console.error("profile load failed", e);
      }

      // Téma alkalmazása a <html> tagre, hogy a CSS [data-theme="light"]
      // szabályok azonnal érvényesüljenek. Ezt React state-be is tartjuk,
      // hogy a téma-függő elemek (pl. sidebar logo) követhessék.
      try {
        const cfg = await api.config();
        const t = cfg.appearance?.theme ?? "light";
        document.documentElement.setAttribute("data-theme", t);
        setTheme(t);
      } catch (e) {
        console.error("theme load failed", e);
      }
      // A `<html data-theme>` változására figyelünk - a Settings-ben
      // változtatás után frissítjük a state-et, hogy a téma-függő
      // elemek (sidebar logo) is azonnal lekövessék.
      const observer = new MutationObserver(() => {
        setTheme(document.documentElement.dataset.theme ?? "light");
      });
      observer.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["data-theme"],
      });
      // Cleanup nem szükséges, a komponens élete = app élete.

      // Avatar első betöltés
      await refreshAvatar();

      // First-run wizard check: ha hiányzik a név vagy a születésnap, modal
      try {
        const status = await api.profileGetSetupStatus();
        if (!status.hasName || !status.hasBirthday) {
          setSetupOnlyName(!status.hasName && status.hasBirthday);
          setSetupOnlyBirthday(status.hasName && !status.hasBirthday);
          setSetupOpen(true);
        } else {
          // A wizard kihagyva → ellenőrizzük rögtön a szülinapot
          checkBirthdayAndCelebrate();
        }
      } catch (e) {
        console.error("setup status failed", e);
      }

      setReady(true);
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // A szülinap-vizsgálatot egy külön függvénybe szervezzük, hogy a first-run
  // bezárása után is meg tudjuk hívni (ha a user pont MA született).
  const checkBirthdayAndCelebrate = async () => {
    if (birthdayCheckedRef.current) return;
    birthdayCheckedRef.current = true;
    try {
      const check = await api.profileCheckBirthday();
      if (check.needsGreeting) {
        const name = (check.displayName ?? "").trim();
        const greetings = name
          ? [`Boldog Születésnapot ${name}!`, `Isten éltessen ${name}!`]
          : ["Boldog Születésnapot!", "Isten éltessen!"];
        const greeting = greetings[Math.floor(Math.random() * greetings.length)];
        setBirthdayGreeting(greeting);
        // Konfetti - több burst egymás után
        runConfetti();
        await api.profileMarkBirthdayGreeted();
      }
    } catch (e) {
      console.error("birthday check failed", e);
    }
  };

  if (!ready) {
    return (
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          height: "100%",
        }}
      >
        LUMI…
      </div>
    );
  }

  return (
    <>
      <ToastContainer />
      <UpdateBanner />
      <FirstRunSetup
        open={setupOpen}
        onlyName={setupOnlyName}
        onlyBirthday={setupOnlyBirthday}
        onComplete={(name, openMemory) => {
          if (name) setDisplayName(name);
          setSetupOpen(false);
          if (openMemory) {
            // "Igen, vágjunk bele" → folytatás a memória-feltöltővel.
            // A születésnap-konfettit ilyenkor halasztjuk, amíg a memória-
            // modal bezárul, hogy ne ütközzön a felhasználói flow-val.
            setMemoryFromOnboarding(true);
          } else {
            // "Talán később" / X → egyenesen a welcome lapra; szülinap-check
            // ha pont ma van.
            checkBirthdayAndCelebrate();
          }
        }}
      />
      <MemoryNotesModal
        open={memoryFromOnboarding}
        onClose={() => {
          setMemoryFromOnboarding(false);
          // A modal bezárása után - akár írt valamit, akár nem - jöhet a
          // szülinap-check (ha pont ma van a user szülinapja).
          checkBirthdayAndCelebrate();
        }}
      />
      <AppShell
        displayName={displayName}
        onNameChange={setDisplayName}
        avatarUrl={avatarUrl}
        onAvatarChange={refreshAvatar}
        birthdayGreeting={birthdayGreeting}
        theme={theme}
      />
    </>
  );
}

/**
 * Confetti burst - visszafogott, kék + fehér paletta.
 * Egyetlen központi burst + rövid oldalsó szivárgás. Nem zavarja a UI-t.
 */
function runConfetti() {
  // Csak kék árnyalatok + fehér - visszafogott, NOMAD-stílusos
  const colors = ["#3b82f6", "#60a5fa", "#93c5fd", "#ffffff"];

  // 1) Központi közepes burst (60 részecske - előtte 120 volt)
  confetti({
    particleCount: 60,
    spread: 80,
    origin: { y: 0.55 },
    colors,
    disableForReducedMotion: true,
  });

  // 2) Rövid oldalsó szivárgás - 1.2 mp, 2-2 részecske/frame
  const duration = 1200;
  const end = Date.now() + duration;
  (function frame() {
    confetti({
      particleCount: 2,
      angle: 60,
      spread: 55,
      origin: { x: 0, y: 0.75 },
      colors,
      disableForReducedMotion: true,
    });
    confetti({
      particleCount: 2,
      angle: 120,
      spread: 55,
      origin: { x: 1, y: 0.75 },
      colors,
      disableForReducedMotion: true,
    });
    if (Date.now() < end) {
      requestAnimationFrame(frame);
    }
  })();
}
