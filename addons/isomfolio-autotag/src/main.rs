use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokenizers::Tokenizer;
use tract_onnx::prelude::{
    tvec, DatumExt, Framework, Graph, InferenceModelExt, RunnableModel, Tensor, TractError,
    TractResult, TypedFact, TypedOp, tract_ndarray,
};

// CLIP ViT-B/32 from Xenova
const B32_SUBDIR: &str = "clip-vit-b32";
const B32_VISION_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model.onnx";
const B32_TEXT_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/text_model.onnx";
const B32_TOKENIZER_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/tokenizer.json";

// CLIP ViT-L/14 from Xenova
const L14_SUBDIR: &str = "clip-vit-l14";
const L14_VISION_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-large-patch14/resolve/main/onnx/vision_model.onnx";
const L14_TEXT_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-large-patch14/resolve/main/onnx/text_model.onnx";
const L14_TOKENIZER_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-large-patch14/resolve/main/tokenizer.json";

// CLIP normalisation constants (same for both variants)
const CLIP_MEAN: [f32; 3] = [0.48145466, 0.4578275, 0.40821073];
const CLIP_STD: [f32; 3] = [0.26862954, 0.26130258, 0.27577711];
const IMAGE_SIZE: usize = 224;
const TOKEN_LEN: usize = 77;
const SOT_TOKEN: i64 = 49406;
const EOT_TOKEN: i64 = 49407;

const VOCABULARY: &[&str] = &[
    "portrait",
    "landscape",
    "street photography",
    "architecture",
    "nature",
    "wildlife",
    "macro photography",
    "aerial photography",
    "night photography",
    "golden hour",
    "sunset",
    "sunrise",
    "black and white photography",
    "candid photography",
    "wedding photography",
    "food photography",
    "product photography",
    "sports photography",
    "travel photography",
    "urban photography",
    "rural scene",
    "beach",
    "mountains",
    "forest",
    "city skyline",
    "abstract photography",
    "minimalist photography",
    "group of people",
    "animals",
    "flowers",
    "waterfall",
    "cloudy sky",
    "interior",
    "outdoor",
    "close-up detail",
    "wide angle scene",
    "long exposure",
    "bokeh background",
    "silhouette",
    "reflection in water",
    "texture",
    "repeating pattern",
    "vibrant colors",
    "monochrome",
    "vintage style",
    "modern architecture",
    "artistic composition",
    "documentary photography",
    "children",
    "elderly person",
];

type TractModel = RunnableModel<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

#[derive(Deserialize, Default)]
struct Config {
    #[serde(default = "default_backend")]
    backend: String,
    api_key: Option<String>,
    #[serde(default = "default_endpoint")]
    api_endpoint: String,
}

fn default_backend() -> String {
    "clip-vit-b32".to_string()
}

fn default_endpoint() -> String {
    "https://api.openai.com/v1/chat/completions".to_string()
}

enum BackendState {
    ClipLocal {
        vision_model: TractModel,
        vocab_embeds: Vec<Vec<f32>>,
    },
    OpenAiVision {
        api_key: String,
        api_endpoint: String,
    },
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
    let models_base = std::env::var("ISOMFOLIO_MODELS_DIR").unwrap_or_else(|_| ".".to_string());

