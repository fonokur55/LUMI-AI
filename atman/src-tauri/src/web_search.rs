//! Internet keresés modul - DuckDuckGo HTML végpontot scrape-eli.
//!
//! Miért DuckDuckGo:
//! - Nincs API kulcs (nem kell userhez kötni semmilyen fiókot)
//! - Nem trackel (illik a NOMAD privacy-first filozófiához)
//! - Stabil HTML output (a /html/ végpont kifejezetten programatic-friendly)
//!
//! Ez a modul OPT-IN: csak akkor hívódik, ha a felhasználó a UI-ban
//! aktiválta a "globe" toggle-t. Alapból az app 100% offline marad.

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub title: String,
    pub snippet: String,
    pub url: String,
}

/// Lekérdezi a DuckDuckGo találati listát és visszaadja a top N eredményt.
/// Hibalcázás esetén Err-t ad (a hívó eldönti, hogy folytatja-e a chat-et
/// keresés nélkül vagy elszáll).
pub async fn search(query: &str, max_results: usize) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let client = reqwest::Client::builder()
        // Tipikus böngésző User-Agent - különben a DDG néha CAPTCHA-t kér.
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP kliens létrehozása sikertelen: {e}"))?;

    // DDG html végpont; a `q` paramétert reqwest URL-encode-olja
    // (magyar ékezetek, szóközök rendben átmennek).
    let resp = client
        .get("https://html.duckduckgo.com/html/")
        .query(&[("q", query), ("kl", "hu-hu")])
        .send()
        .await
        .map_err(|e| format!("DDG kérés sikertelen: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("DDG hibakód: HTTP {}", resp.status()));
    }

    let html = resp
        .text()
        .await
        .map_err(|e| format!("DDG válasz beolvasás sikertelen: {e}"))?;

    Ok(parse_results(&html, max_results))
}

/// HTML parser - a DDG találatok jól előrelátható struktúrában vannak:
///   <a class="result__a" href="...">CÍM</a>
///   <a class="result__snippet" ...>SNIPPET</a>
///
/// A href-en belüli URL gyakran DDG redirect (`//duckduckgo.com/l/?uddg=...`)
/// - az `uddg` paraméterből kibontjuk a valódi URL-t.
fn parse_results(html: &str, max_results: usize) -> Vec<SearchResult> {
    // Egyszerre olvassuk be a cím-link tag-et (href + szöveg) ÉS az utána
    // jövő snippet-et, ami a következő .result__snippet tag-ben van.
    let title_re = Regex::new(
        r#"(?s)<a[^>]+class="result__a"[^>]+href="([^"]+)"[^>]*>(.*?)</a>"#,
    )
    .expect("valid regex");
    let snippet_re = Regex::new(
        r#"(?s)<a[^>]+class="result__snippet"[^>]*>(.*?)</a>"#,
    )
    .expect("valid regex");

    let titles: Vec<_> = title_re.captures_iter(html).collect();
    let snippets: Vec<_> = snippet_re.captures_iter(html).collect();

    let mut out = Vec::new();
    for (i, t) in titles.iter().enumerate() {
        if out.len() >= max_results {
            break;
        }
        let url_raw = t.get(1).map(|m| m.as_str()).unwrap_or("");
        let title_raw = t.get(2).map(|m| m.as_str()).unwrap_or("");
        let snippet_raw = snippets
            .get(i)
            .and_then(|s| s.get(1))
            .map(|m| m.as_str())
            .unwrap_or("");

        let url = normalize_ddg_url(url_raw);
        let title = strip_html(title_raw);
        let snippet = strip_html(snippet_raw);

        if !title.is_empty() && !url.is_empty() {
            out.push(SearchResult { title, snippet, url });
        }
    }
    out
}

/// A DDG sokszor saját redirect-en keresztül adja vissza a linket:
/// `//duckduckgo.com/l/?uddg=https%3A%2F%2Fpelda.hu%2Fcikk&rut=...`
/// Innen kell kibontani a valódi URL-t (uddg paraméter, URL-encoded).
fn normalize_ddg_url(raw: &str) -> String {
    // Ha protokol-nélküli ("//..."), tegyünk eléje https-t hogy
    // a parser elfogadja.
    let raw = if raw.starts_with("//") {
        format!("https:{raw}")
    } else {
        raw.to_string()
    };

    // Próbáljuk kihúzni az `uddg` query paramétert.
    if let Some(uddg_start) = raw.find("uddg=") {
        let after = &raw[uddg_start + 5..];
        let end = after.find('&').unwrap_or(after.len());
        let encoded = &after[..end];
        if let Ok(decoded) = url_decode(encoded) {
            return decoded;
        }
    }
    raw
}

/// Minimal URL percent-decode. Nincs `urlencoding` dep, ezt kézzel
/// elintézzük - csak `%XX` és `+` → szóköz kell.
fn url_decode(s: &str) -> Result<String, ()> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let h = std::str::from_utf8(&bytes[i + 1..i + 3]).map_err(|_| ())?;
                let v = u8::from_str_radix(h, 16).map_err(|_| ())?;
                out.push(v);
                i += 3;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| ())
}

/// Eltávolítja a HTML tag-eket és a leggyakoribb entity-ket - a snippet-ek
/// néha `<b>kiemeléseket` tartalmaznak, amik nyersen csúnyán nézhetnek ki
/// a system promptban.
fn strip_html(s: &str) -> String {
    let tag_re = Regex::new(r"<[^>]+>").expect("valid regex");
    let stripped = tag_re.replace_all(s, "");
    stripped
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

/// LLM-barát formátum: a system promptba beszúrható block.
pub fn format_for_prompt(query: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return format!(
            "\n\n[INTERNET KERESÉS: \"{query}\" - nem érkezett találat. \
             Mondd meg a felhasználónak hogy a keresés üres lett, és próbáljon \
             más kulcsszavakkal.]"
        );
    }
    let mut out = format!(
        "\n\n[INTERNET KERESÉSI EREDMÉNYEK a \"{query}\" lekérdezésre. \
         Ezek a kapott pillanatban érvényes valós találatok - ezekre \
         hivatkozz, ne a tréning-adatra. A források URL-jeit IDÉZD a \
         válasz végén forrás-listaként.]"
    );
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!(
            "\n\n[{}] {}\nForrás: {}\nKivonat: {}",
            i + 1,
            r.title,
            r.url,
            r.snippet
        ));
    }
    out
}
