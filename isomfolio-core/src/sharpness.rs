//! Relative sharpness (focus) scoring. Used only to mark the sharper frame when
//! similar shots are compared directly (Compare) — never as an absolute "blurry"
//! verdict, since the score is scene-dependent.

use image::DynamicImage;

/// Variance of the Laplacian over the image luma — a focus/sharpness proxy. Flat
/// images score ~0; crisp, high-detail images score high. Only meaningful
/// *relative* to a similar frame.
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};

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
