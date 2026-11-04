//! A color picker: a square to pick how strong and how bright a color is, a strip of every
//! hue, a box for a color code like `#57c08c`, and a row of ready-made swatches.
//!
//! It is drawn on iced's canvas and knows nothing about themes, so anything that needs a
//! color can use it: the theme editor now, and the fills and lines of maps later. Keep a
//! [`Picker`] in the state, show [`Picker::view`], and pass its messages to
//! [`Picker::update`], which says when the color changed.

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, gradient};
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Color, Element, Fill, Point, Rectangle, Renderer, Size, Theme};

use crate::style;
use crate::theme::{self, Colors};

/// A color as hue (0 to 360), saturation and value (0 to 1): how a person thinks of
/// picking one, and what the square and the strip show.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsv {
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
}

impl Hsv {
    pub fn from_color(color: Color) -> Self {
        let (r, g, b) = (color.r, color.g, color.b);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let hue = if delta <= f32::EPSILON {
            0.0
        } else if max == r {
            60.0 * ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        Hsv {
            hue,
            saturation: if max <= f32::EPSILON {
                0.0
            } else {
                delta / max
            },
            value: max,
        }
    }

    pub fn to_color(self) -> Color {
        let hue = self.hue.rem_euclid(360.0) / 60.0;
        let chroma = self.value * self.saturation;
        let x = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
        let (r, g, b) = match hue as u32 {
            0 => (chroma, x, 0.0),
            1 => (x, chroma, 0.0),
            2 => (0.0, chroma, x),
            3 => (0.0, x, chroma),
            4 => (x, 0.0, chroma),
            _ => (chroma, 0.0, x),
        };
        let m = self.value - chroma;
        Color::from_rgb(r + m, g + m, b + m)
    }
}

/// What a picker can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// A spot in the square: how strong (left to right) and how bright (bottom to top).
    Square(f32, f32),
    /// A spot on the hue strip, from 0 to 1.
    Hue(f32),
    /// The color code box changed.
    Code(String),
    /// A swatch was clicked.
    Swatch(Color),
}

/// A picker's state: the color, kept as [`Hsv`] so dragging to grey and back keeps the
/// hue, and what is typed in the color code box.
#[derive(Debug, Clone)]
pub struct Picker {
    hsv: Hsv,
    code: String,
}

impl Picker {
    pub fn new(color: Color) -> Self {
        Picker {
            hsv: Hsv::from_color(color),
            code: theme::to_hex(color),
        }
    }

    pub fn color(&self) -> Color {
        self.hsv.to_color()
    }

    /// Acts on `message`, and gives the new color if it changed.
    pub fn update(&mut self, message: Message) -> Option<Color> {
        match message {
            Message::Square(saturation, value) => {
                self.hsv.saturation = saturation.clamp(0.0, 1.0);
                self.hsv.value = value.clamp(0.0, 1.0);
            }
            Message::Hue(at) => self.hsv.hue = at.clamp(0.0, 1.0) * 360.0,
            Message::Code(code) => {
                let typed = theme::parse_color(&code)
                    .or_else(|| theme::parse_color(&format!("#{}", code.trim())));
                self.code = code;
                let color = typed?;
                self.hsv = Hsv::from_color(color);
                return Some(self.color());
            }
            Message::Swatch(color) => self.hsv = Hsv::from_color(color),
        }
        self.code = theme::to_hex(self.color());
        Some(self.color())
    }

    /// The picker, about `width` wide, with `swatches` to pick from under it.
    pub fn view<'a>(&self, c: Colors, width: f32, swatches: &[Color]) -> Element<'a, Message> {
        let square = Canvas::new(Square { hsv: self.hsv })
            .width(width)
            .height(width * 0.6);
        let strip = Canvas::new(Strip { hue: self.hsv.hue })
            .width(width)
            .height(18);

        let preview = container(text(""))
            .width(36)
            .height(30)
            .style(swatch_style(self.color(), c));
        let code = text_input("#57c08c", &self.code)
            .on_input(Message::Code)
            .padding(6)
            .size(13)
            .width(110)
            .style(style::input(c));
        let bad_code = theme::parse_color(&self.code).is_none()
            && theme::parse_color(&format!("#{}", self.code.trim())).is_none();

        let mut swatch_row = row![].spacing(4);
        for &color in swatches {
            swatch_row = swatch_row.push(
                button(text(""))
                    .width(20)
                    .height(20)
                    .padding(0)
                    .style(move |_: &Theme, status| {
                        let mut look = button::Style {
                            background: Some(color.into()),
                            ..button::Style::default()
                        };
                        look.border = iced::Border {
                            color: if status == button::Status::Hovered {
                                c.text
                            } else {
                                c.border
                            },
                            width: 1.0,
                            radius: 3.0.into(),
                        };
                        look
                    })
                    .on_press(Message::Swatch(color)),
            );
        }

        column![
            square,
            strip,
            row![preview, code]
                .push(bad_code.then(|| {
                    text("A color code is # and six letters or digits, like #57c08c.")
                        .size(11)
                        .color(c.warning)
                        .width(Fill)
                }))
                .spacing(8)
                .align_y(iced::Center),
            swatch_row.wrap(),
        ]
        .spacing(8)
        .width(width)
        .into()
    }
}

/// A box filled with `color`, outlined so it shows on any background.
pub fn swatch_style(color: Color, c: Colors) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(color.into()),
        border: iced::Border {
            color: c.border,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    }
}

