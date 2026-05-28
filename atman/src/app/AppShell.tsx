import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { HardwareStatusPill } from "../components/HardwareStatusPill";
import { Modal } from "../components/Modal";
import { WindowControls } from "../components/WindowControls";
import { ChatView } from "../features/chat/ChatView";
import { GroupCreateDialog } from "../features/groups/GroupCreateDialog";
import { GroupIcon } from "../features/groups/GroupIcon";
import { MemoryView } from "../features/memory/MemoryView";
import { ProfileView } from "../features/profile/ProfileView";
import { SettingsView } from "../features/settings/SettingsView";
import {
  api,
  setAkashaPerfProfileBannerHandler,
  type ChatPreview,
  type Group,
  type HardwareProfile,
} from "../lib/api";
import "./AppShell.css";

export type Panel = "chat" | "memory" | "profile" | "settings";

type Props = {
  displayName: string;
  onNameChange: (name: string) => void;
  /** Mentett avatar data URL - ha null, betű-placeholder. */
  avatarUrl?: string | null;
  /** Profile callback, hogy a sidebar avatar is frissüljön mentéskor. */
  onAvatarChange?: () => void;
  /** Ha nem null, ma a user szülinapja → a ChatView welcome címe ez. */
  birthdayGreeting?: string | null;
  /** Aktív téma - a sidebar logo választáshoz */
  theme?: string;
};

function newId() {
  return crypto.randomUUID();
}

