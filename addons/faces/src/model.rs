use std::io::Read;
use std::path::{Path, PathBuf};

use image::{RgbImage, imageops::FilterType};
use tract_onnx::prelude::{
    tvec, DatumExt, Framework, Graph, InferenceModelExt, RunnableModel, TValue, Tensor, TypedFact,
    TypedOp, tract_ndarray,
};

pub const MODEL_VERSION: &str = "scrfd-10g+arcface-w600k-r50-v1";

const BUFFALO_ZIP_URL: &str = "https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_l.zip";
const DET_FILENAME: &str = "det_10g.onnx";
const REC_FILENAME: &str = "w600k_r50.onnx";

const DET_INPUT_SIZE: usize = 640;
const REC_INPUT_SIZE: usize = 112;

// SCRFD: 3 strides, 2 anchors per location
const STRIDES: [usize; 3] = [8, 16, 32];
const NUM_ANCHORS: usize = 2;

// ArcFace target 5-point template for 112×112 output
const ARCFACE_DST: [[f32; 2]; 5] = [
    [38.2946, 51.6963],
    [73.5318, 51.5014],
    [56.0252, 71.7366],
    [41.5493, 92.3655],
    [70.7299, 92.2041],
];

type TractModel = RunnableModel<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

pub struct FaceModels {
    pub detector: TractModel,
    pub recognizer: TractModel,
}

pub struct DetectedFace {
    pub bbox_x: f32,
    pub bbox_y: f32,
    pub bbox_w: f32,
    pub bbox_h: f32,
    pub kps: [[f32; 2]; 5],
}

impl FaceModels {
    pub fn load(models_dir: &str, out: &mut impl std::io::Write) -> Result<Self, String> {
        let dir = PathBuf::from(models_dir).join("buffalo_l");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let det_path = dir.join(DET_FILENAME);
        let rec_path = dir.join(REC_FILENAME);

        if !det_path.exists() || !rec_path.exists() {
            download_and_extract_models(&dir, out)?;
        }

        if !det_path.exists() {
            return Err(format!("{DET_FILENAME} not found in {}", dir.display()));
        }
        if !rec_path.exists() {
            return Err(format!("{REC_FILENAME} not found in {}", dir.display()));
        }

        let detector = load_det_model(&det_path)?;
        let recognizer = load_rec_model(&rec_path)?;

        Ok(FaceModels { detector, recognizer })
    }

    pub fn detect(&self, img: &RgbImage) -> Result<Vec<DetectedFace>, String> {
        let (orig_w, orig_h) = img.dimensions();
        let input_size = DET_INPUT_SIZE as u32;

        let scale_x = orig_w as f32 / input_size as f32;
        let scale_y = orig_h as f32 / input_size as f32;

        let resized = image::imageops::resize(img, input_size, input_size, FilterType::Triangle);
        let tensor = rgb_to_tensor_scrfd(&resized, DET_INPUT_SIZE)?;

        let outputs = self
            .detector
            .run(tvec![tensor.into()])
            .map_err(|e| format!("detector run: {e}"))?;

        decode_scrfd_outputs(&outputs[..], scale_x, scale_y)
    }

    pub fn embed(&self, img: &RgbImage, face: &DetectedFace) -> Result<Vec<f32>, String> {
        let aligned = align_face(img, &face.kps);
        let tensor = rgb_to_tensor_arcface(&aligned, REC_INPUT_SIZE)?;

        let outputs = self
            .recognizer
            .run(tvec![tensor.into()])
            .map_err(|e| format!("recognizer run: {e}"))?;

        let view = outputs[0].to_array_view::<f32>().map_err(|e| e.to_string())?;
        let raw: Vec<f32> = view.iter().copied().collect();
        Ok(l2_normalize(raw))
    }
}

fn load_det_model(path: &Path) -> Result<TractModel, String> {
    tract_onnx::onnx()
        .model_for_path(path)
        .map_err(|e| format!("load det model: {e}"))?
        .with_input_fact(0, f32::fact([1usize, 3, DET_INPUT_SIZE, DET_INPUT_SIZE]).into())
        .map_err(|e| format!("set det input fact: {e}"))?
        .into_optimized()
        .map_err(|e| format!("optimize det model: {e}"))?
        .into_runnable()
        .map_err(|e| format!("build det runnable: {e}"))
}

