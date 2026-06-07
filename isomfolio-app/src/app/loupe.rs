//! Loupe action model: one vocabulary of *intents*, one geometry value object,
//! and one pure reducer that every input path funnels through. Trackpad scroll
//! and drag (in the `LoupeImage` widget), the zoom buttons, and the keyboard
//! shortcuts all lower to a [`LoupeIntent`] and apply it via [`LoupeGeometry`].
//! That removes the old two-sources-of-truth split where the widget zoomed
//! toward the cursor with one formula and the buttons re-centred with another.

use iced::{ContentFit, Point, Size, Vector};

pub use crate::app::{LOUPE_ZOOM_MAX as ZOOM_MAX, LOUPE_ZOOM_MIN as ZOOM_MIN};

/// Multiplicative step for one scroll notch / button press (10% per step).
pub const ZOOM_STEP: f32 = 0.10;

/// Discrete zoom targets a "zoom to" intent can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomLevel {
    /// Fit-to-window (zoom == [`ZOOM_MIN`]).
    Fit,
    /// 1:1 — one image pixel per screen pixel.
    Actual,
}

/// The full input vocabulary of the loupe. Every gesture, key, and button maps
/// to exactly one of these; nothing mutates zoom/pan outside the reducer.
#[derive(Debug, Clone, Copy)]
pub enum LoupeIntent {
    /// Multiply the zoom by `factor`, keeping `anchor` (a point in viewport
    /// coordinates, origin at the viewport's top-left) stationary. Scroll and
    /// the +/- buttons.
    ZoomAround { anchor: Point, factor: f32 },
    /// Jump to a named level, keeping `anchor` stationary. 1:1 / Fit / click.
    ZoomTo { level: ZoomLevel, anchor: Point },
    /// Set the pan offset directly (the widget's drag computes it from the grab
    /// origin); the reducer only clamps it to the image edges.
    PanTo(Vector),
    /// Back to fit, pan zeroed.
    Reset,
}

/// Resulting zoom + pan after applying an intent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoupeZoom {
    pub zoom: f32,
    pub offset: Vector,
}

/// What a plain click would do at a given zoom — the single source the widget
/// uses for *both* the click intent and the cursor shape, so they can't drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickAction {
    ZoomIn,
    ZoomOut,
}

pub fn click_action(zoom: f32) -> ClickAction {
    if zoom > ZOOM_MIN {
        ClickAction::ZoomOut
    } else {
        ClickAction::ZoomIn
    }
}

/// The loupe image area and the photo's native pixel size — everything the
/// zoom/pan math needs. Built live by the widget (authoritative) and from the
/// last hover-reported sizes app-side (for centre-anchored button/key zoom).
#[derive(Debug, Clone, Copy)]
pub struct LoupeGeometry {
    pub viewport: Size,
    pub native: Size,
}

impl LoupeGeometry {
    /// The image's fit-to-window size (zoom == 1.0).
    pub fn fitted(&self) -> Size {
        ContentFit::Contain.fit(self.native, self.viewport)
    }

    /// The drawn size at `zoom`.
    pub fn scaled(&self, zoom: f32) -> Size {
        let f = self.fitted();
        Size::new(f.width * zoom, f.height * zoom)
    }

    /// Zoom factor at which one image pixel maps to one screen pixel.
    pub fn actual_factor(&self) -> f32 {
        let f = self.fitted();
        if f.width <= 0.0 || f.height <= 0.0 {
            return 2.0;
        }
        (self.native.width / f.width).max(self.native.height / f.height)
    }

    /// Centre of the viewport, in viewport coordinates.
    pub fn center(&self) -> Point {
        Point::new(self.viewport.width / 2.0, self.viewport.height / 2.0)
    }

    /// Clamp a pan offset so the image can't be dragged past its own edges.
    pub fn clamp_offset(&self, offset: Vector, zoom: f32) -> Vector {
        let scaled = self.scaled(zoom);
        let hidden_w = ((scaled.width - self.viewport.width) / 2.0).max(0.0);
        let hidden_h = ((scaled.height - self.viewport.height) / 2.0).max(0.0);
        Vector::new(
            offset.x.clamp(-hidden_w, hidden_w),
            offset.y.clamp(-hidden_h, hidden_h),
        )
    }

    /// New pan offset that keeps `anchor` (viewport coordinates) fixed on the
    /// image while zoom moves `from` → `to`. The one anchoring primitive.
    fn zoom_around(&self, anchor: Point, from: f32, to: f32, offset: Vector) -> Vector {
        let factor = to / from - 1.0;
        let anchor_to_center = anchor - self.center();
        let adjustment = anchor_to_center * factor + offset * factor;
        let scaled = self.scaled(to);
        let next = Vector::new(
            if scaled.width > self.viewport.width { offset.x + adjustment.x } else { 0.0 },
            if scaled.height > self.viewport.height { offset.y + adjustment.y } else { 0.0 },
        );
        self.clamp_offset(next, to)
    }

