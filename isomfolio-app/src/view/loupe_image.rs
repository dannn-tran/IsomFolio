//! Zoomable / pannable loupe image.
//!
//! Unlike iced's built-in `image::Viewer`, the zoom/pan state lives in the
//! application (`LoupeState`), not inside the widget. This widget only
//! *classifies* raw input into a [`LoupeIntent`] and applies it through the
//! shared [`LoupeGeometry`] reducer (`crate::app::loupe`), emitting the result
//! via `on_change`. The zoom buttons and keyboard shortcuts feed the very same
//! reducer in `update`, so there is one anchoring formula, not two.

use iced::advanced::image::{self, FilterMethod, Image};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{tree, Tree};
use iced::advanced::{mouse, Clipboard, Shell, Widget};
use iced::{border, Element, Event, Length, Point, Radians, Rectangle, Size, Vector};

use crate::app::loupe::{
    click_action, ClickAction, LoupeGeometry, LoupeIntent, LoupeZoom, ZoomLevel, ZOOM_MIN,
    ZOOM_STEP,
};

/// Pixels the cursor must travel after press before it counts as a drag (pan)
/// rather than a click (zoom toggle).
const CLICK_SLOP: f32 = 3.0;

pub struct LoupeImage<'a, Message, Handle> {
    handle: Handle,
    scale: f32,
    offset: Vector,
    on_change: Box<dyn Fn(f32, Vector) -> Message + 'a>,
    /// Reports `(viewport_size, native_image_size)` on interaction, so the app
    /// can compute the exact "1:1" (actual-pixel) zoom factor.
    on_geometry: Box<dyn Fn(Size, Size) -> Message + 'a>,
    /// Optional click override. When set, a non-drag left-click publishes this
    /// message instead of toggling 1:1↔fit — used by Survey panes, where a click
    /// *focuses* the frame (zoom stays on scroll). Scroll-zoom and drag-pan are
    /// unaffected either way.
    on_click: Option<Box<dyn Fn() -> Message + 'a>>,
}

impl<'a, Message, Handle> LoupeImage<'a, Message, Handle> {
    pub fn new(
        handle: Handle,
        scale: f32,
        offset: Vector,
        on_change: impl Fn(f32, Vector) -> Message + 'a,
        on_geometry: impl Fn(Size, Size) -> Message + 'a,
    ) -> Self {
        Self {
            handle,
            scale,
            offset,
            on_change: Box::new(on_change),
            on_geometry: Box::new(on_geometry),
            on_click: None,
        }
    }

