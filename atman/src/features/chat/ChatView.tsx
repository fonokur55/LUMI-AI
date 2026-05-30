import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  clearAkashaStreamHandlers,
  setAkashaStreamHandlers,
  type ChatMessage,
  type SetupStatus,
  type SlotChoice,
} from "../../lib/api";
import { GenerationIndicator } from "./GenerationIndicator";
import { CopyButton, MarkdownMessage } from "./MarkdownMessage";
import { ThinkingIcon } from "./ThinkingIcon";
import "./ChatView.css";

type Props = {
  chatId: string;
  initialTitle: string;
  displayName: string;
  /** Ha nem null/üres, ma a user szülinapja → ezt mutatjuk a welcome címben. */
  birthdayGreeting?: string | null;
  onPersisted: (title: string) => void;
};

type UiMessage = ChatMessage & { id: string };

type Attachment = {
  id: string;
  name: string;
  path: string;
};

function newId() {
  return crypto.randomUUID();
}

function basename(p: string): string {
  const norm = p.replace(/\\/g, "/");
  const idx = norm.lastIndexOf("/");
  return idx >= 0 ? norm.slice(idx + 1) : norm;
}

function deriveTitle(messages: UiMessage[]): string {
  const firstUser = messages.find((m) => m.role === "user");
  if (!firstUser) return "Új beszélgetés";
  const text = firstUser.content.trim();
  if (!text) return "Új beszélgetés";
  const sliced = Array.from(text).slice(0, 40).join("");
  return Array.from(text).length > 40 ? `${sliced}…` : sliced;
}