fn load_rec_model(path: &Path) -> Result<TractModel, String> {
    tract_onnx::onnx()
        .model_for_path(path)
        .map_err(|e| format!("load rec model: {e}"))?
        .with_input_fact(0, f32::fact([1usize, 3, REC_INPUT_SIZE, REC_INPUT_SIZE]).into())
        .map_err(|e| format!("set rec input fact: {e}"))?
        .into_optimized()
        .map_err(|e| format!("optimize rec model: {e}"))?
        .into_runnable()
        .map_err(|e| format!("build rec runnable: {e}"))
}

fn rgb_to_tensor_scrfd(img: &RgbImage, size: usize) -> Result<Tensor, String> {
    let t: Tensor = tract_ndarray::Array4::from_shape_fn((1, 3, size, size), |(_, c, y, x)| {
        let p = img[(x as u32, y as u32)][c] as f32;
        (p - 127.5) / 128.0
    })
    .into();
    Ok(t)
}

fn rgb_to_tensor_arcface(img: &RgbImage, size: usize) -> Result<Tensor, String> {
    let t: Tensor = tract_ndarray::Array4::from_shape_fn((1, 3, size, size), |(_, c, y, x)| {
        let p = img[(x as u32, y as u32)][c] as f32;
        (p - 127.5) / 127.5
    })
    .into();
    Ok(t)
}

// SCRFD outputs (9 tensors for det_10g with keypoints):
// [0..2]: scores at strides 8, 16, 32 — shape [1, H*W*2, 1]
// [3..5]: bbox preds at strides 8, 16, 32 — shape [1, H*W*2, 4]
// [6..8]: kps preds at strides 8, 16, 32 — shape [1, H*W*2, 10]
fn decode_scrfd_outputs(
    outputs: &[TValue],
    scale_x: f32,
    scale_y: f32,
) -> Result<Vec<DetectedFace>, String> {
    const SCORE_THRESH: f32 = 0.5;
    const NMS_THRESH: f32 = 0.4;

    let mut faces: Vec<(f32, [f32; 4], [[f32; 2]; 5])> = Vec::new();

    if outputs.len() < 9 {
        return Err(format!("expected 9 SCRFD outputs, got {}", outputs.len()));
    }

    for (si, &stride) in STRIDES.iter().enumerate() {
        let h = DET_INPUT_SIZE / stride;
        let w = DET_INPUT_SIZE / stride;
        let n = h * w * NUM_ANCHORS;

        let scores_v = outputs[si].to_array_view::<f32>().map_err(|e| e.to_string())?;
        let bbox_v = outputs[si + 3].to_array_view::<f32>().map_err(|e| e.to_string())?;
        let kps_v = outputs[si + 6].to_array_view::<f32>().map_err(|e| e.to_string())?;

        let scores_len = scores_v.len();
        if scores_len < n {
            continue;
        }

        let anchors = generate_anchors(h, w, stride, NUM_ANCHORS);

        for i in 0..n {
            let score = sigmoid(scores_v.as_slice().unwrap_or(&[])[i]);
            if score < SCORE_THRESH {
                continue;
            }

            let cx = anchors[i][0];
            let cy = anchors[i][1];
            let s = stride as f32;

            let bbox_flat = bbox_v.as_slice().unwrap_or(&[]);
            let kps_flat = kps_v.as_slice().unwrap_or(&[]);
            let bi = i * 4;
            let ki = i * 10;
            if bi + 3 >= bbox_flat.len() || ki + 9 >= kps_flat.len() {
                continue;
            }

            let x1 = (cx - bbox_flat[bi] * s) * scale_x;
            let y1 = (cy - bbox_flat[bi + 1] * s) * scale_y;
            let x2 = (cx + bbox_flat[bi + 2] * s) * scale_x;
            let y2 = (cy + bbox_flat[bi + 3] * s) * scale_y;

            let mut kps = [[0f32; 2]; 5];
            for k in 0..5 {
                kps[k][0] = (cx + kps_flat[ki + k * 2] * s) * scale_x;
                kps[k][1] = (cy + kps_flat[ki + k * 2 + 1] * s) * scale_y;
            }

            faces.push((score, [x1, y1, x2, y2], kps));
        }
    }

    // NMS
    faces.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let kept = nms(&faces, NMS_THRESH);

    Ok(kept
        .into_iter()
        .map(|(_score, bbox, kps)| DetectedFace {
            bbox_x: bbox[0],
            bbox_y: bbox[1],
            bbox_w: bbox[2] - bbox[0],
            bbox_h: bbox[3] - bbox[1],
            kps,
        })
        .collect())
}