    /// Route a non-drag left-click to `f()` (focus the frame) instead of the
    /// default 1:1↔fit zoom-toggle.
    pub fn on_click(mut self, f: impl Fn() -> Message + 'a) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

/// A left-button press in progress. `moved` flips once the cursor passes
/// `CLICK_SLOP`, distinguishing a pan-drag from a click.
struct Press {
    /// Absolute cursor position at press, and the pan offset at that moment.
    origin: Point,
    start_offset: Vector,
    moved: bool,
}

#[derive(Default)]
struct State {
    press: Option<Press>,
}

impl<'a, Message, Handle> LoupeImage<'a, Message, Handle> {
    /// Live geometry from the current layout and the image's native size. The
    /// native size is `0×0` until the renderer can measure the image — callers
    /// must still draw (a zero-size draw is what triggers the upload that makes
    /// the *next* measurement succeed), so this never refuses to produce a value.
    fn geometry<Renderer>(&self, renderer: &Renderer, bounds: Rectangle) -> LoupeGeometry
    where
        Renderer: image::Renderer<Handle = Handle>,
    {
        let raw = renderer.measure_image(&self.handle).unwrap_or_default();
        LoupeGeometry {
            viewport: bounds.size(),
            native: Size::new(raw.width as f32, raw.height as f32),
        }
    }

    /// Whether the image has been measured yet (native size known).
    fn measured(&self, geo: &LoupeGeometry) -> bool {
        geo.native.width > 0.0 && geo.viewport.width > 0.0
    }

    /// Reduce `intent` against the current zoom/pan and publish the result.
    fn emit(&self, geo: &LoupeGeometry, shell: &mut Shell<'_, Message>, intent: LoupeIntent) {
        let cur = LoupeZoom { zoom: self.scale, offset: self.offset };
        let next = geo.apply(cur, intent);
        if next != cur {
            shell.publish((self.on_change)(next.zoom, next.offset));
            shell.request_redraw();
        }
        shell.capture_event();
    }
}

impl<'a, Message, Theme, Renderer, Handle> Widget<Message, Theme, Renderer>
    for LoupeImage<'a, Message, Handle>
where
    Renderer: image::Renderer<Handle = Handle>,
    Handle: Clone,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size { width: Length::Fill, height: Length::Fill }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let geo = self.geometry(renderer, bounds);

        // Keep the app's view of (viewport, native) fresh while the cursor is
        // over the image, so the "1:1" button can compute actual-pixel zoom.
        if matches!(event, Event::Mouse(_)) && cursor.is_over(bounds) && self.measured(&geo) {
            shell.publish((self.on_geometry)(geo.viewport, geo.native));
        }

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let Some(pos) = cursor.position_over(bounds) else { return };
                let y = match *delta {
                    mouse::ScrollDelta::Lines { y, .. }
                    | mouse::ScrollDelta::Pixels { y, .. } => y,
                };
                if y == 0.0 {
                    return;
                }
                let factor = if y > 0.0 { 1.0 + ZOOM_STEP } else { 1.0 / (1.0 + ZOOM_STEP) };
                let anchor = Point::new(pos.x - bounds.x, pos.y - bounds.y);
                self.emit(&geo, shell, LoupeIntent::ZoomAround { anchor, factor });
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(pos) = cursor.position_over(bounds) else { return };
                tree.state.downcast_mut::<State>().press =
                    Some(Press { origin: pos, start_offset: self.offset, moved: false });
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let state = tree.state.downcast_mut::<State>();
                let Some(press) = state.press.as_mut() else { return };
                let delta = *position - press.origin;
                if !press.moved && delta.x.hypot(delta.y) > CLICK_SLOP {
                    press.moved = true;
                }
                // Only a zoomed-in image can pan; at fit a drag does nothing.
                let pan = (press.moved && self.scale > ZOOM_MIN)
                    .then(|| press.start_offset - delta);
                if let Some(offset) = pan {
                    self.emit(&geo, shell, LoupeIntent::PanTo(offset));
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let Some(press) = tree.state.downcast_mut::<State>().press.take() else { return };
                if press.moved {
                    return; // It was a pan; nothing to do on release.
                }
                // A click with an override (Survey pane): focus this frame; zoom
                // stays on scroll. The synced zoom must not jump on a select.
                if let Some(f) = &self.on_click {
                    shell.publish(f());
                    shell.capture_event();
                    return;
                }
                // Otherwise toggle 1:1 (anchored at the click) ↔ fit. This is the
                // behaviour the magnifier cursor has always advertised.
                let anchor = Point::new(press.origin.x - bounds.x, press.origin.y - bounds.y);
                let intent = match click_action(self.scale) {
                    ClickAction::ZoomIn => LoupeIntent::ZoomTo { level: ZoomLevel::Actual, anchor },
                    ClickAction::ZoomOut => LoupeIntent::ZoomTo { level: ZoomLevel::Fit, anchor },
                };
                self.emit(&geo, shell, intent);
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let dragging = tree
            .state
            .downcast_ref::<State>()
            .press
            .as_ref()
            .is_some_and(|p| p.moved);
        if dragging && self.scale > ZOOM_MIN {
            mouse::Interaction::Grabbing
        } else if !cursor.is_over(bounds) {
            mouse::Interaction::None
        } else if self.scale > ZOOM_MIN {
            // Zoomed in: drag to pan (a click focuses or zooms-to-fit).
            mouse::Interaction::Grab
        } else if self.on_click.is_some() {
            // Survey pane at fit: a click selects (focuses) this frame; zoom is
            // on scroll. Signal "clickable", not "magnifier".
            mouse::Interaction::Pointer
        } else {
            // At fit: a click (or scroll) zooms in — and now actually does.
            mouse::Interaction::ZoomIn
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        // Always draw, even before the first successful measurement: the draw
        // itself uploads the texture, which is what lets the *next* measurement
        // (and therefore zoom/pan) work. Bailing here would deadlock.
        let geo = self.geometry(renderer, bounds);
        let scaled = geo.scaled(self.scale);

        let offset = geo.clamp_offset(self.offset, self.scale);
        let centered = Vector::new(
            (bounds.width - scaled.width) / 2.0,
            (bounds.height - scaled.height) / 2.0,
        );
        let translation = centered - offset;

        let drawing_bounds = Rectangle::new(bounds.position(), scaled);
        renderer.with_layer(bounds, |renderer| {
            renderer.with_translation(translation, |renderer| {
                renderer.draw_image(
                    Image {
                        handle: self.handle.clone(),
                        border_radius: border::Radius::default(),
                        filter_method: FilterMethod::default(),
                        rotation: Radians(0.0),
                        opacity: 1.0,
                        snap: true,
                    },
                    drawing_bounds,
                    *viewport - translation,
                );
            });
        });
    }
}

impl<'a, Message, Theme, Renderer, Handle> From<LoupeImage<'a, Message, Handle>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: 'a + image::Renderer<Handle = Handle>,
    Message: 'a,
    Handle: Clone + 'a,
{
    fn from(w: LoupeImage<'a, Message, Handle>) -> Self {
        Element::new(w)
    }
}
