use std::io::{self, BufRead, Write};
use std::path::Path;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

const VOCABULARY: &[&str] = &[
    "portrait", "landscape", "street photography", "architecture", "nature",
    "wildlife", "macro photography", "aerial photography", "night photography",
    "golden hour", "sunset", "sunrise", "black and white photography",
    "candid photography", "wedding photography", "food photography",
    "product photography", "sports photography", "travel photography",
    "urban photography", "rural scene", "beach", "mountains", "forest",
    "city skyline", "abstract photography", "minimalist photography",
    "group of people", "animals", "flowers", "waterfall", "cloudy sky",
    "interior", "outdoor", "close-up detail", "wide angle scene",
    "long exposure", "bokeh background", "silhouette", "reflection in water",
    "texture", "repeating pattern", "vibrant colors", "monochrome",
    "vintage style", "modern architecture", "artistic composition",
    "documentary photography", "children", "elderly person",
];

#[derive(Deserialize, Default)]
struct Config {
    api_key: Option<String>,
    #[serde(default = "default_endpoint")]
    api_endpoint: String,
    #[serde(default = "default_detail")]
    detail: String,
    #[serde(default)]
    vocabulary_file: String,
}

fn default_detail() -> String {
    "low".to_string()
}

fn default_endpoint() -> String {
    DEFAULT_ENDPOINT.to_string()
}

#[derive(Deserialize)]
struct Request {
    id: u64,
    method: String,
    params: Value,
}

#[derive(Serialize)]
struct Response {
    id: u64,
    result: Value,
}

#[derive(Serialize)]
struct ErrorResponse {
    id: u64,
    error: String,
}

fn emit_log(out: &mut impl Write, level: &str, msg: &str) {
    let _ = writeln!(
        out,
        "{}",
        serde_json::json!({ "type": "log", "level": level, "message": msg })
    );
    let _ = out.flush();
}

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let config = load_config(&mut out);

    let vocab = load_vocabulary(&config, &mut out);

    if config.api_key.as_deref().unwrap_or("").is_empty() {
        emit_log(&mut out, "error", "api_key not configured — open Settings to add it");
        return;
    }

    let _ = writeln!(
        out,
        "{}",
        serde_json::json!({
            "type": "hello",
            "protocol_version": 1,
            "addon_api_version": 1,
            "capabilities": ["classify"],
        })
    );
    let _ = out.flush();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<Request>(line) {
            Ok(req) => {
                let resp = match req.method.as_str() {
                    "classify" => match classify_one(&config, &vocab, &req.params) {
                        Ok(r) => serde_json::to_string(&Response { id: req.id, result: r }).unwrap(),
                        Err(e) => serde_json::to_string(&ErrorResponse { id: req.id, error: e }).unwrap(),
                    },
                    m => serde_json::to_string(&ErrorResponse {
                        id: req.id,
                        error: format!("unknown method: {m}"),
                    })
                    .unwrap(),
                };
                let _ = writeln!(out, "{resp}");
                let _ = out.flush();
            }
            Err(e) => eprintln!("[autotag-openai] parse error: {e}"),
        }
    }
}

fn load_config(out: &mut impl Write) -> Config {
    let path = std::env::var("ISOMFOLIO_ADDON_CONFIG").unwrap_or_default();
    if path.is_empty() {
        return Config::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            emit_log(out, "warn", &format!("config parse error: {e}, using defaults"));
            Config::default()
        }),
        Err(_) => Config::default(),
    }
}

fn load_vocabulary(config: &Config, out: &mut impl Write) -> Vec<String> {
    if !config.vocabulary_file.is_empty() {
        match std::fs::read_to_string(&config.vocabulary_file) {
            Ok(content) => {
                let tags: Vec<String> = content
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .collect();
                if !tags.is_empty() {
                    emit_log(out, "info", &format!("loaded {} tags from {}", tags.len(), config.vocabulary_file));
                    return tags;
                }
                emit_log(out, "warn", "vocabulary file empty, using defaults");
            }
            Err(e) => {
                emit_log(out, "warn", &format!("failed to read vocabulary file: {e}, using defaults"));
            }
        }
    }
    VOCABULARY.iter().map(|s| s.to_string()).collect()
}

fn classify_one(config: &Config, vocab: &[String], params: &Value) -> Result<Value, String> {
    let thumb_path = params
        .get("thumbnail_path")
        .and_then(|v| v.as_str())
        .ok_or("missing thumbnail_path")?;

    let api_key = config.api_key.as_deref().unwrap_or("");
    let bytes = std::fs::read(thumb_path).map_err(|e| format!("read image: {e}"))?;

    let ext = Path::new(thumb_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpeg")
        .to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };

    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let data_url = format!("data:{mime};base64,{b64}");

    let vocab_json = serde_json::to_string(vocab).unwrap();
    let prompt = format!(
        "Return a JSON array of the top 5 most relevant tags for this image, \
        chosen only from this list: {vocab_json}. \
        Respond with only the JSON array, no explanation. Example: [\"portrait\", \"golden hour\"]"
    );

    let detail = if ["low", "high", "auto"].contains(&config.detail.as_str()) {
        &config.detail
    } else {
        "low"
    };

    let body = serde_json::json!({
        "model": "gpt-4o-mini",
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "image_url",
                    "image_url": { "url": data_url, "detail": detail }
                },
                { "type": "text", "text": prompt }
            ]
        }],
        "max_tokens": 150
    });

    let response = ureq::post(&config.api_endpoint)
        .header("Authorization", &format!("Bearer {api_key}"))
        .send_json(&body)
        .map_err(|e| format!("API call failed: {e}"))?;

    let text = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("read response: {e}"))?;

    let json: Value =
        serde_json::from_str(&text).map_err(|e| format!("parse response JSON: {e}"))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("[]");

    let content = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let tag_list: Vec<String> = serde_json::from_str(content).unwrap_or_default();
    let tags: Vec<Value> = tag_list
        .iter()
        .take(5)
        .map(|t| serde_json::json!({ "tag": t, "confidence": 1.0 }))
        .collect();

    let file_id = params.get("file_id").and_then(|v| v.as_str()).unwrap_or("");
    Ok(serde_json::json!({ "file_id": file_id, "tags": tags }))
}
