// =========================================================================
//  v0.2.4 - Markdown-aware fordítás a Kód mód output-jához
// =========================================================================
//
//  HÁTTÉR (miért van ez a fájl)
//  ----------------------------
//  A Qwen 2.5 Coder 7B (a Kód expert) kifejezetten erős kódolásban, DE
//  a magyar nyelvi tudása alapból gyenge — magyar magyarázatában
//  szóleletek, ragozás-hibák, sőt félrefogalmazott tényállítások
//  fordulnak elő (lásd Áron tesztje a Steve Jobs HTML-lel).
//
//  MEGOLDÁS
//  --------
//  A Kód mód flow-ja kétlépéses:
//
//    1. A Coder a system promptban kapja, hogy CSAK ANGOLUL válaszoljon.
//       Így a kódolási tudását teljesen ki tudja használni, nem küzd
//       a magyarral.
//
//    2. A válasz kész → a Gemma 2 2B (Szöveg expert, **natív magyar
//       erősség**) átveszi, és magyarra fordítja a NEM-kód részeket.
//       A code-blockok érintetlenül átkerülnek a placeholder-trükkel
//       (kódot nem szabad fordítani: `showMore()` → `többetMutat()`
//       a JavaScript-et halálra ítélné).
//
//  HASZON: a kód angolul marad (az is helyes nemzetközi norma),
//          a magyarázat tiszta, természetes magyar lesz.
//
//  ÁRA: a Coder után kell egy Gemma-betöltés is, ami a v0.2.3 router-
//       flow-jában ~5-10 mp extra latency. A felhasználó a `gondolkodom...`
//       random feliratok mellett várja a fordítást.
// =========================================================================

use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

/// A `Result<TextWithoutCode, Vec<CodeBlock>>` mintázathoz tartozó
/// kivonat-eredmény.
pub struct CodeExtraction {
    /// A szöveg, amiben a kód-részek helyét placeholder-ek jelölik
    /// (pl. `«CODE_0»`, `«INLINE_3»`). A fordítónak ez megy be —
    /// a Gemma 2B utasításban kapja, hogy a placeholder-eket
    /// változatlanul hagyja.
    pub text_with_placeholders: String,
    /// A kivett code-blokkok (ÍGY tartjuk, a markdown-markerek mind
    /// benne maradnak: ```\nlang\n...\n```).
    pub code_blocks: Vec<String>,
    /// Az inline `code` szegmensek (a backtick-ekkel együtt).
    pub inline_codes: Vec<String>,
}

// Lazy-init regex-ek (egyszer fordítjuk, sokszor használjuk)
fn fenced_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // ```optional_lang\n ... ``` (multiline, lazy, lang-tag elfogadása)
        Regex::new(r"(?ms)```[^\n]*\n.*?\n```").expect("fenced regex compile")
    })
}

fn inline_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // `xxx` egy soron belül, legalább egy karakter
        Regex::new(r"`[^`\n]+`").expect("inline regex compile")
    })
}

/// Kivonja a code-blokkokat és inline-code-okat placeholder-ekre cserélve.
///
/// A placeholder formátuma `«CODE_N»` és `«INLINE_N»`. A `«» ` francia
/// idézőjeleket azért használjuk, mert a normál angol szövegben ÉS a kódban
/// szinte sose fordulnak elő — így a fordító ezeket biztosan átengedi,
/// és mi pontosan azonosíthatjuk őket vissza-helyettesítéskor.
pub fn extract_code_blocks(input: &str) -> CodeExtraction {
    let mut code_blocks: Vec<String> = Vec::new();
    let mut inline_codes: Vec<String> = Vec::new();

    // 1. ELŐSZÖR a fenced ``` blokkokat — mert ezek belsejében szintén
    //    lehet `inline code`, amit NEM szabad inline-ként kezelni.
    let mut after_fenced = String::new();
    let mut last = 0;
    let s = input;
    for m in fenced_re().find_iter(s) {
        after_fenced.push_str(&s[last..m.start()]);
        let idx = code_blocks.len();
        code_blocks.push(m.as_str().to_string());
        after_fenced.push_str(&format!("«CODE_{idx}»"));
        last = m.end();
    }
    after_fenced.push_str(&s[last..]);

    // 2. AZTÁN az inline `code`-okat — már csak a fenced blokkokon
    //    KÍVÜLI szövegben keresünk.
    let mut result = String::new();
    let mut last = 0;
    let s2 = after_fenced.as_str();
    for m in inline_re().find_iter(s2) {
        result.push_str(&s2[last..m.start()]);
        let idx = inline_codes.len();
        inline_codes.push(m.as_str().to_string());
        result.push_str(&format!("«INLINE_{idx}»"));
        last = m.end();
    }
    result.push_str(&s2[last..]);

    CodeExtraction {
        text_with_placeholders: result,
        code_blocks,
        inline_codes,
    }
}

