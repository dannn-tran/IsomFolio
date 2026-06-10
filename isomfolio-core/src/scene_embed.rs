//! Cheap, all-Rust image embeddings for *permissive* scene grouping — the
//! "several tries at one shot / same scene, reframed" axis that dHash stacks
//! deliberately don't cover (see `dev-docs/stacking-permissive-clustering.md`).
//!
//! This is an interim, no-ML descriptor: a GIST-lite vector of coarse spatial
//! colour, a global HSV histogram, and a gradient-orientation histogram. Each
//! sub-descriptor is L2-normalised so the three contribute comparably, then the
//! whole vector is L2-normalised so it feeds `clustering::dbscan` (cosine
//! distance) directly — the same shape face embeddings use. It is weaker than a
//! CLIP image encoder (won't survive heavy reframing) but far better than
//! widening dHash, and needs no model or new dependency.

use image::imageops::FilterType;
use image::DynamicImage;

/// Working resolution the thumbnail is reduced to before descriptors are taken.
/// Small enough to be cheap, large enough for a 2×2 orientation grid to mean
/// something.
const WORK: u32 = 64;

/// Spatial-colour grid: `GRID×GRID` cells, mean RGB each.
const GRID: u32 = 4;
/// Global HSV histogram bins (hue carries palette; sat/val carry tone).
const H_BINS: usize = 12;
const S_BINS: usize = 4;
const V_BINS: usize = 4;
/// Orientation histogram: `OR_GRID×OR_GRID` cells × `OR_BINS` gradient angles.
const OR_GRID: u32 = 2;
const OR_BINS: usize = 8;

/// Total descriptor length, for callers that pre-size storage.
pub const SCENE_EMBED_DIM: usize =
    (GRID * GRID) as usize * 3 + (H_BINS + S_BINS + V_BINS) + (OR_GRID * OR_GRID) as usize * OR_BINS;

/// Compute the L2-normalised scene descriptor for an already-decoded image
/// (typically the cached thumbnail). Pure and deterministic. The returned vector
/// has unit length (or is all-zero for a degenerate/empty image), so cosine
/// distance against another descriptor is `1 - dot`.
pub fn scene_embedding(img: &DynamicImage) -> Vec<f32> {
    let rgb = img.resize_exact(WORK, WORK, FilterType::Triangle).to_rgb8();

    let mut color = grid_color(&rgb);
    let mut hist = hsv_histogram(&rgb);
    let mut orient = orientation_histogram(&rgb);
    l2_normalize(&mut color);
    l2_normalize(&mut hist);
    l2_normalize(&mut orient);

    let mut v = Vec::with_capacity(SCENE_EMBED_DIM);
    v.extend(color);
    v.extend(hist);
    v.extend(orient);
    l2_normalize(&mut v);
    v
}

/// Mean RGB per cell of a `GRID×GRID` partition, channels scaled to `0..1`.
/// Coarse layout + palette; tolerant of a mild pan because cells are large.
fn grid_color(rgb: &image::RgbImage) -> Vec<f32> {
    let (w, h) = rgb.dimensions();
    let mut sums = vec![[0f64; 3]; (GRID * GRID) as usize];
    let mut counts = vec![0u32; (GRID * GRID) as usize];
    for (x, y, p) in rgb.enumerate_pixels() {
        let cx = (x * GRID / w).min(GRID - 1);
        let cy = (y * GRID / h).min(GRID - 1);
        let cell = (cy * GRID + cx) as usize;
        for c in 0..3 {
            sums[cell][c] += p[c] as f64;
        }
        counts[cell] += 1;
    }
    let mut out = Vec::with_capacity((GRID * GRID) as usize * 3);
    for (cell, count) in counts.iter().enumerate() {
        let n = (*count).max(1) as f64;
        for c in 0..3 {
            out.push((sums[cell][c] / n / 255.0) as f32);
        }
    }
    out
}

/// Marginal HSV histograms concatenated. Position-free, so it survives reframing
/// of the same subject/palette better than any spatial descriptor.
fn hsv_histogram(rgb: &image::RgbImage) -> Vec<f32> {
    let mut h_hist = vec![0f32; H_BINS];
    let mut s_hist = vec![0f32; S_BINS];
    let mut v_hist = vec![0f32; V_BINS];
    for p in rgb.pixels() {
        let (hue, sat, val) = rgb_to_hsv(p[0], p[1], p[2]);
        let hb = ((hue / 360.0 * H_BINS as f32) as usize).min(H_BINS - 1);
        let sb = ((sat * S_BINS as f32) as usize).min(S_BINS - 1);
        let vb = ((val * V_BINS as f32) as usize).min(V_BINS - 1);
        h_hist[hb] += 1.0;
        s_hist[sb] += 1.0;
        v_hist[vb] += 1.0;
    }
    let mut out = Vec::with_capacity(H_BINS + S_BINS + V_BINS);
    out.extend(h_hist);
    out.extend(s_hist);
    out.extend(v_hist);
    out
}

