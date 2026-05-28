import { useState } from "react";
import { Modal } from "../../components/Modal";
import { GroupIcon } from "./GroupIcon";
import "./GroupCreateDialog.css";

const COLORS = [
  "#3b82f6", // kék (primary)
  "#22c55e", // zöld
  "#a855f7", // lila
  "#ef4444", // piros
  "#f97316", // narancs
  "#06b6d4", // türkiz
  "#8b5cf6", // ibolya
  "#f5f5f5", // fehér
];

// public/icons/ic1.png ... ic12.png
const ICONS = [
  "ic1", "ic2", "ic3", "ic4", "ic5", "ic6",
  "ic7", "ic8", "ic9", "ic10", "ic11", "ic12",
];

type Props = {
  open: boolean;
  onClose: () => void;
  onCreate: (name: string, color: string, icon: string) => Promise<void> | void;
};

export function GroupCreateDialog({ open, onClose, onCreate }: Props) {
  const [name, setName] = useState("");
  const [color, setColor] = useState(COLORS[0]);
  const [icon, setIcon] = useState(ICONS[0]);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    const trimmed = name.trim();
    if (!trimmed || busy) return;
    setBusy(true);
    try {
      await onCreate(trimmed, color, icon);
      setName("");
      setColor(COLORS[0]);
      setIcon(ICONS[0]);
      onClose();
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} title="Új csoport" maxWidth={420}>
      <div className="group-dialog">
        <label className="group-dialog__field">
          <span>Név</span>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Pl. Munka, Tanulás, Privát…"
            autoFocus
            onKeyDown={(e) => {
              if (e.key === "Enter") submit();
            }}
          />
        </label>

        <div className="group-dialog__field">
          <span>Szín</span>
          <div className="group-dialog__swatches">
            {COLORS.map((c) => (
              <button
                key={c}
                type="button"
                className={`group-dialog__swatch ${c === color ? "is-active" : ""}`}
                style={{ background: c }}
                aria-label={`Szín ${c}`}
                onClick={() => setColor(c)}
              />
            ))}
          </div>
        </div>

        <div className="group-dialog__field">
          <span>Ikon</span>
          <div className="group-dialog__icons">
            {ICONS.map((i) => (
              <button
                key={i}
                type="button"
                className={`group-dialog__icon ${i === icon ? "is-active" : ""}`}
                aria-label={i}
                onClick={() => setIcon(i)}
              >
                <GroupIcon value={i} size={22} />
              </button>
            ))}
          </div>
        </div>

        <div className="group-dialog__actions">
          <button
            type="button"
            className="group-dialog__btn"
            onClick={onClose}
            disabled={busy}
          >
            Mégse
          </button>
          <button
            type="button"
            className="group-dialog__btn group-dialog__btn--primary"
            onClick={submit}
            disabled={busy || !name.trim()}
          >
            {busy ? "Létrehozás…" : "Létrehozás"}
          </button>
        </div>
      </div>
    </Modal>
  );
}
