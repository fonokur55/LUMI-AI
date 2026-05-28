import { useCallback, useEffect, useRef, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { api, type ProfileData } from "../../lib/api";
import { showToast } from "../../components/Toast";
import "./ProfileView.css";

type Props = {
  displayName: string;
  onNameChange: (name: string) => void;
  /** Mentés/törlés után a sidebar avatarja is frissül. */
  onAvatarChange?: () => void;
};

const MONTHS_HU = [
  "január", "február", "március", "április", "május", "június",
  "július", "augusztus", "szeptember", "október", "november", "december",
];

export function ProfileView({ displayName, onNameChange, onAvatarChange }: Props) {
  const [profile, setProfile] = useState<ProfileData | null>(null);
  const [nameEdit, setNameEdit] = useState(displayName);
  const [editingAvatar, setEditingAvatar] = useState<string | null>(null);
  const [savedAvatarUrl, setSavedAvatarUrl] = useState<string | null>(null);
  const nameSaveTimer = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    const p = await api.profileGet();
    setProfile(p);
    // A mentett avatart Rust olvassa be base64-ben → data URL
    // (asset protokoll megkerülése - sokkal megbízhatóbb).
    try {
      const url = await api.profileGetAvatarDataUrl();
      setSavedAvatarUrl(url);
    } catch (e) {
      console.error("avatar load failed", e);
      setSavedAvatarUrl(null);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    setNameEdit(displayName);
  }, [displayName]);

  // Név auto-save (debounce)
  useEffect(() => {
    if (nameEdit === displayName) return;
    if (nameSaveTimer.current) window.clearTimeout(nameSaveTimer.current);
    nameSaveTimer.current = window.setTimeout(async () => {
      try {
        await api.profileUpdateName(nameEdit);
        onNameChange(nameEdit);
        showToast("Név mentve");
        refresh();
      } catch (e) {
        showToast(`Hiba: ${e}`, "error", 4000);
      }
    }, 600);
    return () => {
      if (nameSaveTimer.current) window.clearTimeout(nameSaveTimer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nameEdit]);

  const pickAvatarFile = async () => {
    try {
      const path = await openFileDialog({
        multiple: false,
        filters: [{ name: "Kép", extensions: ["png", "jpg", "jpeg", "webp"] }],
      });
      if (!path || typeof path !== "string") return;
      // Rust beolvassa és base64 data URL-ként visszaadja.
      // Megbízható minden képformátumra, nem függ az asset protokolltól.
      const dataUrl = await api.readImageDataUrl(path);
      setEditingAvatar(dataUrl);
    } catch (e) {
      showToast(`Kép-betöltés hiba: ${e}`, "error", 4000);
    }
  };

  const removeAvatar = async () => {
    try {
      await api.profileClearAvatar();
      showToast("Profilkép eltávolítva");
      await refresh();
      onAvatarChange?.();
    } catch (e) {
      showToast(`Hiba: ${e}`, "error", 4000);
    }
  };

  const handleBirthdayChange = async (m: number, d: number) => {
    try {
      await api.profileSetBirthday(m, d);
      showToast("Születésnap mentve");
      await refresh();
    } catch (e) {
      showToast(`Hiba: ${e}`, "error", 4000);
    }
  };

  if (!profile) {
    return <div className="profile-view">Betöltés…</div>;
  }

  // Mentett avatart most már a Rust olvassa be base64-ben (data URL).
  const avatarSrc = savedAvatarUrl;

  return (
    <div className="profile-view">
      <h1>Profil</h1>

      {/* ===== FEJLÉC: avatar + név + alap statisztika ===== */}
      <section className="profile-card profile-header">
        <div className="profile-avatar-wrap">
          {avatarSrc ? (
            <img
              className="profile-avatar"
              src={avatarSrc}
              alt={nameEdit || "Avatar"}
            />
          ) : (
            <div className="profile-avatar profile-avatar--placeholder">
              {(nameEdit || "?").trim().charAt(0).toUpperCase()}
            </div>
          )}
          <div className="profile-avatar-actions">
            <button
              type="button"
              className="profile-btn profile-btn--small"
              onClick={pickAvatarFile}
            >
              {avatarSrc ? "Csere" : "Kép feltöltése"}
            </button>
            {avatarSrc && (
              <button
                type="button"
                className="profile-btn profile-btn--small profile-btn--danger"
                onClick={removeAvatar}
              >
                Törlés
              </button>
            )}
          </div>
        </div>

        <div className="profile-header__body">
          <label className="profile-field">
            <span>Megjelenített név</span>
            <input
              type="text"
              className="profile-name-input"
              value={nameEdit}
              onChange={(e) => setNameEdit(e.target.value)}
              placeholder="Hogy szólítsalak?"
              maxLength={40}
            />
          </label>
          <p className="profile-header__stats">
            {profile.messagesSent} üzenet
          </p>
        </div>
      </section>

      {/* ===== SZÜLETÉSNAP ===== */}
      <section className="profile-card">
        <h2>Születésnap</h2>
        <p className="profile-hint">
          LUMI ezen a napon konfettivel köszönt 🎉
        </p>
        <div className="profile-bday-row">
          <select
            value={profile.birthMonth ?? 1}
            onChange={(e) => {
              const m = Number(e.target.value);
              handleBirthdayChange(m, profile.birthDay ?? 1);
            }}
          >
            {MONTHS_HU.map((name, i) => (
              <option key={i} value={i + 1}>
                {name}
              </option>
            ))}
          </select>
          <select
            value={profile.birthDay ?? 1}
            onChange={(e) =>
              handleBirthdayChange(
                profile.birthMonth ?? 1,
                Number(e.target.value),
              )
            }
          >
            {Array.from(
              { length: daysInMonth(profile.birthMonth ?? 1) },
              (_, i) => i + 1,
            ).map((d) => (
              <option key={d} value={d}>
                {d}.
              </option>
            ))}
          </select>
          {profile.birthMonth && profile.birthDay && (
            <span className="profile-bday-summary">
              {MONTHS_HU[profile.birthMonth - 1]} {profile.birthDay}.
            </span>
          )}
        </div>
      </section>

      {/* ===== AVATAR-CROP MODAL ===== */}
      {editingAvatar && (
        <AvatarCropModal
          src={editingAvatar}
          onClose={() => {
            setEditingAvatar(null);
          }}
          onSave={async (pngBase64) => {
            try {
              await api.profileSaveAvatar(pngBase64);
              showToast("Profilkép mentve");
              setEditingAvatar(null);
              await refresh();
              onAvatarChange?.();
            } catch (e) {
              showToast(`Hiba: ${e}`, "error", 4000);
            }
          }}
        />
      )}
    </div>
  );
}

function daysInMonth(month: number): number {
  const lengths = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return lengths[month - 1] ?? 31;
}

// =====================================================================
//  Avatar crop modal - egyszerű drag-to-position UI canvas alapokon.
//  A user áthúzza a képet egy 256×256-os négyzetbe (rács-overlay
//  segítségével), aztán a canvasból PNG base64-et küldünk a backend-nek.
// =====================================================================
function AvatarCropModal({
  src,
  onClose,
  onSave,
}: {
  src: string;
  onClose: () => void;
  onSave: (pngBase64: string) => Promise<void>;
}) {
  const CROP_SIZE = 256;
  const [scale, setScale] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [imgSize, setImgSize] = useState({ w: 0, h: 0 });
  const dragRef = useRef<{ x: number; y: number; ox: number; oy: number } | null>(
    null,
  );
  const imgRef = useRef<HTMLImageElement | null>(null);
  const [saving, setSaving] = useState(false);

  const onImgLoad = (e: React.SyntheticEvent<HTMLImageElement>) => {
    const img = e.currentTarget;
    setImgSize({ w: img.naturalWidth, h: img.naturalHeight });
    // Initial scale: a kép kisebbik oldalát skálázzuk a crop-méret-re
    const minSide = Math.min(img.naturalWidth, img.naturalHeight);
    setScale(CROP_SIZE / minSide);
    setOffset({ x: 0, y: 0 });
  };

  const onMouseDown = (e: React.MouseEvent) => {
    dragRef.current = {
      x: e.clientX,
      y: e.clientY,
      ox: offset.x,
      oy: offset.y,
    };
  };
  const onMouseMove = (e: React.MouseEvent) => {
    if (!dragRef.current) return;
    const dx = e.clientX - dragRef.current.x;
    const dy = e.clientY - dragRef.current.y;
    setOffset({ x: dragRef.current.ox + dx, y: dragRef.current.oy + dy });
  };
  const onMouseUp = () => {
    dragRef.current = null;
  };

  const handleSave = async () => {
    if (!imgRef.current) return;
    setSaving(true);
    const canvas = document.createElement("canvas");
    canvas.width = CROP_SIZE;
    canvas.height = CROP_SIZE;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    // A kép a crop-doboz közepétől + offset eltolással + scale-szel rajzolódik.
    // A canvas 0,0-ja a crop-doboz bal-felső sarka.
    const drawW = imgSize.w * scale;
    const drawH = imgSize.h * scale;
    const dx = (CROP_SIZE - drawW) / 2 + offset.x;
    const dy = (CROP_SIZE - drawH) / 2 + offset.y;
    ctx.fillStyle = "#1f1f1e";
    ctx.fillRect(0, 0, CROP_SIZE, CROP_SIZE);
    ctx.drawImage(imgRef.current, dx, dy, drawW, drawH);
    const dataUrl = canvas.toDataURL("image/png");
    const base64 = dataUrl.replace(/^data:image\/png;base64,/, "");
    await onSave(base64);
    setSaving(false);
  };

  const drawW = imgSize.w * scale;
  const drawH = imgSize.h * scale;
  const dx = (CROP_SIZE - drawW) / 2 + offset.x;
  const dy = (CROP_SIZE - drawH) / 2 + offset.y;

  return (
    <div className="avatar-crop-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="avatar-crop-modal">
        <h3>Profilkép igazítása</h3>
        <p className="avatar-crop-hint">
          Húzd a képet a megfelelő pozícióba. A kerek vágás-keret mutatja, mi
          fog látszani.
        </p>
        <div
          className="avatar-crop-area"
          style={{ width: CROP_SIZE, height: CROP_SIZE }}
          onMouseDown={onMouseDown}
          onMouseMove={onMouseMove}
          onMouseUp={onMouseUp}
          onMouseLeave={onMouseUp}
        >
          <img
            ref={imgRef}
            src={src}
            onLoad={onImgLoad}
            alt=""
            draggable={false}
            style={{
              position: "absolute",
              left: dx,
              top: dy,
              width: drawW,
              height: drawH,
              userSelect: "none",
              pointerEvents: "none",
            }}
          />
          {/* Rács overlay */}
          <div className="avatar-crop-grid" aria-hidden>
            <div />
            <div />
            <div />
            <div />
          </div>
          {/* Kör-maszk overlay (jelzi a végleges crop-területet) */}
          <div className="avatar-crop-mask" aria-hidden />
        </div>

        <label className="avatar-crop-zoom">
          <span>Nagyítás</span>
          <input
            type="range"
            min={0.2}
            max={3}
            step={0.05}
            value={scale}
            onChange={(e) => setScale(Number(e.target.value))}
          />
        </label>

        <div className="avatar-crop-actions">
          <button type="button" onClick={onClose} className="profile-btn">
            Mégse
          </button>
          <button
            type="button"
            onClick={handleSave}
            disabled={saving}
            className="profile-btn profile-btn--primary"
          >
            {saving ? "Mentés…" : "Mentés"}
          </button>
        </div>
      </div>
    </div>
  );
}
