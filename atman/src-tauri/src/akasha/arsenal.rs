use serde::Deserialize;
use tauri::{AppHandle, Emitter};

/// Specifikus error-marker, amit az `ensure_model_loaded` ad vissza, ha
/// a llama-server azt mondja, hogy a kért preset nem létezik. Ezt a
/// hívó (lib.rs:akasha_chat) elkapja, RESTART-elja a routert (hogy
/// re-indexelje a `models/` mappát) és újrapróbálkozik a load-dal.
///
/// FONTOS: a marker-stringen a teljes Err értéknek `starts_with`-szel
/// illeszkednie kell — a hívó ezt checkeli.
pub const MODEL_NOT_INDEXED_ERROR: &str = "MODEL_NOT_INDEXED_BY_ROUTER";

#[derive(Debug, Deserialize)]
struct ModelsListResponse {
    data: Option<Vec<ModelEntry>>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: Option<String>,
    status: Option<ModelStatus>,
}

#[derive(Debug, Deserialize)]
struct ModelStatus {
    value: Option<String>,
}

pub async fn ensure_model_loaded(
    app: &AppHandle,
    base_url: &str,
    model_id: &str,
) -> Result<(), String> {
    // #region agent log
    crate::debug_log::dlog(
        "arsenal.rs:ensure_model_loaded",
        "H3",
        "entry",
        serde_json::json!({ "model_id": model_id, "base_url": base_url }),
    );
    // #endregion

    let already = is_model_loaded(base_url, model_id).await?;
    // #region agent log
    crate::debug_log::dlog(
        "arsenal.rs:ensure_model_loaded",
        "H3",
        "is_model_loaded check",
        serde_json::json!({ "model_id": model_id, "already_loaded": already }),
    );
    // #endregion
    if already {
        return Ok(());
    }

    let _ = app.emit(
        "akasha-model-loading",
        serde_json::json!({ "modelId": model_id }),
    );

    let url = format!("{}/models/load", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;

    let body = serde_json::json!({ "model": model_id });
    let res = client.post(&url).json(&body).send().await.map_err(|e| e.to_string())?;

    let post_status = res.status();
    let post_status_u16 = post_status.as_u16();
    let is_success = post_status.is_success();

    if is_success {
        // #region agent log
        crate::debug_log::dlog(
            "arsenal.rs:ensure_model_loaded",
            "H3",
            "POST /models/load success, polling",
            serde_json::json!({ "model_id": model_id, "status": post_status_u16 }),
        );
        // #endregion
        for _ in 0..600 {
            if is_model_loaded(base_url, model_id).await? {
                let _ = app.emit(
                    "akasha-model-ready",
                    serde_json::json!({ "modelId": model_id }),
                );
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        let err = format!(
            "Modell betöltése időtúllépés (5 perc): {model_id}. Ellenőrizd a szabad RAM-ot."
        );
        let _ = app.emit("akasha-error", &err);
        return Err(err);
    }

    let text = res.text().await.unwrap_or_default();
    // #region agent log
    let text_excerpt: String = text.chars().take(500).collect();
    crate::debug_log::dlog(
        "arsenal.rs:ensure_model_loaded",
        "H3",
        "POST /models/load failed",
        serde_json::json!({
            "model_id": model_id,
            "status": post_status_u16,
            "body_excerpt": text_excerpt,
            "will_fallback": post_status_u16 == 404 || text.contains("not found"),
        }),
    );
    // #endregion

    // Specifikus "preset not found" jelzés: a router preset-cache nem
    // tartalmazza ezt a model_id-t. Tipikusan akkor fordul elő, ha a
    // server indulása UTÁN érkezett meg a háttér-letöltésből új .gguf
    // fájl. A hívó (lib.rs:akasha_chat) erre a marker-error-ra
    // restart-elja a routert és újrapróbálkozik.
    if (post_status_u16 == 400 || post_status_u16 == 404)
        && text.contains("not found")
    {
        crate::debug_log::dlog(
            "arsenal.rs:ensure_model_loaded",
            "H3",
            "preset hiányzik a router cache-ből - restart kell",
            serde_json::json!({
                "model_id": model_id,
                "http_status": post_status_u16,
            }),
        );
        return Err(MODEL_NOT_INDEXED_ERROR.to_string());
    }

    if !is_success {
        let err = format!("Modell betöltés sikertelen (HTTP {post_status}): {text}");
        let _ = app.emit("akasha-error", &err);
        return Err(err);
    }

    Ok(())
}

async fn is_model_loaded(base_url: &str, model_id: &str) -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    // Több végpontot is próbálunk - különböző llama-server verziók eltérnek.
    for path in ["/models", "/v1/models"] {
        let url = format!("{}{}", base_url.trim_end_matches('/'), path);
        let Ok(res) = client.get(&url).send().await else {
            continue;
        };
        if !res.status().is_success() {
            continue;
        }
        // #region agent log - capture the raw response so we can see the actual schema
        let raw_text = res.text().await.unwrap_or_default();
        let raw_excerpt: String = raw_text.chars().take(800).collect();
        crate::debug_log::dlog(
            "arsenal.rs:is_model_loaded",
            "H3",
            "models endpoint raw response",
            serde_json::json!({ "path": path, "model_id": model_id, "body_excerpt": raw_excerpt }),
        );
        let Ok(parsed) = serde_json::from_str::<ModelsListResponse>(&raw_text) else {
            continue;
        };
        // #endregion
        for entry in parsed.data.unwrap_or_default() {
            let id_match = entry.id.as_deref() == Some(model_id)
                || entry
                    .id
                    .as_deref()
                    .map(|id| id.ends_with(model_id))
                    .unwrap_or(false);
            if !id_match {
                continue;
            }
            // Ha status hiányzik, legtöbb régi verzió esetén loaded-nek tekintjük.
            let status_ok = match entry.status.as_ref().and_then(|s| s.value.as_deref()) {
                Some(v) => v.eq_ignore_ascii_case("loaded") || v.eq_ignore_ascii_case("ready"),
                None => true,
            };
            if status_ok {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Egyes llama-server verziók nem ismerik a /models/load endpointot.
/// Ilyenkor egyszerűen reményvesztünk - a chat completions hívás meg fogja
/// próbálni közvetlenül a megadott `model` paraméterrel, és a llama-server
/// vagy autoload-ol, vagy értelmes hibát ad vissza.
async fn try_legacy_start_with_model(_base_url: &str, _model_id: &str) -> Result<(), String> {
    Ok(())
}

/// Kiakasztja a modell-t a router RAM-jából.
///
/// RAM-takarékos mód: minden válasz után meghívjuk, hogy a 5.5 GB-os
/// Gemma ne maradjon a memóriában amíg a user nem ír újat. Best-effort:
/// ha hibára fut (pl. régi llama-server build), csak logoljuk és tovább
/// megyünk - a user legfeljebb azt látja hogy nem szabadul fel a RAM,
/// de a chat ettől még működik.
pub async fn unload_model(base_url: &str, model_id: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{}/models/unload", base_url.trim_end_matches('/'));
    let body = serde_json::json!({ "model": model_id });

    let res = client.post(&url).json(&body).send().await
        .map_err(|e| format!("/models/unload kérés sikertelen: {e}"))?;

    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        let excerpt: String = text.chars().take(300).collect();
        // #region agent log
        crate::debug_log::dlog(
            "arsenal.rs:unload_model",
            "H11",
            "POST /models/unload non-success",
            serde_json::json!({
                "model_id": model_id,
                "status": status.as_u16(),
                "body_excerpt": excerpt,
            }),
        );
        // #endregion
        return Err(format!("HTTP {status}: {excerpt}"));
    }

    // #region agent log
    crate::debug_log::dlog(
        "arsenal.rs:unload_model",
        "H11",
        "model kiakasztva - RAM-felszabadítás",
        serde_json::json!({ "model_id": model_id }),
    );
    // #endregion
    Ok(())
}
