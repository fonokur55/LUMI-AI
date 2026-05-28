import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { api, type DocumentInfo } from "../../lib/api";
import "./MemoryView.css";

export function MemoryView() {
  const [docs, setDocs] = useState<DocumentInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setDocs(await api.memoryList());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const importDoc = async () => {
    const picked = await open({
      multiple: false,
      filters: [
        {
          name: "Dokumentumok",
          extensions: ["txt", "md", "rs", "ts", "tsx", "js", "json", "py", "csv"],
        },
      ],
    });
    if (!picked || typeof picked !== "string") return;
    setLoading(true);
    setError(null);
    try {
      await api.memoryImport(picked);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const remove = async (id: string) => {
    try {
      await api.memoryDelete(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="memory-view">
      <header className="memory-view__header">
        <h1>Memória</h1>
        <p>Helyi, szerver nélküli RAG - bizalmas dokumentumok és forráskód.</p>
        <button type="button" className="memory-view__import" onClick={importDoc} disabled={loading}>
          {loading ? "Importálás…" : "+ Dokumentum import"}
        </button>
      </header>

      {error && <p className="memory-view__error">{error}</p>}

      <ul className="memory-view__list">
        {docs.length === 0 && (
          <li className="memory-view__empty">Még nincs dokumentum a memóriában.</li>
        )}
        {docs.map((d) => (
          <li key={d.id} className="memory-view__item">
            <div>
              <strong>{d.name}</strong>
              <span>{d.chunkCount} chunk · {new Date(d.createdAt).toLocaleDateString("hu")}</span>
            </div>
            <button type="button" onClick={() => remove(d.id)} aria-label="Törlés">
              ×
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