export function AppShell({
  displayName,
  onNameChange,
  avatarUrl,
  onAvatarChange,
  birthdayGreeting,
  theme = "light",
}: Props) {
  const [panel, setPanel] = useState<Panel>("chat");
  const [sidebarOpen, setSidebarOpen] = useState(true);

  // Chat state
  const [chatId, setChatId] = useState<string>(() => newId());
  const [chatTitle, setChatTitle] = useState<string>("Új beszélgetés");
  const [chats, setChats] = useState<ChatPreview[]>([]);
  const [groups, setGroups] = useState<Group[]>([]);

  // UI state
  const [groupsOpen, setGroupsOpen] = useState(false);
  const [groupFilter, setGroupFilter] = useState<string | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [groupDialogOpen, setGroupDialogOpen] = useState(false);

  // 3-pötty menu
  const [menuOpenFor, setMenuOpenFor] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  // "Csoporthoz adás" modal - a chat ID-jét tartja amelyhez csoportot
  // választunk. Külön modálban, hogy ne csorduljon le a sidebarból a
  // beágyazott submenu.
  const [assignChatId, setAssignChatId] = useState<string | null>(null);

  // Rename + delete
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  // Csoport-törlés megerősítő dialog
  const [confirmGroupDeleteId, setConfirmGroupDeleteId] = useState<string | null>(null);

  // === Adaptív Védelmi Protokoll banner ===
  // - Indítás után az első profilolás után ~10 mp-re látszik az alap üzenet
  // - Chat-küldés előtti tier-recheck után, ha LECSÖKKENT a tier, banner jön
  //   ami egészen addig látszik amíg a user elbocsátja
  const [perfBanner, setPerfBanner] = useState<{
    profile: HardwareProfile;
    autoHide: boolean;
    exiting?: boolean;
  } | null>(null);
  // sessionStorage flag, hogy ne mutassuk minden re-mount-kor az indítási banner-t
  const startupShownRef = useRef(false);

  /** Banner finom eltüntetése: előbb CSS exit-animáció, aztán unmount. */
  const dismissPerfBanner = useCallback(() => {
    setPerfBanner((b) => (b ? { ...b, exiting: true } : null));
    window.setTimeout(() => setPerfBanner(null), 200);
  }, []);

  const refreshChats = useCallback(async () => {
    try {
      const list =
        searchQuery.trim() === ""
          ? await api.chatsList()
          : await api.chatSearch(searchQuery);
      setChats(list);
    } catch (e) {
      console.error("chats list failed", e);
    }
  }, [searchQuery]);

  const refreshGroups = useCallback(async () => {
    try {
      setGroups(await api.groupsList());
    } catch (e) {
      console.error("groups list failed", e);
    }
  }, []);

  useEffect(() => {
    refreshChats();
    refreshGroups();
  }, [refreshChats, refreshGroups]);

  // Startup-banner: lekéri a friss tier-profilt és 10 mp-re megmutatja
  // (kivéve ha a sessionStorage szerint ezt már elbocsátották).
  useEffect(() => {
    if (startupShownRef.current) return;
    startupShownRef.current = true;
    if (sessionStorage.getItem("atman.perfBannerShown") === "1") return;
    api
      .getHardwareProfile()
      .then((profile) => {
        setPerfBanner({ profile, autoHide: true });
        // 10 mp után magától elszáll (fade-out animációval)
        window.setTimeout(() => {
          setPerfBanner((b) => {
            if (!b?.autoHide) return b;
            // exit fázis indítása
            window.setTimeout(() => setPerfBanner(null), 200);
            return { ...b, exiting: true };
          });
          sessionStorage.setItem("atman.perfBannerShown", "1");
        }, 10000);
      })
      .catch(() => {});
  }, []);

  // Pre-chat tier-recheck → ha LECSÖKKENT a tier, banner permanensen ott
  // marad amíg a user elbocsátja (autoHide: false). A dedicated csatorna
  // (setAkashaPerfProfileBannerHandler) garantálja hogy ez NEM ütközik
  // a ChatView stream-handlerekkel.
  useEffect(() => {
    setAkashaPerfProfileBannerHandler((profile) => {
      const downgraded =
        profile.effectiveTier === "limp" ||
        profile.effectiveTier === "blocked";
      if (downgraded) {
        setPerfBanner({ profile, autoHide: false });
      }
    });
    return () => {
      setAkashaPerfProfileBannerHandler(null);
    };
  }, []);

  // Bezárás click-en kívül
  useEffect(() => {
    if (!menuOpenFor) return;
    const onClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpenFor(null);
      }
    };
    window.addEventListener("mousedown", onClick);
    return () => window.removeEventListener("mousedown", onClick);
  }, [menuOpenFor]);

  const newChat = useCallback(() => {
    setPanel("chat");
    setChatId(newId());
    setChatTitle("Új beszélgetés");
  }, []);

  const openChat = useCallback((id: string, title: string) => {
    setPanel("chat");
    setChatId(id);
    setChatTitle(title);
  }, []);

  // === 3-pötty menü műveletek ===

  const togglePin = async (c: ChatPreview) => {
    await api.chatPin(c.id, !c.pinned);
    setMenuOpenFor(null);
    refreshChats();
  };

  const startRename = (c: ChatPreview) => {
    setRenamingId(c.id);
    setRenameValue(c.title);
    setMenuOpenFor(null);
  };

  const commitRename = async () => {
    if (!renamingId) return;
    const trimmed = renameValue.trim();
    if (trimmed) {
      await api.chatRename(renamingId, trimmed);
      if (chatId === renamingId) setChatTitle(trimmed);
    }
    setRenamingId(null);
    setRenameValue("");
    refreshChats();
  };

  const assignToGroup = async (chatPid: string, groupIdOrNull: string | null) => {
    await api.chatSetGroup(chatPid, groupIdOrNull);
    setMenuOpenFor(null);
    setAssignChatId(null);
    refreshChats();
  };

  const confirmDelete = async () => {
    if (!confirmDeleteId) return;
    const idToDelete = confirmDeleteId;
    setConfirmDeleteId(null);
    await api.chatDelete(idToDelete);
    if (chatId === idToDelete) {
      newChat();
    }
    refreshChats();
  };

  // === Csoport létrehozás ===

  const createGroup = async (name: string, color: string, icon: string) => {
    await api.groupCreate(name, color, icon);
    refreshGroups();
  };

  const confirmDeleteGroup = async () => {
    if (!confirmGroupDeleteId) return;
    const id = confirmGroupDeleteId;
    setConfirmGroupDeleteId(null);
    try {
      await api.groupDelete(id);
      // Ha a törölt csoport épp aktív szűrő volt, vegyük le.
      if (groupFilter === id) setGroupFilter(null);
      await refreshGroups();
      await refreshChats(); // a benne lévő chatek groupId-je most már null
    } catch (e) {
      console.error("group delete failed", e);
    }
  };

  // === Lista szűrés csoport szerint ===

  const filteredChats = useMemo(() => {
    if (!groupFilter) {
      // A "Beszélgetések" szekcióban: minden, ami nincs nyitott csoportszűrőben
      return chats;
    }
    return chats.filter((c) => c.groupId === groupFilter);
  }, [chats, groupFilter]);

  // A sidebar által nyújtott callback a ChatView-nak, hogy refresh-elje a listát küldés után.
  const onChatPersisted = useCallback(
    (title: string) => {
      setChatTitle(title);
      refreshChats();
    },
    [refreshChats],
  );

  return (
    <div className="app-shell">
      <header className="app-shell__titlebar" data-tauri-drag-region>
        <div className="app-shell__titlebar-spacer" />
        <div className="app-shell__titlebar-right">
          <HardwareStatusPill onClick={() => setPanel("settings")} />
          <WindowControls />
        </div>
      </header>

      {perfBanner && (
        <div
          className={`perf-banner perf-banner--${perfBanner.profile.effectiveTier} ${perfBanner.exiting ? "is-exiting" : ""}`}
          role="status"
        >
          <span className="perf-banner__dot" aria-hidden />
          <span className="perf-banner__text">{perfBanner.profile.message}</span>
          <button
            type="button"
            className="perf-banner__close"
            aria-label="Bezárás"
            onClick={() => {
              dismissPerfBanner();
              sessionStorage.setItem("atman.perfBannerShown", "1");
            }}
          >
            ×
          </button>
        </div>
      )}

      <div className="app-shell__body">
        <aside className={`sidebar ${sidebarOpen ? "is-open" : "is-closed"}`}>
          <div className="sidebar__top">
            <button
              type="button"
              className="sidebar__brand"
              aria-label="LUMI - új beszélgetés"
              onClick={newChat}
            >
              <img
                src={
                  theme === "light"
                    ? "/brand/lumi_hosszu_kek_fekete_szoveg.png"
                    : "/brand/lumi_hosszu_kek.png"
                }
                alt="LUMI"
                className="sidebar__brand-logo"
              />
            </button>
            <button
              type="button"
              className="sidebar__icon-btn"
              aria-label="Oldalsáv elrejtése"
              onClick={() => setSidebarOpen(false)}
            >
              <img src="/icons/sidebar.png" alt="" width={20} height={20} />
            </button>
          </div>

          <nav className="sidebar__nav">
            <button type="button" className="sidebar__item" onClick={newChat}>
              <img src="/icons/new-message.png" alt="" width={18} height={18} />
              <span>Új üzenet</span>
            </button>

            <button
              type="button"
              className="sidebar__item sidebar__item--groups"
              onClick={() => setGroupsOpen((o) => !o)}
              aria-expanded={groupsOpen}
            >
              <img src="/icons/folder.png" alt="" width={18} height={18} />
              <span>Csoportok</span>
              <img
                src="/icons/chevron.png"
                alt=""
                width={12}
                height={12}
                className={`sidebar__chevron ${groupsOpen ? "is-open" : ""}`}
              />
            </button>

            {groupsOpen && (
              <div className="sidebar__groups">
                {groups.length === 0 && (
                  <div className="sidebar__groups-empty">Még nincs csoport.</div>
                )}
                {groups.map((g) => (
                  <div key={g.id} className="sidebar__group-row">
                    <button
                      type="button"
                      className={`sidebar__group-item ${groupFilter === g.id ? "is-active" : ""}`}
                      onClick={() =>
                        setGroupFilter((curr) => (curr === g.id ? null : g.id))
                      }
                    >
                      <GroupIcon
                        value={g.icon}
                        size={16}
                        className="sidebar__group-icon"
                      />
                      <span
                        className="sidebar__group-dot"
                        style={{ background: g.color }}
                        aria-hidden="true"
                      />
                      <span className="sidebar__group-name">{g.name}</span>
                    </button>
                    <button
                      type="button"
                      className="sidebar__group-delete"
                      aria-label={`Csoport törlése: ${g.name}`}
                      title="Csoport törlése"
                      onClick={(e) => {
                        e.stopPropagation();
                        setConfirmGroupDeleteId(g.id);
                      }}
                    >
                      <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        aria-hidden
                      >
                        <path d="M3 6h18" />
                        <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                        <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
                      </svg>
                    </button>
                  </div>
                ))}
                <button
                  type="button"
                  className="sidebar__group-create"
                  onClick={() => setGroupDialogOpen(true)}
                >
                  <img src="/icons/plus.png" alt="" width={12} height={12} />
                  <span>Új csoport</span>
                </button>
              </div>
            )}

            <div className="sidebar__section">
              <span className="sidebar__section-label">
                {groupFilter
                  ? groups.find((g) => g.id === groupFilter)?.name ?? "Csoport"
                  : "Beszélgetések"}
              </span>
              <button
                type="button"
                className="sidebar__section-action"
                aria-label="Keresés"
                onClick={() => {
                  setSearchOpen((o) => !o);
                  if (searchOpen) setSearchQuery("");
                }}
              >
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  aria-hidden="true"
                >
                  <circle cx="11" cy="11" r="7" />
                  <path d="m20 20-3.5-3.5" />
                </svg>
              </button>
            </div>

            {searchOpen && (
              <input
                type="text"
                className="sidebar__search"
                placeholder="Keresés…"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                autoFocus
              />
            )}

            <div className="sidebar__convs">
              {filteredChats.length === 0 && (
                <div className="sidebar__convs-empty">
                  {searchQuery.trim()
                    ? "Nincs találat."
                    : groupFilter
                      ? "Ebben a csoportban nincs beszélgetés."
                      : "Még nincs mentett beszélgetés."}
                </div>
              )}
              {filteredChats.map((c) => (
                <div
                  key={c.id}
                  className={`sidebar__conv ${chatId === c.id ? "is-active" : ""}`}
                >
                  {renamingId === c.id ? (
                    <input
                      className="sidebar__conv-rename"
                      value={renameValue}
                      autoFocus
                      onChange={(e) => setRenameValue(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") commitRename();
                        if (e.key === "Escape") {
                          setRenamingId(null);
                          setRenameValue("");
                        }
                      }}
                      onBlur={commitRename}
                    />
                  ) : (
                    <button
                      type="button"
                      className="sidebar__conv-btn"
                      onClick={() => openChat(c.id, c.title)}
                    >
                      {c.pinned && (
                        <svg
                          width="11"
                          height="11"
                          viewBox="0 0 24 24"
                          fill="currentColor"
                          className="sidebar__conv-pin"
                          aria-hidden="true"
                        >
                          <path d="M14 4l6 6-3 1-1 5-5-5-6 6v-3l6-6 5-1 1-3z" />
                        </svg>
                      )}
                      <span className="sidebar__conv-title">{c.title}</span>
                    </button>
                  )}

                  <button
                    type="button"
                    className="sidebar__conv-more"
                    aria-label="További műveletek"
                    onClick={(e) => {
                      e.stopPropagation();
                      setMenuOpenFor(menuOpenFor === c.id ? null : c.id);
                    }}
                  >
                    <img src="/icons/more.png" alt="" width={14} height={14} />
                  </button>

                  {menuOpenFor === c.id && (
                    <div ref={menuRef} className="sidebar__conv-menu" role="menu">
                      <button type="button" role="menuitem" onClick={() => togglePin(c)}>
                        <svg
                          width="14"
                          height="14"
                          viewBox="0 0 24 24"
                          fill="currentColor"
                          aria-hidden="true"
                        >
                          <path d="M14 4l6 6-3 1-1 5-5-5-6 6v-3l6-6 5-1 1-3z" />
                        </svg>
                        {c.pinned ? "Kitűzés megszüntetése" : "Kitűzés"}
                      </button>

                      {/* Csoporthoz adás: nem inline submenu (az lelógott a
                          sidebar overflow-ja miatt), hanem egy rendes modal
                          nyílik, középen a képernyőn. */}
                      <button
                        type="button"
                        role="menuitem"
                        onClick={() => {
                          setMenuOpenFor(null);
                          setAssignChatId(c.id);
                        }}
                      >
                        <img src="/icons/folder.png" alt="" width={14} height={14} />
                        Csoporthoz adás…
                      </button>

                      <button
                        type="button"
                        role="menuitem"
                        onClick={() => startRename(c)}
                      >
                        <svg
                          width="14"
                          height="14"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="2"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          aria-hidden="true"
                        >
                          <path d="M12 20h9" />
                          <path d="M16.5 3.5a2.121 2.121 0 1 1 3 3L7 19l-4 1 1-4Z" />
                        </svg>
                        Átnevezés
                      </button>

                      <button
                        type="button"
                        role="menuitem"
                        className="sidebar__conv-menu-danger"
                        onClick={() => {
                          setMenuOpenFor(null);
                          setConfirmDeleteId(c.id);
                        }}
                      >
                        <svg
                          width="14"
                          height="14"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="2"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          aria-hidden="true"
                        >
                          <path d="M3 6h18" />
                          <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
                        </svg>
                        Törlés
                      </button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </nav>

          <div className="sidebar__footer">
            <button
              type="button"
              className={`sidebar__item ${panel === "settings" ? "is-active" : ""}`}
              onClick={() => setPanel("settings")}
            >
              <img src="/icons/settings.png" alt="" width={18} height={18} />
              <span>Beállítások</span>
            </button>
            <button
              type="button"
              className={`sidebar__item ${panel === "profile" ? "is-active" : ""}`}
              onClick={() => setPanel("profile")}
            >
              {avatarUrl ? (
                <img
                  src={avatarUrl}
                  alt=""
                  className="sidebar__avatar sidebar__avatar--img"
                />
              ) : (
                <span className="sidebar__avatar">
                  {(displayName || "?").trim().charAt(0).toUpperCase()}
                </span>
              )}
              <span>Profil</span>
            </button>
          </div>
        </aside>

        <main className="app-shell__main">
          {!sidebarOpen && (
            <div className="app-shell__rail" aria-label="Gyors műveletek">
              <button
                type="button"
                className="app-shell__rail-btn"
                aria-label="Oldalsáv megnyitása"
                onClick={() => setSidebarOpen(true)}
              >
                <img src="/icons/sidebar.png" alt="" width={20} height={20} />
              </button>
              <button
                type="button"
                className="app-shell__rail-btn"
                aria-label="Új üzenet"
                title="Új üzenet"
                onClick={newChat}
              >
                <img src="/icons/new-message.png" alt="" width={20} height={20} />
              </button>
            </div>
          )}

          {/* Panel-fade: a panel-kulcs (chat/memory/profile/settings)
              változására fade-in animáció indul. A ChatView a chatId-t
              is kulcsként használja, hogy beszélgetés-váltáskor is fade-eljen. */}
          <div className="app-shell__panel-fade" key={`${panel}:${chatId}`}>
            {panel === "chat" && (
              <ChatView
                chatId={chatId}
                initialTitle={chatTitle}
                displayName={displayName}
                birthdayGreeting={birthdayGreeting}
                onPersisted={onChatPersisted}
              />
            )}
            {panel === "memory" && <MemoryView />}
            {panel === "profile" && (
              <ProfileView
                displayName={displayName}
                onNameChange={onNameChange}
                onAvatarChange={onAvatarChange}
              />
            )}
            {panel === "settings" && <SettingsView />}
          </div>
        </main>
      </div>

      <GroupCreateDialog
        open={groupDialogOpen}
        onClose={() => setGroupDialogOpen(false)}
        onCreate={createGroup}
      />

      <Modal
        open={assignChatId !== null}
        title="Csoport választása"
        onClose={() => setAssignChatId(null)}
        maxWidth={420}
      >
        <div className="assign-modal">
          <p className="assign-modal__hint">
            Válassz, melyik csoportba kerüljön a beszélgetés. Bármikor
            visszavonható.
          </p>

          <div className="assign-modal__list">
            {/* "Nincs csoport" - kivenni az aktuális csoportból. */}
            <button
              type="button"
              className="assign-modal__item"
              onClick={() =>
                assignChatId && assignToGroup(assignChatId, null)
              }
            >
              <span
                className="sidebar__group-dot"
                style={{ background: "#444" }}
                aria-hidden
              />
              <span className="assign-modal__item-name">Nincs csoport</span>
              {assignChatId &&
                chats.find((c) => c.id === assignChatId)?.groupId === null && (
                  <span className="assign-modal__item-check" aria-hidden>
                    ✓
                  </span>
                )}
            </button>

            {groups.length === 0 && (
              <div className="assign-modal__empty">
                Még nincs csoportod. Hozz létre egyet a sidebar
                „+&nbsp;Új csoport" gombjával.
              </div>
            )}

            {groups.map((g) => {
              const isCurrent =
                assignChatId &&
                chats.find((c) => c.id === assignChatId)?.groupId === g.id;
              return (
                <button
                  key={g.id}
                  type="button"
                  className="assign-modal__item"
                  onClick={() =>
                    assignChatId && assignToGroup(assignChatId, g.id)
                  }
                >
                  <GroupIcon
                    value={g.icon}
                    size={18}
                    className="sidebar__group-icon"
                  />
                  <span
                    className="sidebar__group-dot"
                    style={{ background: g.color }}
                    aria-hidden
                  />
                  <span className="assign-modal__item-name">{g.name}</span>
                  {isCurrent && (
                    <span className="assign-modal__item-check" aria-hidden>
                      ✓
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      </Modal>

      <ConfirmDialog
        open={confirmDeleteId !== null}
        title="Beszélgetés törlése"
        message="Biztosan törlöd ezt a beszélgetést? A művelet nem visszavonható."
        confirmLabel="Törlés"
        cancelLabel="Mégse"
        danger
        onCancel={() => setConfirmDeleteId(null)}
        onConfirm={confirmDelete}
      />

      <ConfirmDialog
        open={confirmGroupDeleteId !== null}
        title="Csoport törlése"
        message={(() => {
          const g = groups.find((x) => x.id === confirmGroupDeleteId);
          const name = g?.name ?? "ez a csoport";
          return `Biztosan törlöd a "${name}" csoportot? A benne lévő beszélgetések megmaradnak (csoporton kívülre kerülnek), csak maga a csoport törlődik.`;
        })()}
        confirmLabel="Törlés"
        cancelLabel="Mégse"
        danger
        onCancel={() => setConfirmGroupDeleteId(null)}
        onConfirm={confirmDeleteGroup}
      />
    </div>
  );
}