    let backend = match init_backend(&config, &models_base, &mut out) {
        Ok(b) => b,
        Err(e) => {
            emit_log(&mut out, "error", &format!("backend init failed: {e}"));
            return;
        }
    };

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
                    "classify" => match handle_classify(&backend, &req.params) {
                        Ok(r) => serde_json::to_string(&Response { id: req.id, result: r }).unwrap(),
                        Err(e) => serde_json::to_string(&ErrorResponse {
                            id: req.id,
                            error: e.to_string(),
                        })
                        .unwrap(),
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
            Err(e) => eprintln!("[autotag] parse error: {e}"),
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

fn init_backend(config: &Config, models_base: &str, out: &mut impl Write) -> Result<BackendState, String> {
    match config.backend.as_str() {
        "openai-vision" => {
            let api_key = config
                .api_key
                .clone()
                .filter(|k| !k.is_empty())
                .ok_or_else(|| "openai-vision backend requires api_key in config".to_string())?;
            emit_log(out, "info", "using OpenAI Vision backend");
            Ok(BackendState::OpenAiVision {
                api_key,
                api_endpoint: config.api_endpoint.clone(),
            })
        }
        backend => {
            let (subdir, vision_url, text_url, tokenizer_url) = match backend {
                "clip-vit-l14" => (L14_SUBDIR, L14_VISION_URL, L14_TEXT_URL, L14_TOKENIZER_URL),
                _ => (B32_SUBDIR, B32_VISION_URL, B32_TEXT_URL, B32_TOKENIZER_URL),
            };
            let model_dir = PathBuf::from(models_base).join(subdir);

            ensure_models(&model_dir, vision_url, text_url, tokenizer_url, out)
                .map_err(|e| format!("model download failed: {e}"))?;

            let vision_model =
                load_vision_model(model_dir.join("vision_model.onnx")).map_err(|e| e.to_string())?;
            let text_model =
                load_text_model(model_dir.join("text_model.onnx"), VOCABULARY.len()).map_err(|e| e.to_string())?;
            let tokenizer = Tokenizer::from_file(model_dir.join("tokenizer.json"))
                .map_err(|e| e.to_string())?;

            emit_log(out, "info", "embedding vocabulary…");
            let vocab_embeds =
                embed_vocabulary(&text_model, &tokenizer).map_err(|e| e.to_string())?;
            emit_log(out, "info", &format!("ready ({backend})"));

            Ok(BackendState::ClipLocal { vision_model, vocab_embeds })
        }
    }
}

fn ensure_models(
    dir: &Path,
    vision_url: &str,
    text_url: &str,
    tokenizer_url: &str,
    out: &mut impl Write,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    download_if_missing(dir.join("vision_model.onnx"), vision_url, out)?;
    download_if_missing(dir.join("text_model.onnx"), text_url, out)?;
    download_if_missing(dir.join("tokenizer.json"), tokenizer_url, out)?;
    Ok(())
}

fn download_if_missing(path: PathBuf, url: &str, out: &mut impl Write) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    emit_log(out, "info", &format!("downloading {name}…"));
    let response = ureq::get(url).call().map_err(|e| format!("{url}: {e}"))?;
    let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    std::io::copy(&mut response.into_body().as_reader(), &mut file).map_err(|e| e.to_string())?;
    emit_log(out, "info", &format!("{name} ready"));
    Ok(())
}

fn load_vision_model(path: PathBuf) -> TractResult<TractModel> {
    tract_onnx::onnx()
        .model_for_path(path)?
        .with_input_fact(0, f32::fact([1usize, 3, IMAGE_SIZE, IMAGE_SIZE]).into())?
        .into_optimized()?
        .into_runnable()
}

fn load_text_model(path: PathBuf, batch: usize) -> TractResult<TractModel> {
    tract_onnx::onnx()
        .model_for_path(path)?
        .with_input_fact(0, i64::fact([batch, TOKEN_LEN]).into())?
        .with_input_fact(1, i64::fact([batch, TOKEN_LEN]).into())?
        .into_optimized()?
        .into_runnable()
}

fn tokenize(tokenizer: &Tokenizer, text: &str) -> (Vec<i64>, Vec<i64>) {
    let enc = tokenizer
        .encode(text, false)
        .unwrap_or_else(|_| tokenizer.encode("", false).unwrap());
    let mut ids: Vec<i64> = std::iter::once(SOT_TOKEN)
        .chain(enc.get_ids().iter().map(|&id| id as i64))
        .chain(std::iter::once(EOT_TOKEN))
        .collect();
    ids.truncate(TOKEN_LEN);
    let mask: Vec<i64> = ids.iter().map(|_| 1).collect();
    ids.resize(TOKEN_LEN, 0);
    let mut mask = mask;
    mask.resize(TOKEN_LEN, 0);
    (ids, mask)
}

fn embed_vocabulary(model: &TractModel, tokenizer: &Tokenizer) -> TractResult<Vec<Vec<f32>>> {
    let n = VOCABULARY.len();
    let mut flat_ids = Vec::with_capacity(n * TOKEN_LEN);
    let mut flat_mask = Vec::with_capacity(n * TOKEN_LEN);
    for label in VOCABULARY {
        let (ids, mask) = tokenize(tokenizer, label);
        flat_ids.extend(ids);
        flat_mask.extend(mask);
    }
    let ids_t: Tensor = tract_ndarray::Array2::from_shape_vec((n, TOKEN_LEN), flat_ids)?.into();
    let mask_t: Tensor = tract_ndarray::Array2::from_shape_vec((n, TOKEN_LEN), flat_mask)?.into();
    let outputs = model.run(tvec![ids_t.into(), mask_t.into()])?;
    extract_embeddings(&outputs[0], n)
}

fn handle_classify(backend: &BackendState, params: &Value) -> TractResult<Value> {
    let thumb_path = params
        .get("thumbnail_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TractError::msg("missing thumbnail_path"))?;

    match backend {
        BackendState::ClipLocal { vision_model, vocab_embeds } => {
            classify_clip(vision_model, vocab_embeds, thumb_path)
        }
        BackendState::OpenAiVision { api_key, api_endpoint } => {
            classify_openai(api_key, api_endpoint, thumb_path)
        }
    }
}

fn classify_clip(
    model: &TractModel,
    vocab_embeds: &[Vec<f32>],
    thumb_path: &str,
) -> TractResult<Value> {
    let img_embed = embed_image(model, thumb_path)?;

    let mut scores: Vec<(f32, &str)> = vocab_embeds
        .iter()
        .zip(VOCABULARY.iter())
        .map(|(emb, label)| (cosine_sim(&img_embed, emb), *label))
        .collect();
    scores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let tags: Vec<Value> = scores
        .iter()
        .take(5)
        .filter(|(s, _)| *s > 0.0)
        .map(|(s, label)| serde_json::json!({ "tag": label, "confidence": s }))
        .collect();

    Ok(serde_json::json!({ "tags": tags }))
}

fn classify_openai(api_key: &str, api_endpoint: &str, thumb_path: &str) -> TractResult<Value> {
    let bytes = std::fs::read(thumb_path)
        .map_err(|e| TractError::msg(format!("read image: {e}")))?;

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

    let vocab_list: Vec<&str> = VOCABULARY.to_vec();
    let vocab_json = serde_json::to_string(&vocab_list)?;
    let prompt = format!(
        "Return a JSON array of the top 5 most relevant photography tags for this image, \
        chosen only from this list: {vocab_json}. \
        Respond with only the JSON array, no explanation. Example: [\"portrait\", \"golden hour\"]"
    );

    let body = serde_json::json!({
        "model": "gpt-4o-mini",
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "image_url",
                    "image_url": { "url": data_url, "detail": "low" }
                },
                { "type": "text", "text": prompt }
            ]
        }],
        "max_tokens": 150
    });

    let response = ureq::post(api_endpoint)
        .header("Authorization", &format!("Bearer {api_key}"))
        .send_json(&body)
        .map_err(|e| TractError::msg(format!("API call failed: {e}")))?;

    let text = response
        .into_body()
        .read_to_string()
        .map_err(|e| TractError::msg(format!("read response: {e}")))?;

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| TractError::msg(format!("parse response JSON: {e}")))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("[]");

    // Strip markdown code fences if present
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

    Ok(serde_json::json!({ "tags": tags }))
}

