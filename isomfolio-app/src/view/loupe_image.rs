//! Zoomable / pannable loupe image.
//!
//! Unlike iced's built-in `image::Viewer`, the zoom/pan state lives in the
//! application (`LoupeState`), not inside the widget. That lets the same state
//! be driven by *both* trackpad/scroll gestures (handled here, emitted as
//! `on_change`) and the on-screen zoom buttons (handled in `update`). The
//! widget is otherwise a thin port of `Viewer`'s geometry + drawing.

use iced::advanced::image::{self, FilterMethod, Image};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{tree, Tree};
use iced::advanced::{mouse, Clipboard, Shell, Widget};
use iced::{
    border, ContentFit, Element, Event, Length, Point, Radians, Rectangle, Size, Vector,
};

pub struct LoupeImage<'a, Message, Handle> {
    handle: Handle,
    scale: f32,
    offset: Vector,
    min_scale: f32,
    max_scale: f32,
    scale_step: f32,
    on_change: Box<dyn Fn(f32, Vector) -> Message + 'a>,
    /// Reports `(viewport_size, native_image_size)` on interaction, so the app
    /// can compute the exact "1:1" (actual-pixel) zoom factor.
    on_geometry: Box<dyn Fn(Size, Size) -> Message + 'a>,
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
            min_scale: 1.0,
            max_scale: 8.0,
            scale_step: 0.10,
            on_change: Box::new(on_change),
            on_geometry: Box::new(on_geometry),
        }
    }
}

#[derive(Default)]
struct State {
    /// Cursor position where a pan-drag began, plus the offset at that moment.
    grabbed: Option<(Point, Vector)>,
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
        let fit = fitted_size(renderer, &self.handle, bounds.size());

        // Keep the app's view of (viewport, native) fresh while the cursor is
        // over the image, so the "1:1" button can compute actual-pixel zoom.
        if matches!(event, Event::Mouse(_)) && cursor.is_over(bounds) {
            let raw = renderer.measure_image(&self.handle).unwrap_or_default();
            if raw.width > 0 && bounds.width > 0.0 {
                let native = Size::new(raw.width as f32, raw.height as f32);
                shell.publish((self.on_geometry)(bounds.size(), native));
            }
        }

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let Some(cursor_position) = cursor.position_over(bounds) else {
                    return;
                };
                let y = match *delta {
                    mouse::ScrollDelta::Lines { y, .. }
                    | mouse::ScrollDelta::Pixels { y, .. } => y,
                };
                if y == 0.0 {
                    return;
                }
                let previous = self.scale;
                let next = if y > 0.0 {
                    self.scale * (1.0 + self.scale_step)
                } else {
                    self.scale / (1.0 + self.scale_step)
                }
                .clamp(self.min_scale, self.max_scale);
                if next == previous {
                    return;
                }

                // Keep the point under the cursor stationary while zooming.
                let factor = next / previous - 1.0;
                let cursor_to_center = cursor_position - bounds.center();
                let adjustment = cursor_to_center * factor + self.offset * factor;
                let scaled = Size::new(fit.width * next, fit.height * next);
                let new_offset = clamp_offset(
                    Vector::new(
                        if scaled.width > bounds.width { self.offset.x + adjustment.x } else { 0.0 },
                        if scaled.height > bounds.height { self.offset.y + adjustment.y } else { 0.0 },
                    ),
                    bounds.size(),
                    scaled,
                );
                shell.publish((self.on_change)(next, new_offset));
                shell.capture_event();
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(cursor_position) = cursor.position_over(bounds) else {
                    return;
                };
                if self.scale <= self.min_scale {
                    return;
                }
                tree.state.downcast_mut::<State>().grabbed = Some((cursor_position, self.offset));
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                tree.state.downcast_mut::<State>().grabbed = None;
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let state = tree.state.downcast_mut::<State>();
                if let Some((origin, starting)) = state.grabbed {
                    let scaled = Size::new(fit.width * self.scale, fit.height * self.scale);
                    let delta = *position - origin;
                    let new_offset =
                        clamp_offset(starting - delta, bounds.size(), scaled);
                    shell.publish((self.on_change)(self.scale, new_offset));
                    shell.capture_event();
                    shell.request_redraw();
                }
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
        if tree.state.downcast_ref::<State>().grabbed.is_some() {
            mouse::Interaction::Grabbing
        } else if self.scale > self.min_scale && cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::None
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
        let fit = fitted_size(renderer, &self.handle, bounds.size());
        let scaled = Size::new(fit.width * self.scale, fit.height * self.scale);

        let offset = clamp_offset(self.offset, bounds.size(), scaled);
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

fn fitted_size<Renderer, Handle>(renderer: &Renderer, handle: &Handle, bounds: Size) -> Size
where
    Renderer: image::Renderer<Handle = Handle>,
{
    let raw = renderer.measure_image(handle).unwrap_or_default();
    let image_size = Size::new(raw.width as f32, raw.height as f32);
    ContentFit::Contain.fit(image_size, bounds)
}

/// Clamp the pan offset so the image can't be dragged past its own edges.
fn clamp_offset(offset: Vector, bounds: Size, scaled: Size) -> Vector {
    let hidden_w = ((scaled.width - bounds.width) / 2.0).max(0.0);
    let hidden_h = ((scaled.height - bounds.height) / 2.0).max(0.0);
    Vector::new(
        offset.x.clamp(-hidden_w, hidden_w),
        offset.y.clamp(-hidden_h, hidden_h),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    mod clamp_offset {
        use super::*;

        #[test]
        fn image_smaller_or_equal_to_bounds_is_pinned_centre() {
            let got = clamp_offset(
                Vector::new(50.0, 50.0),
                Size::new(800.0, 600.0),
                Size::new(800.0, 600.0),
            );
            assert_eq!(got, Vector::new(0.0, 0.0));
        }

        #[test]
        fn offset_clamped_to_half_the_hidden_overflow() {
            // 1000-wide image in an 800 viewport hides 200px → ±100 of pan.
            let got = clamp_offset(
                Vector::new(500.0, 0.0),
                Size::new(800.0, 600.0),
                Size::new(1000.0, 600.0),
            );
            assert_eq!(got.x, 100.0);
        }

        #[test]
        fn within_range_offset_is_unchanged() {
            let got = clamp_offset(
                Vector::new(-40.0, 30.0),
                Size::new(800.0, 600.0),
                Size::new(1000.0, 800.0),
            );
            assert_eq!(got, Vector::new(-40.0, 30.0));
        }
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