fn generate_anchors(h: usize, w: usize, stride: usize, num_anchors: usize) -> Vec<[f32; 2]> {
    let s = stride as f32;
    (0..h).flat_map(|row| {
        (0..w).flat_map(move |col| {
            std::iter::repeat_n([col as f32 * s, row as f32 * s], num_anchors)
        })
    }).collect()
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

fn nms(
    faces: &[(f32, [f32; 4], [[f32; 2]; 5])],
    thresh: f32,
) -> Vec<(f32, [f32; 4], [[f32; 2]; 5])> {
    let mut suppressed = vec![false; faces.len()];
    let mut kept = Vec::new();

    for i in 0..faces.len() {
        if suppressed[i] {
            continue;
        }
        kept.push(faces[i]);
        for j in (i + 1)..faces.len() {
            if suppressed[j] {
                continue;
            }
            if iou(&faces[i].1, &faces[j].1) > thresh {
                suppressed[j] = true;
            }
        }
    }
    kept
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let ix1 = a[0].max(b[0]);
    let iy1 = a[1].max(b[1]);
    let ix2 = a[2].min(b[2]);
    let iy2 = a[3].min(b[3]);
    let inter_w = (ix2 - ix1).max(0.0);
    let inter_h = (iy2 - iy1).max(0.0);
    let inter = inter_w * inter_h;
    let area_a = (a[2] - a[0]) * (a[3] - a[1]);
    let area_b = (b[2] - b[0]) * (b[3] - b[1]);
    let union = area_a + area_b - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

// Estimate similarity transform mapping src keypoints → ARCFACE_DST template,
// then apply it to produce a 112×112 aligned face crop.
fn align_face(img: &RgbImage, kps: &[[f32; 2]; 5]) -> RgbImage {
    let m = estimate_similarity(kps, &ARCFACE_DST);
    warp_affine(img, &m, REC_INPUT_SIZE as u32)
}

// Returns 2×3 affine matrix M: [dst_x, dst_y] = M * [src_x, src_y, 1]^T
// Uses 4-DOF similarity (scale + rotation + translation) estimated via least squares.
fn estimate_similarity(src: &[[f32; 2]; 5], dst: &[[f32; 2]; 5]) -> [[f32; 3]; 2] {
    // Linear system: [a, b, tx, ty] where transform is
    //   x_dst = a*x_src - b*y_src + tx
    //   y_dst = b*x_src + a*y_src + ty
    // Build normal equations A^T A x = A^T b
    let mut ata = [[0f64; 4]; 4];
    let mut atb = [0f64; 4];

    for (s, d) in src.iter().zip(dst.iter()) {
        let r1 = [s[0] as f64, -s[1] as f64, 1.0, 0.0];
        let r2 = [s[1] as f64, s[0] as f64, 0.0, 1.0];
        let b1 = d[0] as f64;
        let b2 = d[1] as f64;
        for i in 0..4 {
            for j in 0..4 {
                ata[i][j] += r1[i] * r1[j] + r2[i] * r2[j];
            }
            atb[i] += r1[i] * b1 + r2[i] * b2;
        }
    }

    let x = solve_4x4(&ata, &atb);
    let a = x[0] as f32;
    let b = x[1] as f32;
    let tx = x[2] as f32;
    let ty = x[3] as f32;

    [[a, -b, tx], [b, a, ty]]
}

fn solve_4x4(a: &[[f64; 4]; 4], b: &[f64; 4]) -> [f64; 4] {
    let mut m = [[0f64; 5]; 4];
    for i in 0..4 {
        for j in 0..4 {
            m[i][j] = a[i][j];
        }
        m[i][4] = b[i];
    }
    // Gaussian elimination with partial pivoting
    for col in 0..4 {
        let mut max_row = col;
        for row in (col + 1)..4 {
            if m[row][col].abs() > m[max_row][col].abs() {
                max_row = row;
            }
        }
        m.swap(col, max_row);
        let pivot = m[col][col];
        if pivot.abs() < 1e-12 {
            continue;
        }
        for row in (col + 1)..4 {
            let factor = m[row][col] / pivot;
            for j in col..5 {
                m[row][j] -= factor * m[col][j];
            }
        }
    }
    let mut x = [0f64; 4];
    for i in (0..4).rev() {
        x[i] = m[i][4];
        for j in (i + 1)..4 {
            x[i] -= m[i][j] * x[j];
        }
        if m[i][i].abs() > 1e-12 {
            x[i] /= m[i][i];
        }
    }
    x
}

fn warp_affine(img: &RgbImage, m: &[[f32; 3]; 2], out_size: u32) -> RgbImage {
    // Inverse of the similarity transform M = [[a,-b,tx],[b,a,ty]]:
    // det = a^2 + b^2
    // M^{-1} = [[a,b,-(a*tx+b*ty)], [-b,a,(b*tx-a*ty)]] / det
    let a = m[0][0];
    let b = m[1][0];
    let tx = m[0][2];
    let ty = m[1][2];
    let det = a * a + b * b;
    let (inv_a, inv_b, inv_tx, inv_ty) = if det > 1e-12 {
        (
            a / det,
            b / det,
            -(a * tx + b * ty) / det,
            (b * tx - a * ty) / det,
        )
    } else {
        (1.0, 0.0, 0.0, 0.0)
    };

    let (src_w, src_h) = img.dimensions();
    let mut out = RgbImage::new(out_size, out_size);

    for py in 0..out_size {
        for px in 0..out_size {
            let sx = inv_a * px as f32 - inv_b * py as f32 + inv_tx;
            let sy = inv_b * px as f32 + inv_a * py as f32 + inv_ty;

            if sx < 0.0 || sy < 0.0 || sx >= src_w as f32 - 1.0 || sy >= src_h as f32 - 1.0 {
                continue;
            }

            let x0 = sx.floor() as u32;
            let y0 = sy.floor() as u32;
            let x1 = x0 + 1;
            let y1 = y0 + 1;
            let fx = sx - x0 as f32;
            let fy = sy - y0 as f32;

            let p00 = img[(x0, y0)];
            let p10 = img[(x1, y0)];
            let p01 = img[(x0, y1)];
            let p11 = img[(x1, y1)];

            let r = bilerp(p00[0], p10[0], p01[0], p11[0], fx, fy);
            let g = bilerp(p00[1], p10[1], p01[1], p11[1], fx, fy);
            let b = bilerp(p00[2], p10[2], p01[2], p11[2], fx, fy);

            out[(px, py)] = image::Rgb([r as u8, g as u8, b as u8]);
        }
    }
    out
}

fn bilerp(v00: u8, v10: u8, v01: u8, v11: u8, fx: f32, fy: f32) -> f32 {
    let a = v00 as f32 * (1.0 - fx) + v10 as f32 * fx;
    let b = v01 as f32 * (1.0 - fx) + v11 as f32 * fx;
    a * (1.0 - fy) + b * fy
}

fn l2_normalize(v: Vec<f32>) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm < 1e-10 {
        v
    } else {
        v.into_iter().map(|x| x / norm).collect()
    }
}

fn download_and_extract_models(dir: &Path, out: &mut impl std::io::Write) -> Result<(), String> {
    emit_log(out, "info", "downloading face models from GitHub…");

    let resp = ureq::get(BUFFALO_ZIP_URL)
        .call()
        .map_err(|e| format!("download failed: {e}"))?;

    let mut zip_bytes = Vec::new();
    resp.into_body()
        .as_reader()
        .read_to_end(&mut zip_bytes)
        .map_err(|e| format!("read failed: {e}"))?;

    emit_log(out, "info", "extracting models…");

    let cursor = std::io::Cursor::new(&zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("invalid zip: {e}"))?;

    let needed = [DET_FILENAME, REC_FILENAME];
    for name in &needed {
        let mut file = archive.by_name(name).map_err(|e| format!("{name} not in archive: {e}"))?;
        let out_path = dir.join(name);
        let mut out_file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut out_file).map_err(|e| e.to_string())?;
        emit_log(out, "info", &format!("{name} ready"));
    }

    Ok(())
}

fn emit_log(out: &mut impl std::io::Write, level: &str, msg: &str) {
    let _ = writeln!(out, "{}", serde_json::json!({"type":"log","level":level,"message":msg}));
    let _ = out.flush();
}

