use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokenizers::Tokenizer;
use tract_onnx::prelude::{
    tvec, DatumExt, Framework, Graph, InferenceModelExt, RunnableModel, Tensor, TractError,
    TractResult, TypedFact, TypedOp, tract_ndarray,
};

const B32_SUBDIR: &str = "clip-vit-b32";
const B32_VISION_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model.onnx";
const B32_TEXT_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/text_model.onnx";
const B32_TOKENIZER_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/tokenizer.json";

const L14_SUBDIR: &str = "clip-vit-l14";
const L14_VISION_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-large-patch14/resolve/main/onnx/vision_model.onnx";
const L14_TEXT_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-large-patch14/resolve/main/onnx/text_model.onnx";
const L14_TOKENIZER_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-large-patch14/resolve/main/tokenizer.json";

const CLIP_MEAN: [f32; 3] = [0.48145466, 0.4578275, 0.40821073];
const CLIP_STD: [f32; 3] = [0.26862954, 0.26130258, 0.27577711];
const IMAGE_SIZE: usize = 224;
const TOKEN_LEN: usize = 77;
const SOT_TOKEN: i64 = 49406;
const EOT_TOKEN: i64 = 49407;

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

type TractModel = RunnableModel<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

#[derive(Deserialize, Default)]
struct Config {
    #[serde(default = "default_variant")]
    variant: String,
    #[serde(default)]
    vocabulary_file: String,
}

fn default_variant() -> String {
    "clip-vit-b32".to_string()
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

    let (vision_model, vocab_labels, vocab_embeds) = match init(&config, &models_base, &mut out) {
        Ok(r) => r,
        Err(e) => {
            emit_log(&mut out, "error", &format!("init failed: {e}"));
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
                    "classify" => match handle_classify(&vision_model, &vocab_labels, &vocab_embeds, &req.params) {
                        Ok(r) => {
                            serde_json::to_string(&Response { id: req.id, result: r }).unwrap()
                        }
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
            Err(e) => eprintln!("[autotag-clip] parse error: {e}"),
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

fn init(
    config: &Config,
    models_base: &str,
    out: &mut impl Write,
) -> Result<(TractModel, Vec<String>, Vec<Vec<f32>>), String> {
    let (subdir, vision_url, text_url, tokenizer_url) = match config.variant.as_str() {
        "clip-vit-l14" => (L14_SUBDIR, L14_VISION_URL, L14_TEXT_URL, L14_TOKENIZER_URL),
        _ => (B32_SUBDIR, B32_VISION_URL, B32_TEXT_URL, B32_TOKENIZER_URL),
    };
    let model_dir = PathBuf::from(models_base).join(subdir);

    ensure_models(&model_dir, vision_url, text_url, tokenizer_url, out)
        .map_err(|e| format!("model download failed: {e}"))?;

    let vocab = load_vocabulary(config, out);

    let vision_model =
        load_vision_model(model_dir.join("vision_model.onnx")).map_err(|e| e.to_string())?;
    let text_model =
        load_text_model(model_dir.join("text_model.onnx"), vocab.len()).map_err(|e| e.to_string())?;
    let tokenizer =
        Tokenizer::from_file(model_dir.join("tokenizer.json")).map_err(|e| e.to_string())?;

    emit_log(out, "info", &format!("embedding {} vocabulary tags…", vocab.len()));
    let vocab_embeds = embed_vocabulary_from(&text_model, &tokenizer, &vocab).map_err(|e| e.to_string())?;
    emit_log(out, "info", &format!("ready ({}, {} tags)", config.variant, vocab.len()));

    Ok((vision_model, vocab, vocab_embeds))
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
    std::io::copy(&mut response.into_body().as_reader(), &mut file)
        .map_err(|e| e.to_string())?;
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

fn embed_vocabulary_from(model: &TractModel, tokenizer: &Tokenizer, vocab: &[String]) -> TractResult<Vec<Vec<f32>>> {
    let n = vocab.len();
    let mut flat_ids = Vec::with_capacity(n * TOKEN_LEN);
    let mut flat_mask = Vec::with_capacity(n * TOKEN_LEN);
    for label in vocab {
        let (ids, mask) = tokenize(tokenizer, label);
        flat_ids.extend(ids);
        flat_mask.extend(mask);
    }
    let ids_t: Tensor = tract_ndarray::Array2::from_shape_vec((n, TOKEN_LEN), flat_ids)?.into();
    let mask_t: Tensor = tract_ndarray::Array2::from_shape_vec((n, TOKEN_LEN), flat_mask)?.into();
    let outputs = model.run(tvec![ids_t.into(), mask_t.into()])?;
    extract_embeddings(&outputs[0], n)
}

fn handle_classify(
    model: &TractModel,
    vocab_labels: &[String],
    vocab_embeds: &[Vec<f32>],
    params: &Value,
) -> TractResult<Value> {
    let thumb_path = params
        .get("thumbnail_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TractError::msg("missing thumbnail_path"))?;

    let img_embed = embed_image(model, thumb_path)?;

    let mut scores: Vec<(f32, &str)> = vocab_embeds
        .iter()
        .zip(vocab_labels.iter())
        .map(|(emb, label)| (cosine_sim(&img_embed, emb), label.as_str()))
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
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}
