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
    #[serde(default = "default_batch_size")]
    batch_size: String,
}

fn default_batch_size() -> String {
    "auto".to_string()
}

fn resolve_batch_size(config: &Config, out: &mut impl Write) -> usize {
    if config.batch_size != "auto" {
        if let Ok(n) = config.batch_size.parse::<usize>() {
            if n >= 1 {
                return n;
            }
        }
    }
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    let size = match cores {
        0..=2 => 1,
        3..=4 => 2,
        5..=8 => 4,
        _ => 8,
    };
    emit_log(out, "info", &format!("auto batch_size={size} ({cores} cores)"));
    size
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
    let batch_size = resolve_batch_size(&config, &mut out);

    let (vision_model, vocab_labels, vocab_embeds) = match init(&config, &models_base, batch_size, &mut out) {
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

    let (line_tx, line_rx) = std::sync::mpsc::sync_channel::<String>(128);
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let line = line.trim().to_string();
            if line.is_empty() { continue; }
            if line_tx.send(line).is_err() { break; }
        }
    });

    loop {
        let Ok(first) = line_rx.recv() else { break };
        let mut lines = vec![first];
        while lines.len() < batch_size {
            match line_rx.recv_timeout(std::time::Duration::from_millis(5)) {
                Ok(line) => lines.push(line),
                Err(_) => break,
            }
        }

        let mut classify_reqs: Vec<(u64, Value)> = Vec::new();
        for line in &lines {
            match serde_json::from_str::<Request>(line) {
                Ok(req) if req.method == "classify" => {
                    classify_reqs.push((req.id, req.params));
                }
                Ok(req) => {
                    let resp = serde_json::to_string(&ErrorResponse {
                        id: req.id,
                        error: format!("unknown method: {}", req.method),
                    }).unwrap();
                    let _ = writeln!(out, "{resp}");
                }
                Err(e) => eprintln!("[autotag-clip] parse error: {e}"),
            }
        }

        if classify_reqs.is_empty() {
            let _ = out.flush();
            continue;
        }

        let results = classify_batch(&vision_model, &vocab_labels, &vocab_embeds, &classify_reqs, batch_size);
        for (id, result) in results {
            let resp = match result {
                Ok(r) => serde_json::to_string(&Response { id, result: r }).unwrap(),
                Err(e) => serde_json::to_string(&ErrorResponse { id, error: e }).unwrap(),
            };
            let _ = writeln!(out, "{resp}");
        }
        let _ = out.flush();
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
    batch_size: usize,
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
        load_vision_model(model_dir.join("vision_model.onnx"), batch_size).map_err(|e| e.to_string())?;
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

fn load_vision_model(path: PathBuf, batch: usize) -> TractResult<TractModel> {
    tract_onnx::onnx()
        .model_for_path(path)?
        .with_input_fact(0, f32::fact([batch, 3, IMAGE_SIZE, IMAGE_SIZE]).into())?
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

fn preprocess_image(path: &str) -> TractResult<Vec<f32>> {
    let img = image::open(path)
        .map_err(|e| TractError::msg(e.to_string()))?
        .to_rgb8();
    let img = image::imageops::resize(
        &img,
        IMAGE_SIZE as u32,
        IMAGE_SIZE as u32,
        image::imageops::FilterType::Triangle,
    );
    let mut pixels = vec![0.0f32; 3 * IMAGE_SIZE * IMAGE_SIZE];
    for c in 0..3 {
        for y in 0..IMAGE_SIZE {
            for x in 0..IMAGE_SIZE {
                let v = img[(x as u32, y as u32)][c] as f32 / 255.0;
                pixels[c * IMAGE_SIZE * IMAGE_SIZE + y * IMAGE_SIZE + x] = (v - CLIP_MEAN[c]) / CLIP_STD[c];
            }
        }
    }
    Ok(pixels)
}

fn embed_images(model: &TractModel, pixel_batches: &[Vec<f32>], model_batch_size: usize) -> TractResult<Vec<Vec<f32>>> {
    let n = pixel_batches.len();
    let img_len = 3 * IMAGE_SIZE * IMAGE_SIZE;
    let mut flat = vec![0.0f32; model_batch_size * img_len];
    for (i, pixels) in pixel_batches.iter().enumerate() {
        flat[i * img_len..(i + 1) * img_len].copy_from_slice(pixels);
    }
    let tensor: Tensor = tract_ndarray::Array4::from_shape_vec(
        (model_batch_size, 3, IMAGE_SIZE, IMAGE_SIZE), flat,
    )?.into();
    let outputs = model.run(tvec![tensor.into()])?;
    let mut all = extract_embeddings(&outputs[0], model_batch_size)?;
    all.truncate(n);
    Ok(all)
}

fn score_embedding(embed: &[f32], vocab_labels: &[String], vocab_embeds: &[Vec<f32>]) -> Vec<(String, f32)> {
    let mut scores: Vec<(f32, &str)> = vocab_embeds
        .iter()
        .zip(vocab_labels.iter())
        .map(|(emb, label)| (cosine_sim(embed, emb), label.as_str()))
        .collect();
    scores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scores.iter().take(5).filter(|(s, _)| *s > 0.0).map(|(s, l)| (l.to_string(), *s)).collect()
}

fn classify_batch(
    model: &TractModel,
    vocab_labels: &[String],
    vocab_embeds: &[Vec<f32>],
    reqs: &[(u64, Value)],
    model_batch_size: usize,
) -> Vec<(u64, Result<Value, String>)> {
    let mut loaded: Vec<(usize, Vec<f32>)> = Vec::new();
    let mut results: Vec<(u64, Result<Value, String>)> = Vec::with_capacity(reqs.len());

    for (i, (_, params)) in reqs.iter().enumerate() {
        let thumb = params.get("thumbnail_path").and_then(|v| v.as_str()).unwrap_or("");
        if thumb.is_empty() || !Path::new(thumb).exists() {
            continue;
        }
        match preprocess_image(thumb) {
            Ok(pixels) => loaded.push((i, pixels)),
            Err(e) => {
                results.push((reqs[i].0, Err(e.to_string())));
            }
        }
    }

    let pixels_only: Vec<Vec<f32>> = loaded.iter().map(|(_, p)| p.clone()).collect();
    let embeddings = match embed_images(model, &pixels_only, model_batch_size) {
        Ok(e) => e,
        Err(e) => {
            for (i, _) in &loaded {
                results.push((reqs[*i].0, Err(e.to_string())));
            }
            return results;
        }
    };

    for ((orig_idx, _), embed) in loaded.iter().zip(embeddings.iter()) {
        let file_id = reqs[*orig_idx].1.get("file_id").and_then(|v| v.as_str()).unwrap_or("");
        let scored = score_embedding(embed, vocab_labels, vocab_embeds);
        let tags: Vec<Value> = scored.iter().map(|(t, c)| serde_json::json!({"tag": t, "confidence": c})).collect();
        results.push((reqs[*orig_idx].0, Ok(serde_json::json!({"file_id": file_id, "tags": tags}))));
    }

    results
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