/// A fordító visszaadta a magyar szöveget a placeholder-ekkel.
/// Visszahelyettesítjük a kódokat eredeti formájukban.
pub fn restore_code_blocks(translated: &str, extraction: &CodeExtraction) -> String {
    let mut out = translated.to_string();
    for (i, block) in extraction.code_blocks.iter().enumerate() {
        out = out.replace(&format!("«CODE_{i}»"), block);
    }
    for (i, inline) in extraction.inline_codes.iter().enumerate() {
        out = out.replace(&format!("«INLINE_{i}»"), inline);
    }
    out
}

// =========================================================================
//  Gemma 2 2B-vel végzett fordítás
// =========================================================================

const TRANSLATOR_MODEL_PRESET: &str = "szoveg";

/// A Gemma 2 2B-nek küldött system prompt fordítási feladathoz.
///
/// FONTOS részletek:
/// - Megőrzi a placeholder-eket változatlanul
/// - Megőrzi a markdown formázást (heading, bold, lista)
/// - Csak a magyar fordítást adja vissza, semmi mást (nincs "Here is...")
const TRANSLATOR_SYSTEM_PROMPT: &str = "You are an English-to-Hungarian translator specialized in technical text. \
Translate the following English text to natural, fluent Hungarian. \
\n\nCRITICAL RULES:\n\
1. Preserve ALL placeholders like «CODE_0», «CODE_1», «INLINE_0», «INLINE_2» etc. — keep them EXACTLY as they are, do NOT translate them.\n\
2. Preserve the markdown formatting: headings (# ## ###), bold (**...**), italic (*...*), lists (- or *), tables.\n\
3. Output ONLY the Hungarian translation. NO preamble like \"Here is the translation:\". NO explanations. NO afterword.\n\
\n\
4. **NATURAL HUNGARIAN — THIS IS THE MOST IMPORTANT RULE:**\n\
   - NEVER invent Hungarian words. If you don't know how to translate a term, use \
     a well-known synonym or keep the English original (e.g., 'API', 'framework').\n\
   - NEVER do word-for-word translation. English sentence structure does NOT work in Hungarian.\n\
   - Use natural Hungarian sentence flow (SOV is common, but variable based on emphasis).\n\
   - Examples of BAD vs GOOD:\n\
     * BAD: 'programnyelő képzettség' (made-up word!) → GOOD: 'programozási nyelvtudás' or 'programozói ismeretek'\n\
     * BAD: 'weboldal létrehozni' (English infinitive structure) → GOOD: 'weboldal készítése'\n\
     * BAD: 'I am being able to' translated as 'képes vagyok lenni' → GOOD: 'képes vagyok'\n\
     * BAD: 'körkörösítés' (made-up technical term) → GOOD: 'lekerekítés'\n\
   - If a literal translation sounds awkward, REPHRASE the whole sentence naturally.\n\
\n\
5. Technical terms: keep widely-recognized English ones (HTML, JavaScript, CSS, div, \
   function, framework, API); translate descriptive prose around them naturally.\n\
\n\
6. If the input is already in Hungarian or contains no translatable prose (only \
   placeholders), return it unchanged.\n\
\n\
7. Tone: friendly, helpful, professional — like a knowledgeable Hungarian developer \
   explaining to a colleague.";

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageOut,
}