/// Whether the mouse button is down on a canvas, so dragging keeps picking even past
/// its edges.
#[derive(Default)]
struct Drag(bool);

/// Where `cursor` is across `bounds`, from 0 to 1 each way, kept inside.
fn spot(cursor: mouse::Cursor, bounds: Rectangle) -> Option<(f32, f32)> {
    let at = cursor.position()?;
    Some((
        ((at.x - bounds.x) / bounds.width).clamp(0.0, 1.0),
        ((at.y - bounds.y) / bounds.height).clamp(0.0, 1.0),
    ))
}

/// Starts, continues and ends a drag on a canvas, publishing `pick` of the spot.
fn drag<M>(
    state: &mut Drag,
    event: &canvas::Event,
    bounds: Rectangle,
    cursor: mouse::Cursor,
    pick: impl Fn((f32, f32)) -> M,
) -> Option<canvas::Action<M>> {
    match event {
        canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            if cursor.is_over(bounds) =>
        {
            state.0 = true;
            Some(canvas::Action::publish(pick(spot(cursor, bounds)?)).and_capture())
        }
        canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) if state.0 => {
            Some(canvas::Action::publish(pick(spot(cursor, bounds)?)).and_capture())
        }
        canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.0 => {
            state.0 = false;
            Some(canvas::Action::capture())
        }
        _ => None,
    }
}

/// A ring that shows on light and dark colors alike.
fn marker(frame: &mut Frame, at: Point) {
    let ring = Path::circle(at, 6.0);
    frame.stroke(
        &ring,
        Stroke::default().with_width(3.0).with_color(Color::BLACK),
    );
    frame.stroke(
        &ring,
        Stroke::default().with_width(1.5).with_color(Color::WHITE),
    );
}

/// The square: the hue, going white to the left and black to the bottom.
struct Square {
    hsv: Hsv,
}

impl canvas::Program<Message> for Square {
    type State = Drag;

    fn update(
        &self,
        state: &mut Drag,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        drag(state, event, bounds, cursor, |(x, y)| {
            Message::Square(x, 1.0 - y)
        })
    }

    fn draw(
        &self,
        _state: &Drag,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let size = bounds.size();
        let all = Path::rectangle(Point::ORIGIN, size);
        let pure = Hsv {
            hue: self.hsv.hue,
            saturation: 1.0,
            value: 1.0,
        }
        .to_color();
        frame.fill(
            &all,
            gradient::Linear::new(Point::ORIGIN, Point::new(size.width, 0.0))
                .add_stop(0.0, Color::WHITE)
                .add_stop(1.0, pure),
        );
        frame.fill(
            &all,
            gradient::Linear::new(Point::ORIGIN, Point::new(0.0, size.height))
                .add_stop(0.0, Color::TRANSPARENT)
                .add_stop(1.0, Color::BLACK),
        );
        marker(
            &mut frame,
            Point::new(
                self.hsv.saturation * size.width,
                (1.0 - self.hsv.value) * size.height,
            ),
        );
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Drag,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

/// The strip of every hue, red to red.
struct Strip {
    hue: f32,
}

impl canvas::Program<Message> for Strip {
    type State = Drag;

    fn update(
        &self,
        state: &mut Drag,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        drag(state, event, bounds, cursor, |(x, _)| Message::Hue(x))
    }

    fn draw(
        &self,
        _state: &Drag,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let size = bounds.size();
        let mut hues = gradient::Linear::new(Point::ORIGIN, Point::new(size.width, 0.0));
        for step in 0..=6 {
            let at = step as f32 / 6.0;
            let color = Hsv {
                hue: at * 360.0,
                saturation: 1.0,
                value: 1.0,
            }
            .to_color();
            hues = hues.add_stop(at, color);
        }
        frame.fill(&Path::rectangle(Point::ORIGIN, size), hues);
        let x = self.hue / 360.0 * size.width;
        let handle = Path::rectangle(Point::new(x - 2.0, 0.0), Size::new(4.0, size.height));
        frame.stroke(
            &handle,
            Stroke::default().with_width(2.0).with_color(Color::BLACK),
        );
        frame.fill(&handle, Color::WHITE);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Drag,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_go_to_hsv_and_back() {
        for hex in [
            "#57c08c", "#1e1e1e", "#ff0000", "#7a2e12", "#ffffff", "#000000",
        ] {
            let color = theme::parse_color(hex).unwrap();
            assert_eq!(theme::to_hex(Hsv::from_color(color).to_color()), hex);
        }
        let red = Hsv::from_color(Color::from_rgb(1.0, 0.0, 0.0));
        assert_eq!((red.hue, red.saturation, red.value), (0.0, 1.0, 1.0));
    }

    #[test]
    fn the_picker_follows_the_square_strip_and_code() {
        let mut picker = Picker::new(Color::from_rgb(1.0, 0.0, 0.0));
        // Grey keeps its hue, so moving back out of grey gives red again.
        picker.update(Message::Square(0.0, 0.5));
        assert_eq!(theme::to_hex(picker.color()), "#808080");
        picker.update(Message::Square(1.0, 1.0));
        assert_eq!(theme::to_hex(picker.color()), "#ff0000");
        picker.update(Message::Hue(1.0 / 3.0));
        assert_eq!(theme::to_hex(picker.color()), "#00ff00");

        // A code works with or without its #, and half-typed ones wait.
        assert_eq!(picker.update(Message::Code("#12".into())), None);
        assert_eq!(
            picker
                .update(Message::Code("57c08c".into()))
                .map(theme::to_hex),
            Some("#57c08c".into())
        );
    }
}
