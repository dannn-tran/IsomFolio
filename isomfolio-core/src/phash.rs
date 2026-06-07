//! Perceptual hashing for content-based stack detection. A "stack" groups
//! near-identical frames shot close together in time — inferred from pixels, not
//! camera burst metadata. The hash (dHash) and a sharpness score are computed
//! once per file from its cached thumbnail; grouping is a pure, per-folder walk.

use image::DynamicImage;

/// 64-bit difference hash. The image is reduced to 9×8 grayscale and each pixel
/// is compared with its right neighbour, yielding one bit per comparison (8×8 =
/// 64). Robust to resolution, mild compression and small tonal shifts; two
/// near-duplicate frames differ in only a handful of bits.
pub fn dhash(img: &DynamicImage) -> u64 {
    let small = img
        .resize_exact(9, 8, image::imageops::FilterType::Triangle)
        .to_luma8();
    let mut hash = 0u64;
    let mut bit = 0u32;
    for y in 0..8 {
        for x in 0..8 {
            let left = small.get_pixel(x, y)[0];
            let right = small.get_pixel(x + 1, y)[0];
            if left > right {
                hash |= 1 << bit;
            }
            bit += 1;
        }
    }
    hash
}

/// Number of differing bits between two hashes — the perceptual distance.
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Variance of the Laplacian over the image luma — a focus/sharpness proxy used
/// to pick the sharpest frame as a stack's representative. Flat images score ~0;
/// crisp, high-detail images score high.
pub fn sharpness(img: &DynamicImage) -> f64 {
    let g = img.to_luma8();
    let (w, h) = g.dimensions();
    if w < 3 || h < 3 {
        return 0.0;
    }
    let at = |x: u32, y: u32| g.get_pixel(x, y)[0] as f64;
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let mut n = 0.0;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let lap = at(x - 1, y) + at(x + 1, y) + at(x, y - 1) + at(x, y + 1) - 4.0 * at(x, y);
            sum += lap;
            sum_sq += lap * lap;
            n += 1.0;
        }
    }
    if n == 0.0 {
        return 0.0;
    }
    let mean = sum / n;
    (sum_sq / n) - mean * mean
}

/// One file's stacking inputs: its perceptual hash and capture time (seconds).
#[derive(Debug, Clone, Copy)]
pub struct HashedFile {
    pub hash: u64,
    pub time: i64,
}

