mod akasha;
mod chats;
mod downloader;
mod memory;
mod memory_notes;
mod portable;
mod profile;
mod state;
mod web_search;
// #region agent log
mod debug_log;
// #endregion

use akasha::arsenal::ensure_model_loaded;
use akasha::client::{stream_chat, StreamContext};
use akasha::router::{route_prompt, RouteInput};
use akasha::types::{domain_to_string, slot_to_domain, AkashaSlot, ChatMessage};
use akasha::{AkashaStatus, HardwareSnapshot, ThrottleLevel};
use chats::{ChatFull, ChatPreview, Group};
use memory::rag::build_rag_context;
use portable::{init_config, migrate_eco_model, save_config, AtmanConfig, AppPaths};
use profile::events::UsageDomain;
use state::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn get_app_paths(state: State<'_, AppState>) -> Result<AppPaths, String> {
    Ok(state.paths.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> Result<AtmanConfig, String> {
    Ok(state.config.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn save_app_config(state: State<'_, AppState>, config: AtmanConfig) -> Result<(), String> {
    let paths = state.paths.lock().map_err(|e| e.to_string())?.clone();
    save_config(&paths, &config)?;
    if let Ok(mut p) = state.profile.lock() {
        if let Some(profile) = p.as_mut() {
            let _ = profile.set_display_name(&config.profile.display_name);
        }
    }
    if let Ok(mut c) = state.config.lock() {
        *c = config;
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AkashaStatusResponse {
    status: AkashaStatus,
    port: u16,
    base_url: Option<String>,
    error: Option<String>,
    active_slot: Option<String>,
    active_model: Option<String>,
    throttle_level: ThrottleLevel,
    hardware: Option<HardwareSnapshot>,
}

#[tauri::command]
fn akasha_hardware(state: State<'_, AppState>) -> Result<HardwareSnapshot, String> {
    let mut hw = state.hardware.lock().map_err(|e| e.to_string())?;
    Ok(hw.snapshot())
}

/// Adaptív Védelmi Protokoll (Fázis 1) - visszaadja a frissen-mért hardware-
/// profilt + a Settings override-okat figyelembe véve a tényleges tier-t.
#[tauri::command]
fn get_hardware_profile(
    state: State<'_, AppState>,
) -> Result<akasha::perf::HardwareProfile, String> {
    let snap = {
        let mut hw = state.hardware.lock().map_err(|e| e.to_string())?;
        hw.snapshot()
    };
    let perf_cfg = state.config.lock().map_err(|e| e.to_string())?.performance.clone();
    Ok(akasha::perf::compute_profile(
        &snap,
        perf_cfg.forced_tier.as_deref(),
        perf_cfg.hardware_protection_enabled,
    ))
}

#[tauri::command]
async fn akasha_status(state: State<'_, AppState>) -> Result<AkashaStatusResponse, String> {
    // #region agent log
    debug_log::dlog(
        "lib.rs:akasha_status",
        "H5",
        "entry",
        serde_json::json!({}),
    );
    // #endregion
    let hardware = {
        let mut hw = state.hardware.lock().map_err(|e| e.to_string())?;
        Some(hw.snapshot())
    };
    let active_slot = state
        .akasha
        .active_slot
        .lock()
        .ok()
        .and_then(|s| s.map(|slot| slot.label().to_lowercase()));
    let active_model = state
        .akasha
        .active_model_id
        .lock()
        .ok()
        .and_then(|m| m.clone());

    // FONTOS: a következőket KÜLÖN `let`-ekbe szedjük, mert egy struct-literalban
    // a temporary MutexGuard a teljes utasítás végéig él. Ha itt `port: *lock()` és
    // `base_url: base_url()` (ami belül megint port-ot lockol) ugyanabban a struct-
    // initializerben szerepelne, a második lock ugyanazon a thread-en deadlock-ot
    // okozna - std::sync::Mutex nem reentráns. Ez korábban örökre megfogta a
    // startup-kori auto-start akasha_status hívást, és emiatt minden későbbi
    // base_url() (pl. az akasha_chat-ből) is örökre lógott.
    let port_val = *state.akasha.port.lock().map_err(|e| e.to_string())?;
    let base_url_val = state.akasha.base_url();
    let error_val = state
        .akasha
        .last_error
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    let status_val = state.akasha.get_status();
    let throttle_val = state.throttle.current_level();

    Ok(AkashaStatusResponse {
        status: status_val,
        port: port_val,
        base_url: base_url_val,
        error: error_val,
        active_slot,
        active_model,
        throttle_level: throttle_val,
        hardware,
    })
}

async fn ensure_akasha_running(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<(), String> {
    if state.akasha.get_status() == AkashaStatus::Ready {
        // #region agent log
        debug_log::dlog(
            "lib.rs:ensure_akasha_running",
            "H1",
            "already ready, skipping",
            serde_json::json!({}),
        );
        // #endregion
        return Ok(());
    }

    let (launch_root, cfg, runtime) = {
        let paths = state.paths.lock().map_err(|e| e.to_string())?;
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        (
            PathBuf::from(&paths.launch_root),
            cfg.akasha.clone(),
            paths.runtime_llama.clone(),
        )
    };

    // #region agent log
    let resolved_models_dir = cfg.models_dir_path(&launch_root);
    let eco_resolved = cfg.arsenal_file_path(&launch_root, &cfg.arsenal.eco);
    debug_log::dlog(
        "lib.rs:ensure_akasha_running",
        "H1",
        "starting AKASHA",
        serde_json::json!({
            "launch_root": launch_root.display().to_string(),
            "runtime_path": runtime,
            "runtime_exists": std::path::Path::new(&runtime).exists(),
            "models_dir_raw": cfg.models_dir,
            "models_dir_resolved": resolved_models_dir.display().to_string(),
            "models_dir_resolved_exists": resolved_models_dir.exists(),
            "arsenal_eco": cfg.arsenal.eco,
            "arsenal_brain": cfg.arsenal.brain,
            "arsenal_creative": cfg.arsenal.creative,
            "eco_resolved": eco_resolved.display().to_string(),
            "eco_exists": eco_resolved.exists(),
            "host": cfg.host,
            "port_cfg": cfg.port,
            "n_ctx": cfg.n_ctx,
            "n_threads": cfg.n_threads,
            "models_max": cfg.models_max,
        }),
    );
    // #endregion

    migrate_eco_model(&launch_root);

    let router_result = state
        .akasha
        .start_router(&launch_root, &cfg, &runtime)
        .await;

    // #region agent log
    match &router_result {
        Ok(port) => debug_log::dlog(
            "lib.rs:ensure_akasha_running",
            "H1",
            "start_router OK",
            serde_json::json!({ "port": port, "base_url": state.akasha.base_url() }),
        ),
        Err(e) => debug_log::dlog(
            "lib.rs:ensure_akasha_running",
            "H1",
            "start_router ERROR",
            serde_json::json!({ "error": e }),
        ),
    }
    // #endregion

    router_result?;

    state.throttle.set_child_pid(state.akasha.child_pid());

    // FONTOS - RAM-takarékos viselkedés:
    // KORÁBBAN itt eco-modell preload történt, ami az app indítása UTÁN
    // azonnal 5.5 GB-ot betöltött a RAM-ba akkor is, ha a user még
    // semmit nem kérdezett. Ezt megszüntettük.
    //
    // Jelenlegi viselkedés:
    //   1. App indul → llama-server router elindul (~50 MB router-only)
    //   2. SEMMI modell nincs RAM-ban - a user nyugodtan használhat
    //      más programokat
    //   3. User üzenetet küld → `akasha_chat` meghívja az
    //      `ensure_model_loaded`-ot a megfelelő slot-modell-fájlra
    //   4. Válasz után az `unload_model_after_response` config
    //      (alapból `true`) automatikusan kiakasztja → vissza a ~50 MB-ra
    //
    // Tehát a router üresen, kicsi RAM-mal vár; a tényleges modell
    // CSAK akkor van bent amíg a user éppen választ vár.
    debug_log::dlog(
        "lib.rs:ensure_akasha_running",
        "H12",
        "router fut, eco preload kihagyva (RAM-takarékos start-up)",
        serde_json::json!({ "base_url": state.akasha.base_url() }),
    );

    Ok(())
}

#[tauri::command]
async fn akasha_start(state: State<'_, AppState>, app: AppHandle) -> Result<AkashaStatusResponse, String> {
    ensure_akasha_running(&app, &state).await?;
    akasha_status(state).await
}

#[tauri::command]
fn akasha_stop(state: State<'_, AppState>) {
    state.akasha.stop();
    state.throttle.set_child_pid(None);
}

/// A felhasználói Stop-gomb hívja meg generálás közben - beállítja a cancel
/// flag-et, amit a `stream_chat` minden chunk után figyel és kilép, megőrizve
/// az addig generált szöveget.
#[tauri::command]
fn akasha_cancel_generation(state: State<'_, AppState>) {
    state.akasha.request_cancel();
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AkashaChatArgs {
    messages: Vec<ChatMessage>,
    use_memory: bool,
    /// Ha `true`, a backend mielőtt elküldené a kérdést a modellnek,
    /// lekér DDG-ről egy keresést a friss user-üzenetre, és az
    /// eredményeket a system promptba injektálja. Alapból false → app
    /// 100%-ban offline marad.
    #[serde(default)]
    use_web: bool,
    /// Opcionális kényszerített slot a felhasználói dropdown-ból:
    /// "eco" / "brain" / "creative". Ha `None` (alapeset = "AUTO"),
    /// a `route_prompt` keyword-alapján dönt. Ha valamelyiket megadja
    /// a frontend, MINDIG azt a slotot/modellt használjuk.
    #[serde(default)]
    force_slot: Option<String>,
    /// Opcionális - ha megadva, az üzenetek automatikusan ehhez a beszélgetéshez perzisztálódnak.
    #[serde(default)]
    chat_id: Option<String>,
    /// Csak akkor használjuk, ha a `chat_id`-hez tartozó beszélgetés még nem létezik.
    #[serde(default)]
    chat_title: Option<String>,
}

/// "eco" / "brain" / "creative" → `AkashaSlot`. Bármi mást None-ként
/// kezelünk (visszaesés AUTO-ra).
fn parse_force_slot(s: Option<&str>) -> Option<AkashaSlot> {
    match s.map(|s| s.to_lowercase()).as_deref() {
        Some("eco") => Some(AkashaSlot::Eco),
        Some("brain") => Some(AkashaSlot::Brain),
        Some("creative") => Some(AkashaSlot::Creative),
        _ => None,
    }
}

#[tauri::command]
async fn akasha_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: AkashaChatArgs,
) -> Result<String, String> {
    // #region agent log
    debug_log::dlog(
        "lib.rs:akasha_chat",
        "H6",
        "entry",
        serde_json::json!({
            "messages_count": payload.messages.len(),
            "use_memory": payload.use_memory,
            "chat_id_present": payload.chat_id.is_some(),
        }),
    );
    // #endregion

    ensure_akasha_running(&app, &state).await?;

    // #region agent log
    debug_log::dlog(
        "lib.rs:akasha_chat",
        "H6",
        "after ensure_akasha_running",
        serde_json::json!({}),
    );
    // #endregion

    let base = state
        .akasha
        .base_url()
        .ok_or_else(|| "AKASHA nincs elindítva.".to_string())?;

    // #region agent log
    debug_log::dlog(
        "lib.rs:akasha_chat",
        "H6",
        "after base_url",
        serde_json::json!({ "base": base }),
    );
    // #endregion

    let user_msg = payload
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let (launch_root, akasha_cfg, throttle_cfg, poll_ms) = {
        let paths = state.paths.lock().map_err(|e| e.to_string())?;
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        (
            PathBuf::from(&paths.launch_root),
            cfg.akasha.clone(),
            cfg.akasha.throttle.clone(),
            cfg.akasha.throttle.poll_interval_ms,
        )
    };

    // #region agent log
    debug_log::dlog(
        "lib.rs:akasha_chat",
        "H6",
        "before hw lock #1",
        serde_json::json!({}),
    );
    // #endregion
    let hw_snap = {
        let mut hw = state.hardware.lock().map_err(|e| e.to_string())?;
        hw.snapshot()
    };
    // #region agent log
    debug_log::dlog(
        "lib.rs:akasha_chat",
        "H6",
        "after hw snapshot",
        serde_json::json!({ "available_ram_mb": hw_snap.available_ram_mb }),
    );
    // #endregion

    let is_critical = {
        let mut hw = state.hardware.lock().map_err(|e| e.to_string())?;
        hw.is_critical(
            throttle_cfg.ram_critical_mb,
            throttle_cfg.cpu_critical_percent,
        )
    };

    // Ha a felhasználó a UI-ban manuálisan választott slotot ("eco" / "brain"
    // / "creative"), MINDIG azt használjuk, kihagyva a keyword-routert.
    // Csak ha "AUTO" (None) van, hívjuk a route_prompt-ot.
    let forced = parse_force_slot(payload.force_slot.as_deref());
    let route = if let Some(slot) = forced {
        let model_filename = match slot {
            AkashaSlot::Eco => akasha_cfg.arsenal.eco.clone(),
            AkashaSlot::Brain => akasha_cfg.arsenal.brain.clone(),
            AkashaSlot::Creative => akasha_cfg.arsenal.creative.clone(),
        };
        akasha::router::RouteResult {
            slot,
            model_filename,
            // A manuális választást nem írjuk felül a resource-throttling-gal -
            // a user explicit kérése felülír mindent.
            resource_limited: false,
        }
    } else {
        route_prompt(&RouteInput {
            prompt: &user_msg,
            available_ram_mb: hw_snap.available_ram_mb,
            is_critical,
            launch_root: &launch_root,
            akasha: &akasha_cfg,
        })
    };

    // #region agent log
    debug_log::dlog(
        "lib.rs:akasha_chat",
        "H3",
        "route_prompt result",
        serde_json::json!({
            "slot": route.slot.label(),
            "model_filename": route.model_filename,
            "resource_limited": route.resource_limited,
            "user_msg_len": user_msg.len(),
            "available_ram_mb": hw_snap.available_ram_mb,
            "is_critical": is_critical,
        }),
    );
    // #endregion

    let _ = state
        .throttle
        .evaluate(&hw_snap, &throttle_cfg, akasha_cfg.n_threads);
    state.throttle.set_child_pid(state.akasha.child_pid());

    // === Adaptív Védelmi Protokoll - Fázis 2 ===
    // Számoljuk újra a tier-t a friss snapshot alapján. Ha lecsökkent a
    // detektált tier (pl. valaki közben megnyitott Chrome-ot 30 tabbal),
    // tier-specifikus modell-fájlra váltunk, ha létezik.
    let perf_cfg = state.config.lock().map_err(|e| e.to_string())?.performance.clone();
    let perf_profile = akasha::perf::compute_profile(
        &hw_snap,
        perf_cfg.forced_tier.as_deref(),
        perf_cfg.hardware_protection_enabled,
    );
    // #region agent log
    debug_log::dlog(
        "lib.rs:akasha_chat",
        "H9",
        "perf tier check",
        serde_json::json!({
            "detected": perf_profile.detected_tier.key(),
            "effective": perf_profile.effective_tier.key(),
            "override_active": perf_profile.override_active,
            "available_ram_gb": perf_profile.available_ram_gb,
        }),
    );
    // #endregion
    // Ha BLOCKED tier-re esett és a védelem be van kapcsolva: érdemes
    // visszadobni egy user-friendly hibát, nem hagyni hogy megpróbálja
    // betölteni a modellt és lefagyasztani a gépet.
    if perf_profile.effective_tier == akasha::perf::Tier::Blocked
        && perf_cfg.hardware_protection_enabled
    {
        let err = perf_profile.message.clone();
        let _ = app.emit("akasha-error", &err);
        return Err(err);
    }
    // Jelezzük a frontend felé a friss profilt (a UI esetleg banner-t
    // mutat ha lecsökkent a tier).
    let _ = app.emit("akasha-perf-profile", &perf_profile);

    // Megjegyzés: tier-specifikus fájl-csere a Fázis 3-ban jön majd, amikor
    // a startup-wizard controlled módon újraindítja a router-t. Most a tier
    // CSAK felhasználói figyelmeztetést + BLOCKED védelmet ad - a fájl maga
    // marad a default (Pro tier).
    let model_path = akasha_cfg.arsenal_file_path(&launch_root, &route.model_filename);
    if !model_path.exists() {
        let err = format!(
            "Modell fájl hiányzik: {}. Futtasd: scripts/fetch-akasha-arsenal.ps1",
            route.model_filename
        );
        let _ = app.emit("akasha-error", &err);
        return Err(err);
    }
    if std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0) < 50_000_000 {
        let err = format!(
            "Modell fájl sérült vagy hiányos (túl kicsi): {}. Töltsd le újra.",
            route.model_filename
        );
        let _ = app.emit("akasha-error", &err);
        return Err(err);
    }

    // A llama-server router módja preset-néven kezeli a modelleket (kiterjesztés nélkül).
    // Ezt a nevet küldjük a /models/load és a /v1/chat/completions végpontokra is.
    let model_preset = route
        .model_filename
        .strip_suffix(".gguf")
        .unwrap_or(&route.model_filename)
        .to_string();

    // #region agent log
    debug_log::dlog(
        "lib.rs:akasha_chat",
        "H3",
        "about to ensure_model_loaded",
        serde_json::json!({
            "model_preset": model_preset,
            "base_url": base,
            "model_path": model_path.display().to_string(),
            "model_path_exists": model_path.exists(),
        }),
    );
    // #endregion

    let load_res = ensure_model_loaded(&app, &base, &model_preset).await;
    // #region agent log
    debug_log::dlog(
        "lib.rs:akasha_chat",
        "H3",
        "ensure_model_loaded returned",
        serde_json::json!({ "ok": load_res.is_ok(), "err": load_res.as_ref().err().cloned() }),
    );
    // #endregion
    load_res.map_err(|e| {
        let _ = app.emit("akasha-error", &e);
        e
    })?;
    state
        .akasha
        .set_active(route.slot, &route.model_filename);

    // === Beszélgetés perzisztencia (új user üzenetek mentése) ===
    if let Some(chat_id) = payload.chat_id.as_deref() {
        let title = payload
            .chat_title
            .clone()
            .unwrap_or_else(|| derive_title_from_messages(&payload.messages));
        if let Ok(store_guard) = state.chats.lock() {
            if let Some(store) = store_guard.as_ref() {
                let _ = store.chat_ensure(chat_id, &title);
                let existing = store
                    .chat_get(chat_id)
                    .map(|c| c.messages.len())
                    .unwrap_or(0);
                // A payload csak `user`/`assistant` üzeneteket tartalmaz - system promptot a backend tesz hozzá.
                for msg in payload.messages.iter().skip(existing) {
                    let _ = store.message_append(chat_id, &msg.role, &msg.content);
                }
            }
        }
    }

    let rag_context = if payload.use_memory {
        let mem_cfg = state.config.lock().map_err(|e| e.to_string())?.memory.clone();
        if let Ok(mem) = state.memory.lock() {
            if let Some(store) = mem.as_ref() {
                build_rag_context(store, &user_msg, &mem_cfg)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Opcionális web-keresés - csak ha a frontend a "globe" toggle-t
    // aktiválta. Ez az EGYETLEN olyan hely az appban ahol kifelé megy net-
    // forgalom (a model letöltésen kívül). A felhasználó explicit opt-in
    // alapján; alapból false.
    let web_context = if payload.use_web {
        let _ = app.emit("akasha-web-searching", &user_msg);
        match web_search::search(&user_msg, 5).await {
            Ok(results) => {
                let _ = app.emit("akasha-web-results", &results);
                Some(web_search::format_for_prompt(&user_msg, &results))
            }
            Err(e) => {
                let _ = app.emit(
                    "akasha-web-error",
                    format!("Web keresés sikertelen: {e}"),
                );
                None
            }
        }
    } else {
        None
    };

    // === Memória-kártyák (Gemini-stílusú "saved info") ===
    // Az engedélyezett kártyákat tömör formában (~600 token max) a system
    // promptba illesztjük - MINDEN üzenetnél, mert ezek általában apró,
    // de mindig releváns user-tények/personality-irányelvek.
    // A token-limit megakadályozza hogy egy 200 soros kártya elfedje a
    // beszélgetést. A 600 token (~2400 karakter) bőven elég 10-15 átlagos
    // kártyához, és nem eszi meg a context-window jelentős részét.
    const MEMORY_NOTES_BUDGET_TOKENS: usize = 600;
    let notes_context: Option<String> = {
        let guard = state.memory_notes.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(store) => match store.list_enabled() {
                Ok(notes) => memory_notes::store::format_for_prompt(
                    &notes,
                    MEMORY_NOTES_BUDGET_TOKENS,
                ),
                Err(_) => None,
            },
            None => None,
        }
    };

    // Memória-kártyák + RAG + web context egyetlen "extra context" stringben
    // a stream_chat felé - az a system promptba illeszti. A memória-kártyák
    // legelőre kerülnek, mert ezek "ki vagyok, hogyan beszélgess velem"-tartalom.
    let combined_context = {
        let mut parts: Vec<String> = Vec::new();
        if let Some(n) = notes_context {
            parts.push(n);
        }
        if let Some(r) = rag_context {
            parts.push(r);
        }
        if let Some(w) = web_context {
            parts.push(w);
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(""))
        }
    };

    let throttle = Arc::clone(&state.throttle);
    let eta = Arc::clone(&state.eta);
    let started = Instant::now();

    // Új generálás → cancel flag nullázása. Innen a frontend Stop-gombja
    // tudja igazra állítani, hogy a stream a következő chunk után megálljon.
    state.akasha.reset_cancel();
    let cancel = state.akasha.cancel_handle();

    // A user neve a DB-ből - beleinjektáljuk a system promptba, hogy AKASHA
    // személyesen szólíthassa (ha a user megadta a first-run modálban).
    // Egyúttal lekérdezzük: MA ez az első chat-e? Ha igen, AKASHA köszöntse.
    let (user_name, is_first_today) = {
        let store_guard = state.profile.lock().map_err(|e| e.to_string())?;
        match store_guard.as_ref() {
            Some(s) => {
                let name = s.get_display_name().ok();
                let first = s.check_and_mark_daily_first_chat().unwrap_or(false);
                (name, first)
            }
            None => (None, false),
        }
    };

    let stream_result = stream_chat(
        app.clone(),
        &base,
        &model_preset,
        payload.messages.clone(),
        StreamContext {
            slot: route.slot,
            model_id: model_preset.clone(),
            resource_limited: route.resource_limited,
        },
        combined_context,
        user_name,
        is_first_today,
        throttle,
        eta,
        throttle_cfg,
        poll_ms,
        state.akasha.child_pid(),
        cancel,
    )
    .await;

    // === RAM-takarékos mód: minden válasz UTÁN kiakasztjuk a modellt ===
    // Ezt MINDIG futtatjuk, mind sikeres, mind hibás stream után, hogy
    // ne maradjon 5+ GB RAM-ban a modell ha a user nem chat-el éppen.
    // A hiba (ha volt) ezután továbbpropagáljuk.
    if perf_cfg.unload_model_after_response {
        match akasha::arsenal::unload_model(&base, &model_preset).await {
            Ok(()) => {
                // Aktív slot reset - a UI ne mutassa hogy van bent modell.
                state.akasha.clear_active();
            }
            Err(e) => {
                // Best-effort: ha az unload nem ment, csak logoljuk.
                debug_log::dlog(
                    "lib.rs:akasha_chat",
                    "H11",
                    "auto-unload sikertelen (best-effort)",
                    serde_json::json!({ "model_id": model_preset, "error": e }),
                );
            }
        }
    }

    let full = stream_result?;

    // Mentjük az asszisztens választ is.
    if let Some(chat_id) = payload.chat_id.as_deref() {
        if !full.is_empty() {
            if let Ok(store_guard) = state.chats.lock() {
                if let Some(store) = store_guard.as_ref() {
                    let _ = store.message_append(chat_id, "assistant", &full);
                }
            }
        }
    }

    let secs = started.elapsed().as_secs().max(1);
    let domain = slot_to_domain(route.slot);
    if let Ok(mut p) = state.profile.lock() {
        if let Some(profile) = p.as_mut() {
            let _ = profile.record_session(
                UsageDomain::from_task(domain_to_string(domain)),
                secs,
            );
            let _ = profile.record_event("message_sent", None);
        }
    }

    if let Ok(mem) = state.memory.lock() {
        if let Some(store) = mem.as_ref() {
            if let Ok(chunks) = store.chunk_count() {
                if let Ok(mut p) = state.profile.lock() {
                    if let Some(profile) = p.as_mut() {
                        let new_badges = profile.evaluate_badges(chunks)?;
                        for badge_id in new_badges {
                            let _ = app.emit("badge-unlocked", badge_id);
                        }
                    }
                }
            }
        }
    }

    Ok(full)
}

#[tauri::command]
async fn memory_import(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<memory::DocumentInfo, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?.memory.clone();
    let store = state.memory.lock().map_err(|e| e.to_string())?;
    let mem = store
        .as_ref()
        .ok_or_else(|| "Memória nincs inicializálva.".to_string())?;
    let doc = mem.import_file(PathBuf::from(&file_path).as_path(), &cfg)?;
    let chunks = mem.chunk_count()?;
    if let Ok(mut p) = state.profile.lock() {
        if let Some(profile) = p.as_mut() {
            let _ = profile.evaluate_badges(chunks);
        }
    }
    Ok(doc)
}

#[tauri::command]
fn memory_list(state: State<'_, AppState>) -> Result<Vec<memory::DocumentInfo>, String> {
    let store = state.memory.lock().map_err(|e| e.to_string())?;
    store
        .as_ref()
        .ok_or_else(|| "Memória nincs inicializálva.".to_string())?
        .list_documents()
}

#[tauri::command]
fn memory_search(state: State<'_, AppState>, query: String) -> Result<Vec<String>, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?.memory.clone();
    let store = state.memory.lock().map_err(|e| e.to_string())?;
    store
        .as_ref()
        .ok_or_else(|| "Memória nincs inicializálva.".to_string())?
        .search(&query, cfg.top_k)
}

#[tauri::command]
fn memory_delete(state: State<'_, AppState>, doc_id: String) -> Result<(), String> {
    let store = state.memory.lock().map_err(|e| e.to_string())?;
    store
        .as_ref()
        .ok_or_else(|| "Memória nincs inicializálva.".to_string())?
        .delete_document(&doc_id)
}

// ===========================================================================
//  MEMÓRIA-KÁRTYÁK (személyes információ + személyiség, Gemini-stílus)
// ===========================================================================

fn with_memory_notes<F, R>(state: &State<'_, AppState>, f: F) -> Result<R, String>
where
    F: FnOnce(&memory_notes::MemoryNotesStore) -> Result<R, String>,
{
    let guard = state.memory_notes.lock().map_err(|e| e.to_string())?;
    let store = guard
        .as_ref()
        .ok_or_else(|| "Memória-kártyák tára nincs inicializálva.".to_string())?;
    f(store)
}

#[tauri::command]
fn memory_notes_list(
    state: State<'_, AppState>,
) -> Result<Vec<memory_notes::MemoryNote>, String> {
    with_memory_notes(&state, |s| s.list_all())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryNoteCreateArgs {
    title: String,
    content: String,
}

#[tauri::command]
fn memory_notes_create(
    state: State<'_, AppState>,
    args: MemoryNoteCreateArgs,
) -> Result<memory_notes::MemoryNote, String> {
    with_memory_notes(&state, |s| s.create(&args.title, &args.content))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryNoteUpdateArgs {
    id: String,
    title: String,
    content: String,
}

#[tauri::command]
fn memory_notes_update(
    state: State<'_, AppState>,
    args: MemoryNoteUpdateArgs,
) -> Result<(), String> {
    with_memory_notes(&state, |s| s.update(&args.id, &args.title, &args.content))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryNoteToggleArgs {
    id: String,
    enabled: bool,
}

#[tauri::command]
fn memory_notes_toggle(
    state: State<'_, AppState>,
    args: MemoryNoteToggleArgs,
) -> Result<(), String> {
    with_memory_notes(&state, |s| s.toggle_enabled(&args.id, args.enabled))
}

#[tauri::command]
fn memory_notes_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    with_memory_notes(&state, |s| s.delete(&id))
}

// ===========================================================================
//  FIRST-RUN DOWNLOADER (modellek + llama-server runtime)
// ===========================================================================

#[tauri::command]
fn check_setup_status(
    state: State<'_, AppState>,
) -> Result<downloader::SetupStatus, String> {
    let paths = state.paths.lock().map_err(|e| e.to_string())?;
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let runtime_path = PathBuf::from(&paths.runtime_llama);
    let models_dir = PathBuf::from(&paths.models_akasha);
    Ok(downloader::store::check_setup_status(
        &runtime_path,
        &models_dir,
        &cfg.akasha.arsenal.eco,
        &cfg.akasha.arsenal.brain,
        &cfg.akasha.arsenal.creative,
    ))
}

#[tauri::command]
async fn check_online() -> bool {
    downloader::store::is_online().await
}

/// Letölti a megadott komponenst (`runtime` / `eco` / `brain` / `creative`).
/// Progress event-eket emit-tál: `download-start`, `download-progress`,
/// `download-done`. Ha a fájl már létezik (és teljes), azonnal `done`-t emit-ál.
#[tauri::command]
async fn download_component(
    app: AppHandle,
    state: State<'_, AppState>,
    component: String,
) -> Result<(), String> {
    let (runtime_dir, models_dir, eco_file, brain_file, creative_file) = {
        let paths = state.paths.lock().map_err(|e| e.to_string())?;
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        let runtime_path = PathBuf::from(&paths.runtime_llama);
        // A runtime mappa az llama-server bináris szülő-mappája
        let runtime_dir = runtime_path
            .parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "Runtime path szülő-mappa hiba".to_string())?;
        (
            runtime_dir,
            PathBuf::from(&paths.models_akasha),
            cfg.akasha.arsenal.eco.clone(),
            cfg.akasha.arsenal.brain.clone(),
            cfg.akasha.arsenal.creative.clone(),
        )
    };

    match component.as_str() {
        "runtime" => downloader::store::download_runtime(&app, runtime_dir).await,
        slot @ ("eco" | "brain" | "creative") => {
            let src = downloader::store::model_source(slot)
                .ok_or_else(|| format!("Ismeretlen slot: {slot}"))?;
            let filename = match slot {
                "eco" => eco_file,
                "brain" => brain_file,
                "creative" => creative_file,
                _ => unreachable!(),
            };
            let target = models_dir.join(filename);
            downloader::store::download_model(&app, slot, target, src.repo, src.file).await
        }
        _ => Err(format!("Ismeretlen komponens: {component}")),
    }
}

#[tauri::command]
fn profile_get(state: State<'_, AppState>) -> Result<profile::db::ProfileData, String> {
    let chunks = state
        .memory
        .lock()
        .ok()
        .and_then(|m| m.as_ref().and_then(|s| s.chunk_count().ok()))
        .unwrap_or(0);
    let store = state.profile.lock().map_err(|e| e.to_string())?;
    store
        .as_ref()
        .ok_or_else(|| "Profil nincs inicializálva.".to_string())?
        .get_profile(chunks)
}

#[tauri::command]
fn profile_update_name(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let store = state.profile.lock().map_err(|e| e.to_string())?;
    store
        .as_ref()
        .ok_or_else(|| "Profil nincs inicializálva.".to_string())?
        .set_display_name(&name)?;
    if let Ok(mut cfg) = state.config.lock() {
        cfg.profile.display_name = name;
        let paths = state.paths.lock().map_err(|e| e.to_string())?.clone();
        let _ = save_config(&paths, &cfg);
    }
    Ok(())
}

#[tauri::command]
fn profile_record_event(state: State<'_, AppState>, kind: String) -> Result<(), String> {
    let store = state.profile.lock().map_err(|e| e.to_string())?;
    store
        .as_ref()
        .ok_or_else(|| "Profil nincs inicializálva.".to_string())?
        .record_event(&kind, None)
}

#[tauri::command]
fn profile_set_birthday(
    state: State<'_, AppState>,
    month: u32,
    day: u32,
) -> Result<(), String> {
    let store = state.profile.lock().map_err(|e| e.to_string())?;
    store
        .as_ref()
        .ok_or_else(|| "Profil nincs inicializálva.".to_string())?
        .set_birthday(month, day)
}

#[tauri::command]
fn profile_get_setup_status(
    state: State<'_, AppState>,
) -> Result<profile::db::ProfileSetupStatus, String> {
    let store = state.profile.lock().map_err(|e| e.to_string())?;
    store
        .as_ref()
        .ok_or_else(|| "Profil nincs inicializálva.".to_string())?
        .get_setup_status()
}

#[tauri::command]
fn profile_check_birthday(
    state: State<'_, AppState>,
) -> Result<profile::db::BirthdayCheck, String> {
    let store = state.profile.lock().map_err(|e| e.to_string())?;
    store
        .as_ref()
        .ok_or_else(|| "Profil nincs inicializálva.".to_string())?
        .check_birthday_today()
}

#[tauri::command]
fn profile_mark_birthday_greeted(state: State<'_, AppState>) -> Result<(), String> {
    let store = state.profile.lock().map_err(|e| e.to_string())?;
    store
        .as_ref()
        .ok_or_else(|| "Profil nincs inicializálva.".to_string())?
        .mark_birthday_greeted()
}

/// Avatar PNG mentése a `data/profile/avatar.png`-be (base64 PNG bemenetből).
/// A frontend a crop-kanvasz `toDataURL()`-jét küldi base64-encoded PNG-ként.
#[tauri::command]
fn profile_save_avatar(
    state: State<'_, AppState>,
    png_base64: String,
) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let bytes = B64
        .decode(png_base64.trim())
        .map_err(|e| format!("Base64 decode hiba: {e}"))?;
    let profile_dir = {
        let paths = state.paths.lock().map_err(|e| e.to_string())?;
        std::path::PathBuf::from(&paths.profile_dir)
    };
    std::fs::create_dir_all(&profile_dir).map_err(|e| e.to_string())?;
    let avatar_path = profile_dir.join("avatar.png");
    std::fs::write(&avatar_path, bytes).map_err(|e| e.to_string())?;
    let avatar_str = avatar_path.display().to_string();
    {
        let store = state.profile.lock().map_err(|e| e.to_string())?;
        store
            .as_ref()
            .ok_or_else(|| "Profil nincs inicializálva.".to_string())?
            .set_avatar_path(Some(&avatar_str))?;
    }
    Ok(avatar_str)
}

/// Beolvas egy képfájlt és visszaadja `data:image/<ext>;base64,...` URL-ként.
/// A frontend ezzel tudja megjeleníteni a kiválasztott fájlt az <img> tagben
/// anélkül hogy az asset protokollra kéne támaszkodnia (ami a webview / CSP
/// / capability beállításoktól függ - kevésbé megbízható).
#[tauri::command]
fn read_image_data_url(path: String) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let bytes = std::fs::read(&path).map_err(|e| format!("Fájl olvasás hiba: {e}"))?;
    let mime = match std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "image/png",
    };
    let b64 = B64.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

/// A felhasználó mentett avatarját adja vissza data URL-ként a profilból,
/// vagy None-t ha nincs avatar.
#[tauri::command]
fn profile_get_avatar_data_url(
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let avatar_path: Option<String> = {
        let store_guard = state.profile.lock().map_err(|e| e.to_string())?;
        store_guard.as_ref().and_then(|s| {
            s.get_profile(0)
                .ok()
                .and_then(|p| p.avatar_path)
        })
    };
    match avatar_path {
        Some(path) if std::path::Path::new(&path).exists() => {
            read_image_data_url(path).map(Some)
        }
        _ => Ok(None),
    }
}

#[tauri::command]
fn profile_clear_avatar(state: State<'_, AppState>) -> Result<(), String> {
    let profile_dir = {
        let paths = state.paths.lock().map_err(|e| e.to_string())?;
        std::path::PathBuf::from(&paths.profile_dir)
    };
    let avatar_path = profile_dir.join("avatar.png");
    let _ = std::fs::remove_file(&avatar_path);
    let store = state.profile.lock().map_err(|e| e.to_string())?;
    store
        .as_ref()
        .ok_or_else(|| "Profil nincs inicializálva.".to_string())?
        .set_avatar_path(None)
}

// ===========================================================================
// CHATS + GROUPS
// ===========================================================================

fn with_chats<F, R>(state: &State<'_, AppState>, f: F) -> Result<R, String>
where
    F: FnOnce(&chats::ChatStore) -> Result<R, String>,
{
    let guard = state.chats.lock().map_err(|e| e.to_string())?;
    let store = guard
        .as_ref()
        .ok_or_else(|| "Beszélgetés-tár nincs inicializálva.".to_string())?;
    f(store)
}

fn derive_title_from_messages(messages: &[ChatMessage]) -> String {
    let first_user = messages.iter().find(|m| m.role == "user");
    match first_user {
        Some(m) => {
            let text = m.content.trim();
            let truncated: String = text.chars().take(40).collect();
            if text.chars().count() > 40 {
                format!("{truncated}…")
            } else if truncated.is_empty() {
                "Új beszélgetés".to_string()
            } else {
                truncated
            }
        }
        None => "Új beszélgetés".to_string(),
    }
}

#[tauri::command]
fn chats_list(state: State<'_, AppState>) -> Result<Vec<ChatPreview>, String> {
    with_chats(&state, |s| s.chats_list())
}

#[tauri::command]
fn chat_get(state: State<'_, AppState>, chat_id: String) -> Result<ChatFull, String> {
    with_chats(&state, |s| s.chat_get(&chat_id))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatCreateArgs {
    chat_id: String,
    title: String,
}

#[tauri::command]
fn chat_create(state: State<'_, AppState>, args: ChatCreateArgs) -> Result<(), String> {
    with_chats(&state, |s| s.chat_ensure(&args.chat_id, &args.title))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRenameArgs {
    chat_id: String,
    title: String,
}

#[tauri::command]
fn chat_rename(state: State<'_, AppState>, args: ChatRenameArgs) -> Result<(), String> {
    with_chats(&state, |s| s.chat_rename(&args.chat_id, &args.title))
}

#[tauri::command]
fn chat_delete(state: State<'_, AppState>, chat_id: String) -> Result<(), String> {
    with_chats(&state, |s| s.chat_delete(&chat_id))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatPinArgs {
    chat_id: String,
    pinned: bool,
}

#[tauri::command]
fn chat_pin(state: State<'_, AppState>, args: ChatPinArgs) -> Result<(), String> {
    with_chats(&state, |s| s.chat_pin(&args.chat_id, args.pinned))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatSetGroupArgs {
    chat_id: String,
    group_id: Option<String>,
}

#[tauri::command]
fn chat_set_group(state: State<'_, AppState>, args: ChatSetGroupArgs) -> Result<(), String> {
    with_chats(&state, |s| {
        s.chat_set_group(&args.chat_id, args.group_id.as_deref())
    })
}

#[tauri::command]
fn chat_search(state: State<'_, AppState>, query: String) -> Result<Vec<ChatPreview>, String> {
    with_chats(&state, |s| s.chat_search(&query, 50))
}

#[tauri::command]
fn groups_list(state: State<'_, AppState>) -> Result<Vec<Group>, String> {
    with_chats(&state, |s| s.groups_list())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupCreateArgs {
    name: String,
    color: String,
    icon: String,
}

#[tauri::command]
fn group_create(state: State<'_, AppState>, args: GroupCreateArgs) -> Result<Group, String> {
    with_chats(&state, |s| s.group_create(&args.name, &args.color, &args.icon))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupUpdateArgs {
    group_id: String,
    name: Option<String>,
    color: Option<String>,
    icon: Option<String>,
}

#[tauri::command]
fn group_update(state: State<'_, AppState>, args: GroupUpdateArgs) -> Result<(), String> {
    with_chats(&state, |s| {
        s.group_update(
            &args.group_id,
            args.name.as_deref(),
            args.color.as_deref(),
            args.icon.as_deref(),
        )
    })
}

#[tauri::command]
fn group_delete(state: State<'_, AppState>, group_id: String) -> Result<(), String> {
    with_chats(&state, |s| s.group_delete(&group_id))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (paths, config) = init_config().expect("Portable layout inicializálás sikertelen");
    let app_state = AppState::new(paths, config).expect("App state inicializálás sikertelen");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(app_state)
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Some(state) = handle.try_state::<AppState>() {
                    let app = handle.clone();
                    let _ = akasha_start(state, app).await;
                }
            });
            Ok(())
        })
        // === Process-leak védelem: explicit takarítás bezáráskor ===
        // A felhasználó normál bezárása (X gomb, Alt+F4) GRACEFUL módon
        // leállítja az llama-server child-eket. Ha valami másra crash-elne
        // az atman.exe, a Windows Job Object (server.rs) safety-net-ként
        // még mindig megöli a child-eket OS-szinten.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Some(state) = window.try_state::<AppState>() {
                    state.akasha.stop();
                    state.throttle.set_child_pid(None);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            get_app_paths,
            get_config,
            save_app_config,
            akasha_hardware,
            get_hardware_profile,
            akasha_status,
            akasha_start,
            akasha_stop,
            akasha_cancel_generation,
            akasha_chat,
            memory_import,
            memory_list,
            memory_search,
            memory_delete,
            memory_notes_list,
            memory_notes_create,
            memory_notes_update,
            memory_notes_toggle,
            memory_notes_delete,
            check_setup_status,
            check_online,
            download_component,
            profile_get,
            profile_update_name,
            profile_record_event,
            profile_set_birthday,
            profile_get_setup_status,
            profile_check_birthday,
            profile_mark_birthday_greeted,
            profile_save_avatar,
            profile_clear_avatar,
            read_image_data_url,
            profile_get_avatar_data_url,
            chats_list,
            chat_get,
            chat_create,
            chat_rename,
            chat_delete,
            chat_pin,
            chat_set_group,
            chat_search,
            groups_list,
            group_create,
            group_update,
            group_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