#[derive(Debug, Deserialize)]
struct ChatMessageOut {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

/// Magyarra fordítja az angol szöveget a Gemma 2 2B-vel.
///
/// FONTOS: a hívónak gondoskodnia kell hogy a Gemma 2B betöltött
/// state-ben legyen a router-ben (`ensure_model_loaded("szoveg")`).
/// Ez a függvény csak a `/v1/chat/completions` POST-ot küldi el.
pub async fn translate_to_hungarian(
    base_url: &str,
    english_text: &str,
) -> Result<String, String> {
    if english_text.trim().is_empty() {
        return Ok(String::new());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Translator HTTP klient: {e}"))?;

    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": TRANSLATOR_MODEL_PRESET,
        "messages": [
            { "role": "system", "content": TRANSLATOR_SYSTEM_PROMPT },
            { "role": "user", "content": english_text }
        ],
        "stream": false,
        // Alacsony temperature: a fordításnak DETERMINISZTIKUSnak kell lennie,
        // ne hozzon kreatív "saját szavakkal" eltéréseket.
        "temperature": 0.2,
        "max_tokens": 4096,
        "repeat_penalty": 1.10,
        "stop": [
            "<|im_end|>",
            "<|end|>",
            "<|endoftext|>",
            "<end_of_turn>",
            "<|eot_id|>",
        ],
    });

    let res = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Translator POST: {e}"))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Translator HTTP {status}: {text}"));
    }

    let parsed: ChatResponse = res
        .json()
        .await
        .map_err(|e| format!("Translator JSON parse: {e}"))?;

    let translated = parsed
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| "Translator: nincs content a választban".to_string())?;

    Ok(translated)
}

// =========================================================================
//  Magas-szintű API: a teljes flow egy függvényben
// =========================================================================

/// A Coder angol output-ját átírja magyarra a Gemma 2B-vel,
/// megőrizve a code-blokkokat változatlanul.
///
/// Ha a `english_response` üres vagy csak whitespace, az eredeti
/// stringet adja vissza (no-op).
pub async fn coder_output_to_hungarian(
    base_url: &str,
    english_response: &str,
) -> Result<String, String> {
    if english_response.trim().is_empty() {
        return Ok(english_response.to_string());
    }
    let extraction = extract_code_blocks(english_response);

    // Ha a code-blockok kivétele után csak whitespace + placeholder marad,
    // semmi szöveges fordítandó. Visszaállítjuk és kész.
    let stripped = extraction
        .text_with_placeholders
        .replace(|c: char| c.is_whitespace(), "");
    let only_placeholders = stripped.chars().all(|c| {
        c == '«' || c == '»' || c.is_ascii_alphanumeric() || c == '_'
    });
    if only_placeholders {
        return Ok(english_response.to_string());
    }

    let translated_with_placeholders =
        translate_to_hungarian(base_url, &extraction.text_with_placeholders).await?;
    Ok(restore_code_blocks(&translated_with_placeholders, &extraction))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_fenced_block() {
        let input = "Itt egy függvény:\n```javascript\nfunction hello() { return 42; }\n```\nEz volt.";
        let ex = extract_code_blocks(input);
        assert_eq!(ex.code_blocks.len(), 1);
        assert!(ex.text_with_placeholders.contains("«CODE_0»"));
        assert!(ex.code_blocks[0].contains("function hello()"));
    }

    #[test]
    fn extract_inline_code() {
        let input = "A `showMore()` függvényt használd.";
        let ex = extract_code_blocks(input);
        assert_eq!(ex.inline_codes.len(), 1);
        assert!(ex.text_with_placeholders.contains("«INLINE_0»"));
        assert_eq!(ex.inline_codes[0], "`showMore()`");
    }

    #[test]
    fn restore_keeps_code_intact() {
        let input = "Use the `foo()` function:\n```js\nfoo();\n```\nIt works.";
        let ex = extract_code_blocks(input);
        // Tegyük úgy mintha a fordító magyarra fordítaná a prózát
        let fake_translated = ex
            .text_with_placeholders
            .replace("Use the", "Használd a")
            .replace("function:", "függvényt:")
            .replace("It works.", "Működik.");
        let restored = restore_code_blocks(&fake_translated, &ex);
        assert!(restored.contains("`foo()`"));
        assert!(restored.contains("```js\nfoo();\n```"));
        assert!(restored.contains("Használd a"));
        assert!(restored.contains("Működik."));
    }
}
