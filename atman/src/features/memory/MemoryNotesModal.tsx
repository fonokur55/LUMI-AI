import { useCallback, useEffect, useState } from "react";
import { Modal } from "../../components/Modal";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { api, type MemoryNote } from "../../lib/api";
import { showToast } from "../../components/Toast";
import "./MemoryNotesModal.css";

type Props = {
  open: boolean;
  onClose: () => void;
};

/**
 * Memória-kártyák kezelő modal - Gemini-stílusú "saved info" UI.
 *
 * - Bal/teteje: a kártyák listája (cím + tartalom-preview)
 * - Új kártya hozzáadás gomb
 * - Kártya kiválasztva: edit-form (cím + nagy textarea + mentés/törlés/kikapcs)
 *
 * Az engedélyezett kártyák tartalma minden AKASHA-chathívás előtt
 * tömör formátumban a system promptba kerül (max ~600 token), így a
 * felhasználó context-mérete továbbra is nagy marad.
 */
export function MemoryNotesModal({ open, onClose }: Props) {
  const [notes, setNotes] = useState<MemoryNote[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draftTitle, setDraftTitle] = useState("");
  const [draftContent, setDraftContent] = useState("");
  const [isNew, setIsNew] = useState(false);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await api.memoryNotesList();
      setNotes(list);
    } catch (e) {
      console.error("memory notes list failed", e);
    }
  }, []);

  useEffect(() => {
    if (open) {
      refresh();
    } else {
      // bezáráskor töröljük a draftot
      setSelectedId(null);
      setIsNew(false);
      setDraftTitle("");
      setDraftContent("");
    }
  }, [open, refresh]);

  const selectNote = (n: MemoryNote) => {
    setSelectedId(n.id);
    setIsNew(false);
    setDraftTitle(n.title);
    setDraftContent(n.content);
  };

  const startNew = () => {
    setSelectedId(null);
    setIsNew(true);
    setDraftTitle("");
    setDraftContent("");
  };

  const cancelEdit = () => {
    setSelectedId(null);
    setIsNew(false);
    setDraftTitle("");
    setDraftContent("");
  };

  const save = async () => {
    const title = draftTitle.trim();
    const content = draftContent.trim();
    if (!content) {
      showToast("A tartalom nem lehet üres", "error", 3000);
      return;
    }
    try {
      if (isNew) {
        await api.memoryNotesCreate(title, content);
        showToast("Memória-kártya létrehozva");
      } else if (selectedId) {
        await api.memoryNotesUpdate(selectedId, title, content);
        showToast("Memória-kártya mentve");
      }
      await refresh();
      cancelEdit();
    } catch (e) {
      showToast(`Hiba: ${e}`, "error", 4000);
    }
  };

  const toggle = async (n: MemoryNote) => {
    try {
      await api.memoryNotesToggle(n.id, !n.enabled);
      await refresh();
    } catch (e) {
      showToast(`Hiba: ${e}`, "error", 4000);
    }
  };

  const confirmDelete = async () => {
    if (!confirmDeleteId) return;
    const id = confirmDeleteId;
    setConfirmDeleteId(null);
    try {
      await api.memoryNotesDelete(id);
      showToast("Memória-kártya törölve");
      await refresh();
      if (selectedId === id) cancelEdit();
    } catch (e) {
      showToast(`Hiba: ${e}`, "error", 4000);
    }
  };

  const isEditing = isNew || selectedId !== null;

  return (
    <>
      <Modal open={open} onClose={onClose} title="Memória" maxWidth={760}>
        <div className="mem-notes">
          <p className="mem-notes__intro">
            Ide tölthetsz fel{" "}
            <strong>személyes információkat magadról</strong> (hobbi, munka,
            preferenciák), vagy megmondhatod, milyen legyen{" "}
            <strong>AKASHA személyisége</strong>. Több kártyát is
            létrehozhatsz.
          </p>

          {isEditing ? (
            <div className="mem-notes__editor">
              <label className="mem-notes__field">
                <span>Cím (opcionális)</span>
                <input
                  type="text"
                  value={draftTitle}
                  onChange={(e) => setDraftTitle(e.target.value)}
                  placeholder="Pl. Munka, Hobbi, AKASHA-személyiség…"
                  maxLength={80}
                  autoFocus
                />
              </label>
              <label className="mem-notes__field">
                <span>Tartalom</span>
                <textarea
                  value={draftContent}
                  onChange={(e) => setDraftContent(e.target.value)}
                  placeholder={
                    isNew
                      ? "PL. Irodai munkát végzem, mindig magyarul beszélj hozzám, szeretem a lényegre törő tömör válaszokat.\n\nVAGY\n\nPL. Mindig magyarul válaszolj, kerüld a mellébeszélést és mindig nézz utána minden információnak."
                      : ""
                  }
                  rows={8}
                />
              </label>

              <div className="mem-notes__editor-actions">
                <button
                  type="button"
                  className="mem-notes__btn"
                  onClick={cancelEdit}
                >
                  Mégse
                </button>
                <button
                  type="button"
                  className="mem-notes__btn mem-notes__btn--primary"
                  onClick={save}
                  disabled={!draftContent.trim()}
                >
                  {isNew ? "Létrehozás" : "Mentés"}
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="mem-notes__list">
                {notes.length === 0 ? (
                  <div className="mem-notes__empty">
                    Még nincs egyetlen kártyád sem. Az alábbi gombbal létre
                    tudsz hozni egyet.
                  </div>
                ) : (
                  notes.map((n) => (
                    <div
                      key={n.id}
                      className={`mem-notes__card ${
                        n.enabled ? "" : "is-disabled"
                      }`}
                    >
                      <button
                        type="button"
                        className="mem-notes__card-body"
                        onClick={() => selectNote(n)}
                        title="Szerkesztés"
                      >
                        {n.title.trim() && (
                          <span className="mem-notes__card-title">
                            {n.title}
                          </span>
                        )}
                        <span className="mem-notes__card-preview">
                          {n.content.length > 160
                            ? `${n.content.slice(0, 160)}…`
                            : n.content}
                        </span>
                      </button>
                      <div className="mem-notes__card-actions">
                        <label
                          className="mem-notes__toggle"
                          title={
                            n.enabled
                              ? "Bekapcsolva - figyelembe veszi AKASHA"
                              : "Kikapcsolva - figyelmen kívül hagyja"
                          }
                        >
                          <input
                            type="checkbox"
                            checked={n.enabled}
                            onChange={() => toggle(n)}
                          />
                          <span className="mem-notes__toggle-slider" />
                        </label>
                        <button
                          type="button"
                          className="mem-notes__delete"
                          aria-label="Törlés"
                          onClick={() => setConfirmDeleteId(n.id)}
                        >
                          <svg
                            width="16"
                            height="16"
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
                    </div>
                  ))
                )}
              </div>

              <button
                type="button"
                className="mem-notes__btn mem-notes__btn--primary mem-notes__add"
                onClick={startNew}
              >
                + Új kártya
              </button>
            </>
          )}
        </div>
      </Modal>

      <ConfirmDialog
        open={confirmDeleteId !== null}
        title="Memória-kártya törlése"
        message="Biztosan törlöd ezt a memória-kártyát? A művelet nem visszavonható."
        confirmLabel="Törlés"
        cancelLabel="Mégse"
        danger
        onCancel={() => setConfirmDeleteId(null)}
        onConfirm={confirmDelete}
      />
    </>
  );
}