fn embed_image(model: &TractModel, path: &str) -> TractResult<Vec<f32>> {
    let img = image::open(path)
        .map_err(|e| TractError::msg(e.to_string()))?
        .to_rgb8();
    let img = image::imageops::resize(
        &img,
        IMAGE_SIZE as u32,
        IMAGE_SIZE as u32,
        image::imageops::FilterType::Triangle,
    );
    let pixel_values: Tensor = tract_ndarray::Array4::from_shape_fn(
        (1, 3, IMAGE_SIZE, IMAGE_SIZE),
        |(_, c, y, x)| {
            let v = img[(x as u32, y as u32)][c] as f32 / 255.0;
            (v - CLIP_MEAN[c]) / CLIP_STD[c]
        },
    )
    .into();
    let outputs = model.run(tvec![pixel_values.into()])?;
    extract_embeddings(&outputs[0], 1).map(|mut v| v.remove(0))
}

fn extract_embeddings(tensor: &Tensor, batch: usize) -> TractResult<Vec<Vec<f32>>> {
    let view = tensor.to_array_view::<f32>()?;
    let flat: Vec<f32> = view.iter().copied().collect();
    let dim = flat.len() / batch;
    Ok(flat.chunks_exact(dim).map(|c| c.to_vec()).collect())
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}
