//! Widget styles. Every color comes from the active theme's [`Colors`].

use iced::border::Radius;
use iced::widget::{button, checkbox, container, rule, text_editor, text_input};
use iced::{Background, Border, Color, Theme};

use crate::theme::Colors;

pub fn panel(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style::default().background(color)
}

pub fn menu_bar(c: Colors) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        container::Style::default()
            .background(c.menu_bar)
            .color(c.text)
            .border(Border::default().color(c.border).width(0))
    }
}

pub fn status_bar(c: Colors) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        container::Style::default()
            .background(c.status_bar)
            .color(c.status_text)
            .border(Border::default().color(c.border).width(top_edge(c)))
    }
}

/// The width of the line between the status bar and the window above it.
///
/// Themes with thick borders (like High Contrast) outline the status bar; others don't.
fn top_edge(c: Colors) -> f32 {
    if c.border_width > 1.0 {
        c.border_width
    } else {
        0.0
    }
}

pub fn dialog(c: Colors) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        container::Style::default()
            .background(c.sidebar)
            .color(c.text)
            .border(
                Border::default()
                    .color(c.border)
                    .width(c.border_width)
                    .rounded(c.corner_radius),
            )
    }
}

/// An open dropdown menu.
pub fn dropdown(c: Colors) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        container::Style::default()
            .background(c.menu)
            .color(c.text)
            .border(
                Border::default()
                    .color(c.border)
                    .width(c.border_width)
                    .rounded(c.corner_radius.min(6.0)),
            )
    }
}

pub fn backdrop(_theme: &Theme) -> container::Style {
    container::Style::default().background(Color::from_rgba8(0, 0, 0, 0.6))
}

pub fn separator(c: Colors) -> impl Fn(&Theme) -> rule::Style {
    move |_| rule::Style {
        color: c.border,
        radius: Radius::default(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}

/// A tab in the tab bar. The active tab blends into the editor below it.
pub fn tab(c: Colors, active: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let radius = c.corner_radius.min(10.0);
        container::Style::default()
            .color(if active { c.text } else { c.text_muted })
            .background(if active { c.tab_active } else { c.tab_inactive })
            .border(Border {
                color: c.border,
                width: if c.border_width > 1.0 {
                    c.border_width
                } else {
                    0.0
                },
                radius: Radius::default().top(radius),
            })
    }
}

/// A flat button that only shows a background on hover, like VS Code's list rows.
pub fn flat(c: Colors, selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let background = match status {
            _ if selected => Some(c.selection),
            button::Status::Hovered | button::Status::Pressed => Some(c.hover),
            _ => None,
        };

        button::Style {
            background: background.map(Background::Color),
            text_color: match status {
                button::Status::Disabled => c.text_muted,
                _ => c.text,
            },
            ..button::Style::default()
        }
    }
}

/// A title in the menu bar, highlighted while its menu is open.
pub fn menu_title(c: Colors, open: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let background = match status {
            _ if open => Some(c.selection),
            button::Status::Hovered | button::Status::Pressed => Some(c.hover),
            _ => None,
        };

        button::Style {
            background: background.map(Background::Color),
            text_color: c.text,
            border: Border::default().rounded(c.corner_radius.min(4.0)),
            ..button::Style::default()
        }
    }
}

/// An item in an open dropdown menu.
pub fn menu_item(c: Colors) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let (background, text_color) = match status {
            button::Status::Hovered | button::Status::Pressed => (Some(c.accent), c.accent_text),
            button::Status::Disabled => (None, c.text_muted),
            button::Status::Active => (None, c.text),
        };

        button::Style {
            background: background.map(Background::Color),
            text_color,
            border: Border::default().rounded(c.corner_radius.min(4.0)),
            ..button::Style::default()
        }
    }
}

/// A flat button for text on a tab, which stays transparent so the tab shows through.
pub fn tab_button(c: Colors, active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        text_color: match status {
            button::Status::Hovered => c.text,
            _ if active => c.text,
            _ => c.text_muted,
        },
        ..button::Style::default()
    }
}

pub fn primary(c: Colors) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let background = match status {
            button::Status::Hovered | button::Status::Pressed => mix(c.accent, c.accent_text, 0.15),
            button::Status::Disabled => mix(c.accent, c.background, 0.5),
            button::Status::Active => c.accent,
        };

        button::Style {
            background: Some(Background::Color(background)),
            text_color: c.accent_text,
            border: Border::default()
                .color(c.border)
                .width(edge(c))
                .rounded(c.corner_radius.min(8.0)),
            ..button::Style::default()
        }
    }
}

/// A button for secondary actions.
pub fn secondary(c: Colors) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let background = match status {
            button::Status::Hovered | button::Status::Pressed => c.hover,
            _ => c.selection,
        };

        button::Style {
            background: Some(Background::Color(background)),
            text_color: c.text,
            border: Border::default()
                .color(c.border)
                .width(edge(c))
                .rounded(c.corner_radius.min(8.0)),
            ..button::Style::default()
        }
    }
}

/// The outline width for buttons and fields: only themes with thick borders draw one.
fn edge(c: Colors) -> f32 {
    if c.border_width > 1.0 {
        c.border_width
    } else {
        0.0
    }
}

pub fn input(c: Colors) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_, status| {
        let border_color = match status {
            text_input::Status::Focused { .. } => c.accent,
            _ => c.border,
        };

        text_input::Style {
            background: Background::Color(c.field_background),
            border: Border::default()
                .color(border_color)
                .width(c.border_width.max(1.0))
                .rounded(c.corner_radius.min(8.0)),
            icon: c.text_muted,
            placeholder: c.text_muted,
            value: c.text,
            selection: with_alpha(c.accent, 0.35),
        }
    }
}

pub fn check(c: Colors) -> impl Fn(&Theme, checkbox::Status) -> checkbox::Style {
    move |_, status| {
        let checked = match status {
            checkbox::Status::Active { is_checked }
            | checkbox::Status::Hovered { is_checked }
            | checkbox::Status::Disabled { is_checked } => is_checked,
        };

        checkbox::Style {
            background: Background::Color(if checked {
                c.accent
            } else {
                c.field_background
            }),
            icon_color: c.accent_text,
            border: Border::default()
                .color(if checked { c.accent } else { c.border })
                .width(c.border_width.max(1.0))
                .rounded(c.corner_radius.min(4.0)),
            text_color: Some(c.text),
        }
    }
}

pub fn editor(c: Colors) -> impl Fn(&Theme, text_editor::Status) -> text_editor::Style {
    move |_, _| text_editor::Style {
        background: Background::Color(c.background),
        border: Border::default(),
        placeholder: c.text_muted,
        value: c.text,
        selection: with_alpha(c.accent, 0.35),
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

/// Blends `amount` of `b` into `a`.
fn mix(a: Color, b: Color, amount: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * amount,
        g: a.g + (b.g - a.g) * amount,
        b: a.b + (b.b - a.b) * amount,
        a: a.a,
    }
}