/// Per-cell gradient-orientation histogram (HOG-lite) over luma, each angle bin
/// weighted by gradient magnitude. Captures coarse structure/edges, giving the
/// descriptor a non-colour axis so two scenes with similar palettes but
/// different layout still separate.
fn orientation_histogram(rgb: &image::RgbImage) -> Vec<f32> {
    let (w, h) = rgb.dimensions();
    let luma: Vec<f32> = rgb
        .pixels()
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
        .collect();
    let at = |x: u32, y: u32| luma[(y * w + x) as usize];

    let mut hist = vec![0f32; (OR_GRID * OR_GRID) as usize * OR_BINS];
    if w < 3 || h < 3 {
        return hist;
    }
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let gx = at(x + 1, y) - at(x - 1, y);
            let gy = at(x, y + 1) - at(x, y - 1);
            let mag = (gx * gx + gy * gy).sqrt();
            if mag <= f32::EPSILON {
                continue;
            }
            // Unsigned orientation in [0, π) — an edge and its reverse are the same.
            let mut ang = gy.atan2(gx);
            if ang < 0.0 {
                ang += std::f32::consts::PI;
            }
            let bin = ((ang / std::f32::consts::PI * OR_BINS as f32) as usize).min(OR_BINS - 1);
            let cx = (x * OR_GRID / w).min(OR_GRID - 1);
            let cy = (y * OR_GRID / h).min(OR_GRID - 1);
            let cell = (cy * OR_GRID + cx) as usize;
            hist[cell * OR_BINS + bin] += mag;
        }
    }
    hist
}

fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let hue = if hue < 0.0 { hue + 360.0 } else { hue };
    let sat = if max <= f32::EPSILON { 0.0 } else { delta / max };
    (hue, sat, max)
}

/// One file's scene-grouping inputs: its embedding and a sharpness score (used
/// only to pick the group's default keeper — the sharpest frame).
#[derive(Debug, Clone)]
pub struct SceneItem {
    pub embedding: Vec<f32>,
    pub sharpness: f64,
}

/// Group files into *scenes* by clustering their embeddings (cosine distance,
/// `eps` = cosine-distance radius) and dropping noise/singletons, mirroring the
/// `phash::group_stacks` contract so the result drops into the same Review queue:
/// each returned group is ≥ 2 indices into `items`, **sharpest frame first** as
/// the default keeper. Pure — order of `items` is preserved within the sharpness
/// tie-break. This is the embedding-based queue source behind "Review Scenes".
pub fn group_scenes(items: &[SceneItem], eps: f32, min_pts: usize) -> Vec<Vec<usize>> {
    use std::collections::BTreeMap;
    if items.is_empty() {
        return Vec::new();
    }
    let embeddings: Vec<Vec<f32>> = items.iter().map(|i| i.embedding.clone()).collect();
    let labels = crate::clustering::dbscan(&embeddings, eps, min_pts);

    let mut by_label: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
    for (idx, &label) in labels.iter().enumerate() {
        if label >= 0 {
            by_label.entry(label).or_default().push(idx);
        }
    }

    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (_, mut members) in by_label {
        if members.len() < 2 {
            continue;
        }
        // Sharpest first = default keeper; stable on ties via the original index.
        members.sort_by(|&a, &b| {
            items[b].sharpness
                .partial_cmp(&items[a].sharpness)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.cmp(&b))
        });
        groups.push(members);
    }
    groups
}

fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgb, RgbImage};

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    /// A diagonal colour-ramp "scene": deterministic, with structure + palette.
    fn scene(shift: i32) -> DynamicImage {
        let mut img = RgbImage::new(64, 64);
        for y in 0..64i32 {
            for x in 0..64i32 {
                let sx = (x + shift).rem_euclid(64) as u32;
                let r = (sx * 4).min(255) as u8;
                let g = (y * 4).min(255) as u8;
                let b = ((sx + y as u32) * 2).min(255) as u8;
                img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
            }
        }
        DynamicImage::ImageRgb8(img)
    }

    mod scene_embedding_fn {
        use super::*;

        #[test]
        fn is_deterministic_and_unit_length() {
            let a = scene_embedding(&scene(0));
            let b = scene_embedding(&scene(0));
            assert_eq!(a, b);
            assert_eq!(a.len(), SCENE_EMBED_DIM);
            let norm = a.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-4, "norm {norm} not unit");
        }

        #[test]
        fn reframed_scene_stays_closer_than_a_different_one() {
            // Same scene panned a few pixels should be more cosine-similar than a
            // tonally inverted, opposite-structure image — the property permissive
            // grouping relies on.
            let base = scene_embedding(&scene(0));
            let panned = scene_embedding(&scene(4));

            // A different scene: horizontal colour bands — unlike the diagonal
            // ramp in palette, spatial layout, and dominant edge orientation.
            let mut bands = RgbImage::new(64, 64);
            for y in 0..64u32 {
                let c = match y / 16 {
                    0 => Rgb([220, 30, 30]),
                    1 => Rgb([30, 220, 30]),
                    2 => Rgb([30, 30, 220]),
                    _ => Rgb([220, 220, 30]),
                };
                for x in 0..64u32 {
                    bands.put_pixel(x, y, c);
                }
            }
            let different = scene_embedding(&DynamicImage::ImageRgb8(bands));

            let sim_panned = cosine(&base, &panned);
            let sim_diff = cosine(&base, &different);
            assert!(
                sim_panned > sim_diff + 0.05,
                "panned {sim_panned} not clearly closer than different {sim_diff}"
            );
        }

        #[test]
        fn degenerate_image_yields_finite_vector() {
            let flat = DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 64, Rgb([128, 128, 128])));
            let v = scene_embedding(&flat);
            assert_eq!(v.len(), SCENE_EMBED_DIM);
            assert!(v.iter().all(|x| x.is_finite()));
        }
    }

    mod group_scenes_fn {
        use super::*;

        fn unit(v: [f32; 4]) -> Vec<f32> {
            let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            v.iter().map(|x| x / n).collect()
        }

        fn item(emb: [f32; 4], sharpness: f64) -> SceneItem {
            SceneItem { embedding: unit(emb), sharpness }
        }

        #[test]
        fn clusters_scenes_dropping_noise_and_singletons() {
            // Cluster A near e0, cluster B near e1, one outlier near e2.
            let items = [
                item([1.0, 0.05, 0.0, 0.0], 0.3), // A
                item([1.0, 0.0, 0.05, 0.0], 0.9), // A
                item([1.0, 0.03, 0.03, 0.0], 0.5), // A
                item([0.05, 1.0, 0.0, 0.0], 0.4), // B
                item([0.0, 1.0, 0.05, 0.0], 0.8), // B
                item([0.03, 1.0, 0.03, 0.0], 0.6), // B
                item([0.0, 0.0, 1.0, 0.0], 0.7), // outlier
            ];
            let groups = group_scenes(&items, 0.1, 2);
            assert_eq!(groups.len(), 2);
            let sizes: Vec<usize> = groups.iter().map(|g| g.len()).collect();
            assert!(sizes.iter().all(|&s| s == 3), "got sizes {sizes:?}");
            // Outlier index 6 appears in no group.
            assert!(groups.iter().all(|g| !g.contains(&6)));
        }

        #[test]
        fn keeper_is_sharpest_first() {
            let items = [
                item([1.0, 0.05, 0.0, 0.0], 0.3),
                item([1.0, 0.0, 0.05, 0.0], 0.9), // sharpest
                item([1.0, 0.03, 0.03, 0.0], 0.5),
            ];
            let groups = group_scenes(&items, 0.1, 2);
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0][0], 1, "sharpest frame must lead the group");
        }

        #[test]
        fn empty_input_yields_no_groups() {
            assert!(group_scenes(&[], 0.1, 2).is_empty());
        }
    }

    mod rgb_to_hsv_fn {
        use super::*;

        #[test]
        fn primaries_map_to_expected_hues() {
            assert!((rgb_to_hsv(255, 0, 0).0 - 0.0).abs() < 1.0);
            assert!((rgb_to_hsv(0, 255, 0).0 - 120.0).abs() < 1.0);
            assert!((rgb_to_hsv(0, 0, 255).0 - 240.0).abs() < 1.0);
        }

        #[test]
        fn grey_is_zero_saturation() {
            let (_, s, v) = rgb_to_hsv(100, 100, 100);
            assert!(s < 1e-4);
            assert!((v - 100.0 / 255.0).abs() < 1e-4);
        }
    }
}