export function ChatView({
  chatId,
  initialTitle,
  displayName,
  birthdayGreeting,
  onPersisted,
}: Props) {
  const [messages, setMessages] = useState<UiMessage[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [streamBuf, setStreamBuf] = useState("");
  // Web-search toggle: ha aktív, az `akasha_chat` előtt DDG keresést
  // futtat a backend és az eredményeket a system promptba injektálja.
  // Alapból off → az app 100% offline marad.
  const [useWeb, setUseWeb] = useState(false);
  const [webStatus, setWebStatus] = useState<string | null>(null);

  // Manuális slot-választó: "auto" = a backend router dönt prompt-keyword
  // alapján; "eco"/"brain"/"creative" = mindig a megadott slot/modell.
  const [slotChoice, setSlotChoice] = useState<SlotChoice>("auto");
  const [slotMenuOpen, setSlotMenuOpen] = useState(false);
  const slotMenuRef = useRef<HTMLDivElement>(null);
  // Telepített modellek - a hiányzókat a slot-menüben disabled-é tesszük
  const [setupStatus, setSetupStatus] = useState<SetupStatus | null>(null);

  // Küldés-animáció flag - rövid kék gradient-hullám + glow a composer
  // körül, jelezve hogy elindult a kérés. Auto-feloldódik 1.5 mp után.
  const [sendingPulse, setSendingPulse] = useState(false);
  // Külön buffer a reasoning-modellek "gondolkodási" tokenjeinek.
  const [thinkingBuf, setThinkingBuf] = useState("");
  const [thinkingOpen, setThinkingOpen] = useState(true);
  const [useMemory] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [loadingModel, setLoadingModel] = useState<string | null>(null);
  const [elapsedSec, setElapsedSec] = useState(0);
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [historyLoaded, setHistoryLoaded] = useState(false);

  // Streaming-mascot: a fő logó a chatbox jobb felső sarkán "üldögél",
  // amint AKASHA elkezdte kiírni a választ ÉS attól kezdve végig - akkor
  // is amikor stream véget ér és csak nézegeti a user a kész szöveget.
  // Eltűnik viszont:
  //   - welcome állapotban (még semmilyen üzenet sincs)
  //   - a "gondolkodás" fázis alatt (streaming=true, de streamBuf üres) -
  //     ekkor a buborékban lévő gondol1/2 ikon vesz át.
  // Pislogás: 2.5 mp-enként rövid időre (~180ms) pislog.png-re vált.
  const [mascotBlinking, setMascotBlinking] = useState(false);
  const hasAssistantOutput =
    messages.some((m) => m.role === "assistant") || streamBuf.length > 0;
  // A "gondolkodás" fázis: streamelünk de még semmi szöveg nincs - a
  // gondol1/2 ikon él a buborékban, a mascot legyen kikapcsolva.
  const isThinkingPhase = streaming && streamBuf.length === 0;
  const showMascot = hasAssistantOutput && !isThinkingPhase;
  useEffect(() => {
    if (!showMascot) {
      setMascotBlinking(false);
      return;
    }
    // 2.5 mp-enként egy pillanatra pislog. A blink-flag csak ~180ms
    // tart - utána visszaáll a sima logóra.
    const cycle = window.setInterval(() => {
      setMascotBlinking(true);
      // Gyors pislogás - kb. mint egy ember rebegő szemhéja (~90 ms).
      window.setTimeout(() => setMascotBlinking(false), 90);
    }, 2500);
    return () => window.clearInterval(cycle);
  }, [showMascot]);

  // Welcome logo "pattog 2-t" + boldog_pattog.png csere kattintásra.
  // A `bouncing` ideje alatt a CSS animáció 2× lepattanik (1.2 mp össz),
  // és a logó-src átvált a boldog változatra. Onnan ne lehessen újra
  // triggerelni amíg le nem futott - dupla-klikk védve.
  const [logoBouncing, setLogoBouncing] = useState(false);
  const bounceLogo = () => {
    if (logoBouncing) return;
    setLogoBouncing(true);
    window.setTimeout(() => setLogoBouncing(false), 1200);
  };

  const bottomRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const startTimeRef = useRef<number | null>(null);
  // Refek a streamBuf/thinkingBuf "élő" tartalmához. Azért kellenek, mert
  // React StrictMode dev-ben a useState funkcionális updater-eket KÉTSZER
  // hívja meg (mellékhatás-bug-felfedezés céljából). Ha a setMessages
  // mellékhatást egy useState updater BELSEJÉBEN végeznénk (mint régen),
  // egymásba ágyazott updaterek esetén 2×2 = 4 másolat kerülne a chatbe.
  // A refek mentesek ettől - a `done` handler innen olvas és csak EGYSZER
  // hívja setMessages-t.
  const streamBufRef = useRef("");
  const thinkingBufRef = useRef("");

  const currentTitle = useMemo(() => {
    // Ha a sidebar-ból érkezett initialTitle nem az alapértelmezett
    // "Új beszélgetés", akkor MINDIG azt használjuk (átnevezett cím,
    // vagy persistálás után az auto-derived cím). Csak akkor esünk vissza
    // a deriveTitle-re, ha még friss az új chat és nem volt mentés.
    if (initialTitle && initialTitle !== "Új beszélgetés") {
      return initialTitle;
    }
    return messages.length > 0 ? deriveTitle(messages) : "";
  }, [messages, initialTitle]);

  // Beszélgetés betöltése a backend-ből, ha létezik
  useEffect(() => {
    let cancelled = false;
    setHistoryLoaded(false);
    api
      .chatGet(chatId)
      .then((full) => {
        if (cancelled) return;
        setMessages(
          full.messages.map((m) => ({
            id: m.id,
            role: m.role,
            content: m.content,
          })),
        );
      })
      .catch(() => {
        if (!cancelled) setMessages([]);
      })
      .finally(() => {
        if (!cancelled) setHistoryLoaded(true);
      });
    return () => {
      cancelled = true;
    };
  }, [chatId]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamBuf]);

  // AKASHA stream-handlerek a GLOBÁLIS, egy-példányú Tauri listener-en
  // keresztül (lib/api.ts `setAkashaStreamHandlers`). Így bármennyiszer
  // is mountolódik újra a ChatView (StrictMode dupla mount, chatId váltás
  // miatti remount, HMR), a token-eseményeknek csak EGY rendezvénylánca
  // van - nincs többé "MaMaMaMa" vagy 4× ismételt teljes válasz.
  useEffect(() => {
    setAkashaStreamHandlers({
      onToken: (token) => {
        // Ref a forrás-tárolóhoz (mellékhatás-mentes, StrictMode-safe).
        // Az állapot csak megjelenítés-célból frissül - de a setMessages
        // a `done` handler-ben a ref-ből olvas, nem ebből.
        streamBufRef.current += token;
        setStreamBuf(streamBufRef.current);
      },
      onThinkingToken: (token) => {
        thinkingBufRef.current += token;
        setThinkingBuf(thinkingBufRef.current);
      },
      onDone: () => {
        // FONTOS - refekből olvasunk, NEM a setStreamBuf/setThinkingBuf
        // funkcionális updaterekből. StrictMode dev-ben a useState
        // updaterek kétszer futnak (mellékhatás-felfedezés céljából);
        // ha a setMessages mellékhatás egy updater BELSEJÉBEN lenne,
        // egymásba ágyazott setStreamBuf → setThinkingBuf → setMessages
        // hívás 2×2 = 4 másolatot eredményezne a chatben.
        const buf = streamBufRef.current;
        const think = thinkingBufRef.current;
        const finalContent =
          buf.trim().length > 0
            ? buf
            : think.trim().length > 0
              ? `_(csak gondolkodás)_\n\n${think}`
              : "";
        if (finalContent) {
          setMessages((m) => [
            ...m,
            { id: newId(), role: "assistant", content: finalContent },
          ]);
        }
        // Refek és state nullázása (egyszerű setterekkel - nincs side-effect benne)
        streamBufRef.current = "";
        thinkingBufRef.current = "";
        setStreamBuf("");
        setThinkingBuf("");
        setStreaming(false);
        setLoadingModel(null);
        startTimeRef.current = null;
        setElapsedSec(0);
      },
      onError: (e) => {
        setError(e);
        setStreaming(false);
        setLoadingModel(null);
        streamBufRef.current = "";
        thinkingBufRef.current = "";
        setStreamBuf("");
        setThinkingBuf("");
        startTimeRef.current = null;
        setElapsedSec(0);
      },
      onModelLoading: (id) => setLoadingModel(id),
      onModelReady: () => setLoadingModel(null),
      onWebSearching: (q) =>
        setWebStatus(`🌐 Internet keresés: „${q.slice(0, 60)}${q.length > 60 ? "…" : ""}"`),
      onWebResults: (results) => {
        if (results.length === 0) {
          setWebStatus("🌐 Nincs találat a webről");
        } else {
          setWebStatus(`🌐 ${results.length} találat a webről`);
        }
        // 3 mp múlva eltűnik a státusz, addig is megy a stream
        window.setTimeout(() => setWebStatus(null), 3000);
      },
      onWebError: (msg) => {
        setWebStatus(`🌐 Web hiba: ${msg}`);
        window.setTimeout(() => setWebStatus(null), 4000);
      },
    });

    return () => {
      clearAkashaStreamHandlers();
    };
  }, []);

  // Egyszerű, frontend-vezérelt felfelé számláló - pontosan akkor fut,
  // amíg a streaming aktív (a backend ETA-becsléseit nem használjuk).
  useEffect(() => {
    if (!streaming) return;
    const id = window.setInterval(() => {
      if (startTimeRef.current == null) return;
      setElapsedSec((Date.now() - startTimeRef.current) / 1000);
    }, 100);
    return () => window.clearInterval(id);
  }, [streaming]);

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }, [input]);

  // Telepített modellek lekérése - induláskor + amikor a slot-menüt
  // megnyitja a user (hátha közben a Beállításokból letöltött egyet).
  useEffect(() => {
    api.checkSetupStatus().then(setSetupStatus).catch(() => {});
  }, []);
  useEffect(() => {
    if (slotMenuOpen) {
      api.checkSetupStatus().then(setSetupStatus).catch(() => {});
    }
  }, [slotMenuOpen]);

  // Slot-menü bezárása ha a felhasználó máshova kattint.
  useEffect(() => {
    if (!slotMenuOpen) return;
    const onClick = (e: MouseEvent) => {
      if (
        slotMenuRef.current &&
        !slotMenuRef.current.contains(e.target as Node)
      ) {
        setSlotMenuOpen(false);
      }
    };
    window.addEventListener("mousedown", onClick);
    return () => window.removeEventListener("mousedown", onClick);
  }, [slotMenuOpen]);

  const pickAttachment = async () => {
    try {
      const picked = await open({
        multiple: true,
        filters: [
          {
            name: "Kép vagy dokumentum",
            extensions: [
              "png", "jpg", "jpeg", "gif", "webp",
              "pdf", "txt", "md", "csv", "json",
              "rs", "ts", "tsx", "js", "py",
            ],
          },
        ],
      });
      if (!picked) return;
      const arr = Array.isArray(picked) ? picked : [picked];
      const next: Attachment[] = arr
        .filter((p): p is string => typeof p === "string")
        .map((p) => ({ id: newId(), name: basename(p), path: p }));
      setAttachments((prev) => [...prev, ...next]);
    } catch (e) {
      setError(String(e));
    }
  };

  const removeAttachment = (id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id));
  };

  const stopGeneration = async () => {
    try {
      await api.akashaCancelGeneration();
    } catch (e) {
      console.error("[chat] cancel failed", e);
    }
  };

  const send = async () => {
    const text = input.trim();
    if ((!text && attachments.length === 0) || streaming) return;
    setError(null);

    const composedText = attachments.length
      ? `${text}\n\n[Csatolva: ${attachments.map((a) => a.name).join(", ")}]`
      : text;

    const userMsg: UiMessage = {
      id: newId(),
      role: "user",
      content: composedText,
    };
    const next = [...messages, userMsg];
    setMessages(next);
    setInput("");
    setAttachments([]);
    setStreaming(true);
    // Rövid (1.5 mp) kék-pulzáló animáció a composer körül - vizuális
    // jelzés hogy elindult a kérés. Auto-feloldódik.
    setSendingPulse(true);
    window.setTimeout(() => setSendingPulse(false), 1500);
    // FONTOS: a refeket IS nulláznunk kell - különben az előző válasz
    // szövege ott marad benne és a következő `done` arra rácsapna.
    streamBufRef.current = "";
    thinkingBufRef.current = "";
    setStreamBuf("");
    setThinkingBuf("");
    setThinkingOpen(true);
    startTimeRef.current = Date.now();
    setElapsedSec(0);

    const title = deriveTitle(next);

    // FONTOS: nem hívjuk meg külön az `akasha_start`-ot a frontendből -
    // az `akasha_chat` parancs MAGA gondoskodik AKASHA elindításáról
    // (ensure_akasha_running). Az `akasha_start` belsőleg meghívja a
    // `akasha_status`-t is, ami egy lassú, mutex-elt hardware.snapshot()-tal
    // járó hívás - ez korábban beragasztotta a flow-t, mert a JS await
    // a status-választ várta és nem ment tovább a chat hívásra.
    console.log("[chat] send -> akasha_chat", {
      messages: next.length,
      useMemory,
      chatId,
    });
    try {
      await api.akashaChat(
        next.map(({ role, content }) => ({ role, content })),
        useMemory,
        { chatId, chatTitle: title, useWeb, forceSlot: slotChoice },
      );
      console.log("[chat] akasha_chat resolved OK");
      onPersisted(title);
    } catch (e) {
      console.error("[chat] akasha_chat FAILED:", e);
      setError(String(e));
      setStreaming(false);
      setLoadingModel(null);
      startTimeRef.current = null;
      setElapsedSec(0);
    }
  };

  const showWelcome =
    historyLoaded && messages.length === 0 && !streaming;
  // A "gondolkodik" indikátor MINDIG látszik amíg streamelünk -
  // még a legelső token előtt is, hogy a user biztosan lássa: dolgozik.
  const showGenIndicator = streaming;
  // Amint az első token megjön, jelenik meg az asszisztens buborék
  // (streamBuf-fel vagy üresen + villogó kurzorral).
  const showAssistantBubble = streaming;

  const canSend =
    !streaming && (input.trim().length > 0 || attachments.length > 0);

  // Welcome szöveg-blokk - kétféle helyen rendereljük (welcome stage vagy
  // a scroll területen) ezért külön változóként.
  const welcomeNode = (
    <div className="chat-view__welcome">
      <img
        src={logoBouncing ? "/brand/boldog_pattog.png" : "/brand/logo.png"}
        alt=""
        className={`chat-view__welcome-logo ${logoBouncing ? "is-bouncing" : ""}`}
        aria-hidden
        onClick={bounceLogo}
        draggable={false}
      />
      <h1>
        {birthdayGreeting ? (
          <span className="chat-view__welcome-bday">{birthdayGreeting} 🎉</span>
        ) : displayName.trim() ? (
          <>
            Szia{" "}
            <span className="chat-view__welcome-name">{displayName}</span>
            !
          </>
        ) : (
          <>Szia! Hol kezdjük?</>
        )}
      </h1>
      {(birthdayGreeting || displayName.trim()) && <p>Hol kezdjük?</p>}
    </div>
  );

  return (
    <div className={`chat-view ${showWelcome ? "chat-view--welcome" : ""}`}>
      {messages.length > 0 && currentTitle && (
        <div className="chat-view__title">{currentTitle}</div>
      )}

      {/* Welcome stage: csak a kezdőlapon (üres chat) - a welcome + composer
          egy középre igazított csoportként jelenik meg, hogy ne tátongjon
          üres tér a chatbox felett. Amint valamilyen üzenet jön, vissza-
          váltunk normál layout-ra: scroll fent, composer alul. */}
      {showWelcome ? (
        <div className="chat-view__center-stage">{welcomeNode}</div>
      ) : null}

      <div className="chat-view__scroll" hidden={showWelcome}>
        {messages.map((m) => (
          <div key={m.id} className={`chat-msg chat-msg--${m.role}`}>
            <div className="chat-msg__bubble">
              {m.role === "assistant" ? (
                <MarkdownMessage content={m.content} />
              ) : (
                m.content
              )}
            </div>
            {m.role === "assistant" && (
              <MessageCopyButton text={m.content} />
            )}
          </div>
        ))}

        {showAssistantBubble && (
          <div className="chat-msg chat-msg--assistant">
            {thinkingBuf && (
              <details
                className="chat-msg__thinking"
                open={thinkingOpen}
                onToggle={(e) =>
                  setThinkingOpen((e.target as HTMLDetailsElement).open)
                }
              >
                <summary className="chat-msg__thinking-summary">
                  <span className="chat-msg__thinking-icon" aria-hidden>
                    ✦
                  </span>
                  Gondolkodás
                  {streamBuf.length === 0 && (
                    <span className="chat-msg__thinking-dots" aria-hidden>
                      <span />
                      <span />
                      <span />
                    </span>
                  )}
                </summary>
                <div className="chat-msg__thinking-body">{thinkingBuf}</div>
              </details>
            )}
            <div className="chat-msg__bubble">
              {streamBuf && <MarkdownMessage content={streamBuf} />}
              {/* Két állapot - sosem egyszerre:
                  1) streamBuf > 0 → klasszikus villogó kurzor a szöveg végén
                  2) streamBuf üres ÉS nincs thinking-panel → gondol1/2 ikon
                     ott ahol a kurzor villogna (AKASHA "gondolkodik" jel) */}
              {streamBuf.length > 0 ? (
                <span className="chat-msg__caret" aria-hidden />
              ) : !thinkingBuf ? (
                <ThinkingIcon size={56} className="chat-msg__thinking-img" />
              ) : null}
            </div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      {error && (
        <div className="chat-view__error" role="alert">
          <span>{error}</span>
          <button
            type="button"
            className="chat-view__error-close"
            aria-label="Bezárás"
            onClick={() => setError(null)}
          >
            ×
          </button>
        </div>
      )}

      <div
        className={`chat-view__composer-wrap ${sendingPulse ? "is-sending" : ""}`}
      >
        {/* Streaming mascot - a chatbox (composer) jobb-felső peremén
            "üldögél", amíg/miután AKASHA írja a választ. Kissé túllóg
            a doboz tetején, mintha a tetején ülne. 2.5 mp-enként
            pislog.png-re vált pár tized mp-re, aztán visszaáll. */}
        {showMascot && (
          <img
            src={mascotBlinking ? "/brand/pislog.png" : "/brand/logo.png"}
            alt=""
            aria-hidden
            draggable={false}
            className={`chat-view__mascot ${mascotBlinking ? "is-blinking" : ""}`}
          />
        )}
        {webStatus && (
          <div className="chat-view__web-status" role="status">
            {webStatus}
          </div>
        )}
        <GenerationIndicator
          visible={showGenIndicator}
          loadingModel={loadingModel}
          elapsedSec={elapsedSec}
        />

        <div className="chat-view__composer">
          {attachments.length > 0 && (
            <div className="chat-view__attachments">
              {attachments.map((a) => (
                <div key={a.id} className="chat-view__chip">
                  <span className="chat-view__chip-name">{a.name}</span>
                  <button
                    type="button"
                    className="chat-view__chip-remove"
                    aria-label="Eltávolítás"
                    onClick={() => removeAttachment(a.id)}
                  >
                    ×
                  </button>
                </div>
              ))}
            </div>
          )}

          <textarea
            ref={textareaRef}
            className="chat-view__input"
            placeholder="Kérdezz bármit..."
            rows={1}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send();
              }
            }}
            disabled={streaming}
          />

          <div className="chat-view__composer-bar">
            <button
              type="button"
              className="chat-view__attach"
              aria-label="Melléklet hozzáadása"
              onClick={pickAttachment}
              disabled={streaming}
            >
              <img src="/icons/plus.png" alt="" width={16} height={16} />
            </button>

            <button
              type="button"
              className={`chat-view__web ${useWeb ? "is-active" : ""}`}
              aria-label={
                useWeb
                  ? "Internet keresés bekapcsolva - kattints a kikapcsoláshoz"
                  : "Internet keresés bekapcsolása"
              }
              title={
                useWeb
                  ? "Internet mód: AKASHA most lekérdezhet friss webes információt. Kattints a kikapcsoláshoz."
                  : "Internet mód: be ↔ ki. Bekapcsolva a backend DuckDuckGo-n lekérdez és a találatokat a kérdéshez fűzi."
              }
              onClick={() => setUseWeb((v) => !v)}
              disabled={streaming}
            >
              <GlobeIcon active={useWeb} />
            </button>

            {/* AKASHA modell-slot választó */}
            <div className="chat-view__slot" ref={slotMenuRef}>
              <button
                type="button"
                className={`chat-view__slot-btn ${
                  slotChoice !== "auto" ? "is-manual" : ""
                }`}
                aria-haspopup="menu"
                aria-expanded={slotMenuOpen}
                aria-label="AKASHA modell-mód választó"
                title={
                  slotChoice === "auto"
                    ? "AKASHA modell: AUTO (a router választ a kérdés alapján). Kattints másikra."
                    : `AKASHA modell: ${slotLabel(slotChoice)} (manuálisan rögzítve). Kattints AUTO-ra a visszaállításhoz.`
                }
                onClick={() => setSlotMenuOpen((v) => !v)}
                disabled={streaming}
              >
                <span className="chat-view__slot-dot" aria-hidden />
                <span className="chat-view__slot-label">
                  {slotChoice === "auto" ? "AUTO" : slotLabel(slotChoice)}
                </span>
                <svg
                  className={`chat-view__slot-chev ${slotMenuOpen ? "is-open" : ""}`}
                  width="10"
                  height="10"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  aria-hidden
                >
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>

              {slotMenuOpen && (
                <div className="chat-view__slot-menu" role="menu">
                  {(
                    [
                      {
                        v: "auto",
                        title: "AUTO",
                        desc: "A router választ a kérdés alapján",
                      },
                      // v0.2.0 sorrend: Szöveg / Logika / Kód
                      {
                        v: "szoveg",
                        title: "Szöveg",
                        desc: "Beszélgetés, kreatív írás, magyar nyelv",
                      },
                      {
                        v: "logika",
                        title: "Logika & matek",
                        desc: "Matematika, logika, Chain-of-Thought",
                      },
                      {
                        v: "kod",
                        title: "Kód",
                        desc: "Programozás: Rust / Python / TS / SQL",
                      },
                    ] as { v: SlotChoice; title: string; desc: string }[]
                  ).map((opt) => {
                    // v0.2.0 gating: Logika és Kód disabled, amíg nem települt.
                    // AUTO és Szöveg mindig elérhető (Szöveg bundle-elt).
                    const expert =
                      opt.v !== "auto"
                        ? setupStatus?.experts.find((e) => e.slot === opt.v)
                        : null;
                    const missing = !!setupStatus && !!expert && !expert.installed;
                    return (
                      <button
                        key={opt.v}
                        type="button"
                        role="menuitemradio"
                        aria-checked={slotChoice === opt.v}
                        className={`chat-view__slot-item ${
                          slotChoice === opt.v ? "is-active" : ""
                        } ${missing ? "is-missing" : ""}`}
                        disabled={!!missing}
                        title={
                          missing
                            ? "Még tölt… Várd meg a háttér-letöltést, vagy nézd meg a státuszát a jobb alsó sarokban."
                            : undefined
                        }
                        onClick={() => {
                          if (missing) return;
                          setSlotChoice(opt.v);
                          setSlotMenuOpen(false);
                        }}
                      >
                        <span className="chat-view__slot-item-title">
                          {opt.title}
                          {missing && (
                            <span className="chat-view__slot-item-missing">
                              Még tölt…
                            </span>
                          )}
                        </span>
                        <span className="chat-view__slot-item-desc">
                          {opt.desc}
                        </span>
                        {slotChoice === opt.v && !missing && (
                          <span className="chat-view__slot-item-check" aria-hidden>
                            ✓
                          </span>
                        )}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>

            <div className="chat-view__composer-spacer" />


            {streaming ? (
              <button
                type="button"
                className="chat-view__send chat-view__send--stop"
                aria-label="Generálás leállítása"
                onClick={stopGeneration}
              >
                <span className="chat-view__stop-square" aria-hidden />
              </button>
            ) : (
              <button
                type="button"
                className="chat-view__send"
                aria-label="Küldés"
                onClick={send}
                disabled={!canSend}
              >
                <img src="/icons/send.png" alt="" width={16} height={16} />
              </button>
            )}
          </div>
        </div>
        <p className="chat-view__disclaimer">
          A Lumi egy mesterséges intelligencia, amely hibázhat. Fontold meg a
          fontos információk ellenőrzését.
        </p>
      </div>
    </div>
  );
}

function slotLabel(s: SlotChoice): string {
  switch (s) {
    case "szoveg":
      return "Szöveg";
    case "logika":
      return "Logika";
    case "kod":
      return "Kód";
    default:
      return "AUTO";
  }
}

// =====================================================================
//  Inline SVG világháló-ikon - saját komponens hogy a kapcsoló-állapotot
//  egyszerűen vizualizálni tudjuk (aranyszín, ha aktív).
// =====================================================================
function GlobeIcon({ active }: { active: boolean }) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={active ? 2.2 : 1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18" />
      <path d="M12 3a14 14 0 0 1 0 18" />
      <path d="M12 3a14 14 0 0 0 0 18" />
    </svg>
  );
}

// =====================================================================
//  Másolás-gomb az asszisztens üzenet alatt - hover-re jelenik meg.
//  Ugyanazt a CopyButton komponenst használja, mint a kódkártyák,
//  hogy konzisztens legyen a UI (azonos ikon, azonos zöld pipa).
// =====================================================================
function MessageCopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  const onClick = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      /* ignore */
    }
  };
  return (
    <div className="chat-msg__copy-wrap">
      <CopyButton copied={copied} onClick={onClick} withLabel />
    </div>
  );
}
