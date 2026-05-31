use super::estimate::{EtaEstimator, GenTickEvent};
use super::throttle::{DynamicThrottle, ThrottleStatus};
use super::types::{slot_system_prompt, AkashaSlot, ChatMessage};
use crate::portable::config::AkashaThrottleConfig;
use chrono::Datelike;
use futures_util::StreamExt;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
    /// Reasoning-modellek (pl. DeepSeek-R1 alapú GGUF-ok) a thinking tokeneket
    /// ide írják, és a `content` üres marad. Ha az `enable_thinking=false`
    /// switch nem hatékony, akkor ezt használjuk fallback-ként, hogy
    /// a felhasználó lássa a választ.
    #[serde(default)]
    reasoning_content: Option<String>,
}

pub struct StreamContext {
    pub slot: AkashaSlot,
    pub model_id: String,
    pub resource_limited: bool,
}

pub async fn stream_chat(
    app: AppHandle,
    base_url: &str,
    model_id: &str,
    messages: Vec<ChatMessage>,
    ctx: StreamContext,
    rag_context: Option<String>,
    // A felhasználó megadott neve a system promptba - ha None/üres, AKASHA
    // nem fogja semmilyen néven szólítani (személytelen, semleges).
    user_display_name: Option<String>,
    // Ha igaz, MA ez az első chat-üzenete a usernek → AKASHA köszöntse
    // egyszer az aznap első válaszában.
    is_daily_first_chat: bool,
    throttle: Arc<DynamicThrottle>,
    eta: Arc<EtaEstimator>,
    throttle_cfg: AkashaThrottleConfig,
    poll_interval_ms: u64,
    child_pid: Option<u32>,
    cancel: Arc<AtomicBool>,
    // v0.2.4 - opcionális system prompt extension (a Kód translation-flow
    // ezt használja, hogy a Coder kifejezetten angolul válaszoljon).
    system_prompt_suffix: Option<String>,
    // v0.2.4 - ha true, NEM emittál `akasha-token`/`akasha-thinking-token`
    // event-eket (a frontend csak a `akasha-phase` indikátort látja).
    // A Kód translation-flow első fázisa ezt használja: az angol Coder
    // válasz a felhasználó számára nem látszik, csak a végén jelenik
    // meg a fordított magyar verzió.
    suppress_frontend_tokens: bool,
) -> Result<String, String> {
    throttle.set_child_pid(child_pid);

    let mut system = slot_system_prompt(ctx.slot, ctx.resource_limited);

    // Extra context (RAG memória + opcionális web-keresési eredmény) - ezek
    // a saját formátumukat hozzák magukkal (külön szekció-fejléccel), így
    // csak hozzáfűzzük a system prompthoz.
    if let Some(extra) = rag_context.as_ref() {
        if !extra.is_empty() {
            system.push_str(extra);
        }
    }

    // === Felhasználó neve ===
    // Ha a user megadta, beleinjektáljuk a system promptba, hogy AKASHA
    // személyesen szólíthassa. Ha üres (még nincs first-run modal után),
    // semmilyen néven nem szólít - semleges marad.
    let user_name_trimmed = user_display_name
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "felhasználó");

    if let Some(ref name) = user_name_trimmed {
        system.push_str(&format!(
            "\n\nA FELHASZNÁLÓ NEVE: {name}. \
             Szólítsd néven természetesen, ha indokolt - \
             ne minden mondatban, de pl. üdvözléskor vagy \
             amikor érzelmes/személyes a téma."
        ));
    }

    // Napi első chat - AKASHA röviden köszöntse a usert az aznap első
    // válaszában. NEM lehet hosszú formális köszöntés; csak természetes
    // "szia + miben segítek" hangulat. Aztán térjen rá a kérdésre.
    if is_daily_first_chat {
        let greeting_hint = match user_name_trimmed.as_deref() {
            Some(name) => format!(
                "\n\nMA ELSŐ ÜZENET: ez a mai első üzenet a felhasználótól. \
                 A válaszod ELSŐ MONDATA legyen egy rövid, természetes \
                 köszöntés \"{name}\" néven (pl. \"Szia {name}!\" vagy \
                 \"Jó látni {name}!\"), aztán térj rá a kérdésre. \
                 Egy mondat, ne legyen ünnepélyes - csak emberi nyitás."
            ),
            None => "\n\nMA ELSŐ ÜZENET: ez a mai első üzenet a felhasználótól. \
                     A válaszod ELSŐ MONDATA legyen egy rövid természetes \
                     köszöntés (pl. \"Szia!\"), aztán térj rá a kérdésre. \
                     Egy mondat, ne legyen ünnepélyes."
                .to_string(),
        };
        system.push_str(&greeting_hint);
    }

    // A LLM nem tudja a mai dátumot (nincs órája, csak a tréning idejét tudja).
    // A futási idejű dátumot beinjektáljuk, így a "Mi a mai dátum?" típusú
    // kérdésekre normális választ tud adni.
    //
    // FONTOS: a hét napját MAGYARUL adjuk át, NEM angolul. A `chrono` `%A`
    // formátuma az alapértelmezett (angol) locale-t használja ("Sunday"),
    // és a kis modellek néha mistranslate-elik magyarra ("Kedd", "Csütörtök"
    // vegyesen). Manuális lookup-pal garantáljuk a helyes magyar nap-nevet.
    let now = chrono::Local::now();
    let day_hu = match now.weekday() {
        chrono::Weekday::Mon => "hétfő",
        chrono::Weekday::Tue => "kedd",
        chrono::Weekday::Wed => "szerda",
        chrono::Weekday::Thu => "csütörtök",
        chrono::Weekday::Fri => "péntek",
        chrono::Weekday::Sat => "szombat",
        chrono::Weekday::Sun => "vasárnap",
    };
    let month_hu = match now.month() {
        1 => "január", 2 => "február", 3 => "március", 4 => "április",
        5 => "május", 6 => "június", 7 => "július", 8 => "augusztus",
        9 => "szeptember", 10 => "október", 11 => "november", 12 => "december",
        _ => "?",
    };
    system.push_str(&format!(
        "\n\nA mai dátum: {year}. {month} {day}. ({weekday}), pontos idő \
         {time}. Ez a felhasználó rendszer-órájáról származik - MINDIG \
         erre a dátumra és napra hivatkozz, ha dátumot vagy hetet kérdez. \
         Soha ne találj ki más napot vagy évet.",
        year = now.format("%Y"),
        month = month_hu,
        day = now.day(),
        weekday = day_hu,
        time = now.format("%H:%M"),
    ));

    // v0.2.4 - opcionális system prompt extension. A Kód translation-flow
    // ide egy "answer ONLY in English" instrukciót fűz, hogy a Coder
    // teljesen angolul válaszoljon (a fordító réteg utána magyarra teszi).
    if let Some(suffix) = system_prompt_suffix.as_ref() {
        if !suffix.is_empty() {
            system.push_str("\n\n");
            system.push_str(suffix);
        }
    }

    // (A `rag_context` paramétert már fentebb beolvastuk a system promptba -
    // most már RAG-memória és web-keresési eredmények egyszerre érkezhetnek
    // benne, ezért a régi "Relevant memory" prefix helyett a hívó adja meg
    // a formátumot.)

    let user_len: usize = messages
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| m.content.len())
        .sum();

    let gen_start = eta.estimate_start(ctx.slot, model_id, user_len);
    let _ = app.emit("akasha-gen-start", &gen_start);

    let mut api_messages = vec![ChatMessage {
        role: "system".into(),
        content: system,
    }];
    api_messages.extend(messages);

    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(86400))
        .build()
        .map_err(|e| e.to_string())?;

    // v0.2.2: kibővített inference-paraméterek, hogy elkerüljük a v0.2.1-ben
    // észlelt két nagy hibát:
    //
    //   1. **Generálás-loop**: a Kód modell egyszer már le-generálta a HTML-t,
    //      majd kiírt egy "Szia!"-t és újrakezdte (lásd Áron screenshotja a
    //      Steve Jobs oldalról). Az ok: a modell hallucinált egy stop-token-t,
    //      de a kérésben nem volt explicit `stop`-lista, ezért tovább ment.
    //      A `stop` mező explicit felsorolja a modern modell-chat-templátok
    //      end-of-turn markereit (Qwen/ChatML, Gemma, Llama 3, Phi/GPT).
    //
    //   2. **Túl rövid kód-válasz**: az 1024 token plafon egy közepes HTML
    //      fájlt is csonkolt → felemelve 4096-ra. Az n_ctx=8192 (v0.2.1)
    //      bőven elbírja: 4096 max-out + ~3000 system prompt + ~500 user
    //      üzenet még belefér.
    //
    //   3. **Ismétlés-elkerülés**: a `repeat_penalty` 1.15 → 1.18 és
    //      `frequency_penalty` 0.1, hogy a "ugyanaz a mondat 3x" típusú
    //      hallucinációkat csillapítsuk.
    //
    // Megtartottuk a reasoning-fallback flageket is (`reasoning_effort: "none"`,
    // `enable_thinking: false`) — ezek nem hibáznak ha a modell nem reasoning-
    // típusú, csak a thinking-tokeneket kerülik el.
    let body = serde_json::json!({
        "model": model_id,
        "messages": api_messages,
        "stream": true,
        "temperature": 0.7,
        "max_tokens": 4096,
        "repeat_penalty": 1.18,
        "frequency_penalty": 0.1,
        "presence_penalty": 0.0,
        "stop": [
            // Qwen / ChatML (Qwen 2.5 Coder 7B is ezt használja)
            "<|im_end|>",
            "<|im_start|>",
            // Phi-3.5 chat template
            "<|end|>",
            "<|endoftext|>",
            // Gemma 2 chat template
            "<end_of_turn>",
            "<start_of_turn>",
            // Llama 3 család
            "<|eot_id|>",
        ],
        "reasoning_effort": "none",
        "chat_template_kwargs": { "enable_thinking": false },
    });

    let started = Instant::now();
    let mut last_tick = Instant::now();
    let mut full = String::new();

    // #region agent log
    crate::debug_log::dlog(
        "client.rs:stream_chat",
        "H4",
        "POST /v1/chat/completions",
        serde_json::json!({
            "url": url,
            "model_id": model_id,
            "messages_count": api_messages.len(),
            "user_chars": user_len,
        }),
    );
    // #endregion

    let res = client.post(&url).json(&body).send().await.map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        let status = res.status();
        let status_u16 = status.as_u16();
        let text = res.text().await.unwrap_or_default();
        // #region agent log
        let text_excerpt: String = text.chars().take(800).collect();
        crate::debug_log::dlog(
            "client.rs:stream_chat",
            "H4",
            "chat completions FAILED",
            serde_json::json!({
                "status": status_u16,
                "body_excerpt": text_excerpt,
                "model_id": model_id,
            }),
        );
        // #endregion
        let err = format!("AKASHA HTTP {status}: {text}");
        let _ = app.emit("akasha-error", &err);
        return Err(err);
    }

    // #region agent log
    crate::debug_log::dlog(
        "client.rs:stream_chat",
        "H4",
        "chat completions OK, starting stream",
        serde_json::json!({ "status": res.status().as_u16() }),
    );
    // #endregion

    let mut stream = res.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        // Felhasználói Stop: ha a frontend kérte a leállást, kilépünk
        // és visszaadjuk az addig összegyűlt szöveget. A `done` event-et is
        // emittáljuk, hogy a UI állapotgépe rendben legyen.
        if cancel.load(Ordering::Acquire) {
            eta.record_completion(ctx.slot, started.elapsed().as_millis() as u64, full.len());
            // v0.2.6 - suppress mode: a translation flow majd a végén küldi
            // az akasha-done-t. Ha most küldenénk, a frontend `streaming`-et
            // false-ra állítaná, az indicator eltűnne, és a user 2-3 mp-ig
            // "üres képet" látna mielőtt a Gemma magyar válasza megkezdődne.
            if !suppress_frontend_tokens {
                let _ = app.emit("akasha-done", ());
            }
            return Ok(full);
        }

        // === Adaptív Védelmi Protokoll (Fázis 2) - kíméletes mód ===
        // Ha a throttle CRITICAL állapotban van (RAM <1 GB vagy CPU >90%),
        // szándékosan lassítjuk a stream-feldolgozást, hogy a többi program
        // is tudjon dolgozni. Az async sleep nem blokkolja a Tokio thread-et.
        match throttle.current_level() {
            super::throttle::ThrottleLevel::Critical => {
                tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            }
            super::throttle::ThrottleLevel::Warning => {
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            }
            super::throttle::ThrottleLevel::Normal => {}
        }

        let bytes = chunk.map_err(|e| e.to_string())?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        if last_tick.elapsed().as_millis() >= poll_interval_ms as u128 {
            emit_progress(
                &app,
                &eta,
                ctx.slot,
                started,
                full.len(),
                gen_start.estimated_total_ms,
                &throttle,
                &throttle_cfg,
            );
            last_tick = Instant::now();
        }

        while let Some(pos) = buffer.find("\n\n") {
            let line_block = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();

            for line in line_block.lines() {
                let line = line.trim();
                if !line.starts_with("data: ") {
                    continue;
                }
                let data = line.trim_start_matches("data: ").trim();
                if data == "[DONE]" {
                    eta.record_completion(ctx.slot, started.elapsed().as_millis() as u64, full.len());
                    // v0.2.6 - lásd a fenti suppress mode komment
                    if !suppress_frontend_tokens {
                        let _ = app.emit("akasha-done", ());
                    }
                    return Ok(full);
                }
                if let Ok(parsed) = serde_json::from_str::<StreamChunk>(data) {
                    if let Some(choice) = parsed.choices.first() {
                        // Reasoning-modellek két külön mezőt streamelnek:
                        //   - reasoning_content → a belső "gondolkodás" (chain-of-thought)
                        //   - content          → a valódi, végleges válasz
                        // Az UI külön panelben jeleníti meg a kettőt: a gondolkodás
                        // egy halvány, becsukható dobozban, a valódi válasz a fő
                        // beszélgetés-buborékban. A "full" stringbe csak a valódi
                        // content kerül, mert ez kerül elmentésre a chat history-ba.
                        if let Some(thinking) = &choice.delta.reasoning_content {
                            if !thinking.is_empty() && !suppress_frontend_tokens {
                                let _ = app.emit("akasha-thinking-token", thinking.clone());
                            }
                        }
                        if let Some(content) = &choice.delta.content {
                            if !content.is_empty() {
                                full.push_str(content);
                                if !suppress_frontend_tokens {
                                    let _ = app.emit("akasha-token", content.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    eta.record_completion(ctx.slot, started.elapsed().as_millis() as u64, full.len());
    // v0.2.6 - lásd a fenti suppress mode komment
    if !suppress_frontend_tokens {
        let _ = app.emit("akasha-done", ());
    }
    Ok(full)
}

fn emit_progress(
    app: &AppHandle,
    eta: &EtaEstimator,
    slot: AkashaSlot,
    started: Instant,
    chars: usize,
    total_est_ms: u64,
    throttle: &DynamicThrottle,
    throttle_cfg: &AkashaThrottleConfig,
) {
    let elapsed = started.elapsed().as_millis() as u64;
    let tick: GenTickEvent = eta.tick(slot, elapsed, chars, total_est_ms);
    let _ = app.emit("akasha-gen-tick", &tick);

    let mut hw = crate::akasha::hardware::HardwareMonitor::new();
    let snap = hw.snapshot();
    let status: ThrottleStatus = throttle.evaluate(&snap, throttle_cfg, 0);
    let _ = app.emit("akasha-throttle", &status);
}