    /// The whole reducer. Pure: `(current, intent) -> next`.
    pub fn apply(&self, cur: LoupeZoom, intent: LoupeIntent) -> LoupeZoom {
        match intent {
            LoupeIntent::ZoomAround { anchor, factor } => {
                let to = (cur.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
                self.settle(cur, to, anchor)
            }
            LoupeIntent::ZoomTo { level, anchor } => {
                let to = match level {
                    ZoomLevel::Fit => ZOOM_MIN,
                    ZoomLevel::Actual => self.actual_factor().clamp(ZOOM_MIN, ZOOM_MAX),
                };
                self.settle(cur, to, anchor)
            }
            LoupeIntent::PanTo(offset) => LoupeZoom {
                zoom: cur.zoom,
                offset: self.clamp_offset(offset, cur.zoom),
            },
            LoupeIntent::Reset => LoupeZoom { zoom: ZOOM_MIN, offset: Vector::ZERO },
        }
    }

    fn settle(&self, cur: LoupeZoom, to: f32, anchor: Point) -> LoupeZoom {
        if to <= ZOOM_MIN {
            return LoupeZoom { zoom: ZOOM_MIN, offset: Vector::ZERO };
        }
        if to == cur.zoom {
            return cur;
        }
        let offset = self.zoom_around(anchor, cur.zoom, to, cur.offset);
        LoupeZoom { zoom: to, offset }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo() -> LoupeGeometry {
        // 1000×500 image in an 800×600 viewport → fits to 800×400 (zoom 1.0),
        // so actual_factor = 1000/800 = 1.25.
        LoupeGeometry { viewport: Size::new(800.0, 600.0), native: Size::new(1000.0, 500.0) }
    }

    fn fit() -> LoupeZoom {
        LoupeZoom { zoom: ZOOM_MIN, offset: Vector::ZERO }
    }

    mod apply {
        use super::*;

        #[test]
        fn zoom_around_clamps_to_bounds() {
            let z = geo().apply(fit(), LoupeIntent::ZoomAround { anchor: geo().center(), factor: 100.0 });
            assert_eq!(z.zoom, ZOOM_MAX);
        }

        #[test]
        fn zoom_to_actual_uses_native_over_fitted() {
            let z = geo().apply(fit(), LoupeIntent::ZoomTo { level: ZoomLevel::Actual, anchor: geo().center() });
            assert!((z.zoom - 1.25).abs() < 1e-4, "got {}", z.zoom);
        }

        #[test]
        fn zoom_to_fit_zeroes_pan() {
            let zoomed = LoupeZoom { zoom: 4.0, offset: Vector::new(50.0, 20.0) };
            let z = geo().apply(zoomed, LoupeIntent::ZoomTo { level: ZoomLevel::Fit, anchor: geo().center() });
            assert_eq!(z, fit());
        }

        #[test]
        fn reset_returns_fit() {
            let zoomed = LoupeZoom { zoom: 3.0, offset: Vector::new(10.0, 10.0) };
            assert_eq!(geo().apply(zoomed, LoupeIntent::Reset), fit());
        }

        #[test]
        fn centre_anchored_zoom_keeps_pan_centred() {
            // Zooming around the exact centre introduces no pan.
            let z = geo().apply(fit(), LoupeIntent::ZoomAround { anchor: geo().center(), factor: 2.0 });
            assert_eq!(z.zoom, 2.0);
            assert_eq!(z.offset, Vector::ZERO);
        }

        #[test]
        fn pan_to_is_clamped_to_hidden_overflow() {
            // At zoom 2.0 the image is 1600×800; viewport 800×600 hides 400 / 100
            // px → ±400 / ±100 of pan.
            let zoomed = LoupeZoom { zoom: 2.0, offset: Vector::ZERO };
            let z = geo().apply(zoomed, LoupeIntent::PanTo(Vector::new(9999.0, 9999.0)));
            assert_eq!(z.offset, Vector::new(400.0, 100.0));
        }
    }

    mod click_action_fn {
        use super::*;

        #[test]
        fn fit_zooms_in_zoomed_zooms_out() {
            assert_eq!(click_action(ZOOM_MIN), ClickAction::ZoomIn);
            assert_eq!(click_action(3.0), ClickAction::ZoomOut);
        }
    }
}