/// Group consecutive near-duplicate frames within a folder. Files are walked in
/// capture order; a frame joins the current stack when it is within `threshold`
/// Hamming distance of the stack's **representative** (its first frame — keeps the
/// group visually anchored, no drift) **and** within `window_secs` of the
/// previous frame (temporal contiguity, so the same scene reshot hours later
/// starts a new stack). Only groups of ≥ 2 are returned, as vectors of indices
/// into `items`. Pure — `items` need not be pre-sorted.
pub fn group_stacks(items: &[HashedFile], threshold: u32, window_secs: i64) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| (items[i].time, i));

    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut rep_hash: u64 = 0;
    let mut prev_time: i64 = 0;

    for &i in &order {
        let f = items[i];
        let joins = match current.last() {
            None => false,
            Some(_) => {
                hamming(rep_hash, f.hash) <= threshold && (f.time - prev_time) <= window_secs
            }
        };
        if joins {
            current.push(i);
        } else {
            if current.len() >= 2 {
                groups.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
            current.push(i);
            rep_hash = f.hash;
        }
        prev_time = f.time;
    }
    if current.len() >= 2 {
        groups.push(current);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GrayImage, Luma};

    fn gradient(offset: u8) -> DynamicImage {
        let mut img = GrayImage::new(16, 16);
        for y in 0..16u32 {
            for x in 0..16u32 {
                let v = ((x * 16) as u8).wrapping_add(offset);
                img.put_pixel(x, y, Luma([v]));
            }
        }
        DynamicImage::ImageLuma8(img)
    }

    mod dhash_fn {
        use super::*;

        #[test]
        fn is_deterministic() {
            assert_eq!(dhash(&gradient(0)), dhash(&gradient(0)));
        }

        #[test]
        fn near_duplicate_is_close() {
            // A uniform tonal shift should leave most difference-bits unchanged.
            let d = hamming(dhash(&gradient(0)), dhash(&gradient(8)));
            assert!(d <= 4, "near-dup distance {d} too large");
        }

        #[test]
        fn different_image_is_far() {
            // Reverse gradient: left>right everywhere, the opposite bit pattern.
            let mut rev = GrayImage::new(16, 16);
            for y in 0..16u32 {
                for x in 0..16u32 {
                    rev.put_pixel(x, y, Luma([((15 - x) * 16) as u8]));
                }
            }
            let d = hamming(dhash(&gradient(0)), dhash(&DynamicImage::ImageLuma8(rev)));
            assert!(d >= 4, "distinct-image distance {d} too small");
        }
    }

    mod hamming_fn {
        use super::*;

        #[test]
        fn counts_differing_bits() {
            assert_eq!(hamming(0, 0), 0);
            assert_eq!(hamming(0b1011, 0b0010), 2);
            assert_eq!(hamming(u64::MAX, 0), 64);
        }
    }

    mod sharpness_fn {
        use super::*;

        #[test]
        fn flat_image_scores_zero() {
            let flat = DynamicImage::ImageLuma8(GrayImage::from_pixel(16, 16, Luma([100])));
            assert_eq!(sharpness(&flat), 0.0);
        }

        #[test]
        fn detailed_image_scores_positive() {
            let mut img = GrayImage::new(16, 16);
            for y in 0..16u32 {
                for x in 0..16u32 {
                    let v = if (x + y) % 2 == 0 { 0 } else { 255 };
                    img.put_pixel(x, y, Luma([v]));
                }
            }
            assert!(sharpness(&DynamicImage::ImageLuma8(img)) > 0.0);
        }
    }

    mod group_stacks_fn {
        use super::*;

        fn f(hash: u64, time: i64) -> HashedFile {
            HashedFile { hash, time }
        }

        #[test]
        fn groups_near_dupes_shot_close() {
            let items = [f(0b0000, 0), f(0b0001, 1), f(0b0011, 2)];
            let groups = group_stacks(&items, 2, 10);
            assert_eq!(groups, vec![vec![0, 1, 2]]);
        }

        #[test]
        fn splits_on_scene_change_despite_close_time() {
            // Second frame is visually distant — a pan, not a near-dup.
            let items = [f(0, 0), f(u64::MAX, 1), f(u64::MAX, 2)];
            let groups = group_stacks(&items, 4, 10);
            assert_eq!(groups, vec![vec![1, 2]]);
        }

        #[test]
        fn splits_on_time_gap_despite_identical_pixels() {
            let items = [f(0, 0), f(0, 1), f(0, 100)];
            let groups = group_stacks(&items, 0, 10);
            assert_eq!(groups, vec![vec![0, 1]]);
        }

        #[test]
        fn joins_near_dupes_across_a_three_second_gap() {
            // The old time-only detector (≤3s) missed this; pixels carry it.
            let items = [f(0, 0), f(0b1, 6)];
            let groups = group_stacks(&items, 2, 10);
            assert_eq!(groups, vec![vec![0, 1]]);
        }

        #[test]
        fn excludes_singletons() {
            let items = [f(0, 0), f(u64::MAX, 50)];
            assert!(group_stacks(&items, 2, 10).is_empty());
        }

        #[test]
        fn representative_anchored_to_first_frame_prevents_drift() {
            // Each step is within threshold of the previous, but the last is far
            // from the first; anchoring to the rep must NOT let it chain in.
            let items = [f(0b0000_0000, 0), f(0b0000_1111, 1), f(0b1111_1111, 2)];
            let groups = group_stacks(&items, 4, 10);
            assert_eq!(groups, vec![vec![0, 1]]);
        }
    }
}
