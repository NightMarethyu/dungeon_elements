//! Colors and widget styles, loosely modeled on VS Code's dark theme.

use iced::theme::Palette;
use iced::widget::{button, container, text_editor};
use iced::{Background, Border, Color, Theme};

pub const EDITOR: Color = Color::from_rgb8(0x1e, 0x1e, 0x1e);
pub const SIDEBAR: Color = Color::from_rgb8(0x25, 0x25, 0x26);
pub const TITLE_BAR: Color = Color::from_rgb8(0x32, 0x32, 0x33);
pub const INACTIVE_TAB: Color = Color::from_rgb8(0x2d, 0x2d, 0x2d);
pub const HOVER: Color = Color::from_rgb8(0x2a, 0x2d, 0x2e);
pub const SELECTED: Color = Color::from_rgb8(0x37, 0x37, 0x3d);
pub const BORDER: Color = Color::from_rgb8(0x3c, 0x3c, 0x3c);
pub const ACCENT: Color = Color::from_rgb8(0x00, 0x7a, 0xcc);
pub const TEXT: Color = Color::from_rgb8(0xcc, 0xcc, 0xcc);
pub const MUTED: Color = Color::from_rgb8(0x96, 0x96, 0x96);

pub fn theme() -> Theme {
    Theme::custom(
        "Dungeon Elements Dark",
        Palette {
            background: EDITOR,
            text: TEXT,
            primary: ACCENT,
            success: Color::from_rgb8(0x89, 0xd1, 0x85),
            warning: Color::from_rgb8(0xcc, 0xa7, 0x00),
            danger: Color::from_rgb8(0xf4, 0x87, 0x71),
        },
    )
}

pub fn panel(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style::default().background(color)
}

pub fn status_bar(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(ACCENT)
        .color(Color::WHITE)
}

pub fn dialog(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(SIDEBAR)
        .border(Border::default().color(BORDER).width(1).rounded(6))
}

pub fn backdrop(_theme: &Theme) -> container::Style {
    container::Style::default().background(Color::from_rgba8(0, 0, 0, 0.6))
}

/// A tab in the tab bar. The active tab blends into the editor below it.
pub fn tab(active: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let style = container::Style::default().color(if active { Color::WHITE } else { MUTED });

        if active {
            style.background(EDITOR)
        } else {
            style.background(INACTIVE_TAB)
        }
    }
}

/// A flat button that only shows a background on hover, like VS Code's list rows.
pub fn flat(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let background = match status {
            _ if selected => Some(SELECTED),
            button::Status::Hovered | button::Status::Pressed => Some(HOVER),
            _ => None,
        };

        button::Style {
            background: background.map(Background::Color),
            text_color: TEXT,
            ..button::Style::default()
        }
    }
}

/// A flat button for text on a tab, which stays transparent so the tab shows through.
pub fn tab_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        text_color: match status {
            button::Status::Hovered => Color::WHITE,
            _ if active => Color::WHITE,
            _ => MUTED,
        },
        ..button::Style::default()
    }
}

/// The default primary button, with white text like VS Code's buttons.
pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        text_color: Color::WHITE,
        ..button::primary(theme, status)
    }
}

/// A gray button for secondary actions, like VS Code's.
pub fn secondary(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgb8(0x45, 0x49, 0x4e),
        _ => Color::from_rgb8(0x3a, 0x3d, 0x41),
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: Color::WHITE,
        border: Border::default().rounded(2),
        ..button::Style::default()
    }
}

pub fn editor(theme: &Theme, status: text_editor::Status) -> text_editor::Style {
    text_editor::Style {
        background: Background::Color(EDITOR),
        border: Border::default(),
        ..text_editor::default(theme, status)
    }
}
