//! The menu bar and its dropdown menus.
//!
//! iced has no menu widget, so the dropdown is drawn as a layer over the whole
//! window. Every title has the same width, which tells us where to put it.

use iced::widget::{
    Svg, button, column, container, mouse_area, opaque, row, rule, space, stack, svg, text,
};
use iced::{Center, Element, Fill, Length};

use crate::style;
use crate::theme::Colors;

pub const BAR_HEIGHT: f32 = 30.0;
const LOGO_WIDTH: f32 = 40.0;
const TITLE_WIDTH: f32 = 52.0;
const DROPDOWN_WIDTH: f32 = 280.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    File,
    Edit,
    View,
    Help,
}

impl Menu {
    pub const ALL: [Menu; 4] = [Menu::File, Menu::Edit, Menu::View, Menu::Help];

    fn label(self) -> &'static str {
        match self {
            Menu::File => "File",
            Menu::Edit => "Edit",
            Menu::View => "View",
            Menu::Help => "Help",
        }
    }

    /// How far from the left edge of the window the menu's title starts.
    fn offset(self) -> f32 {
        let index = Menu::ALL.iter().position(|menu| *menu == self).unwrap_or(0);
        LOGO_WIDTH + TITLE_WIDTH * index as f32
    }
}

/// A line in a dropdown menu.
pub enum Entry<Message> {
    Item {
        label: String,
        shortcut: Option<&'static str>,
        /// The message sent when the item is clicked, or `None` if it is disabled.
        on_press: Option<Message>,
        /// Whether a check mark shows next to the item, for items that toggle.
        checked: bool,
    },
    /// A label for the items below it, which can't be clicked.
    Heading(String),
    Separator,
}

/// An item that sends `on_press` when clicked, or is disabled if `on_press` is `None`.
pub fn item<Message>(label: impl Into<String>, on_press: Option<Message>) -> Entry<Message> {
    Entry::Item {
        label: label.into(),
        shortcut: None,
        on_press,
        checked: false,
    }
}

impl<Message> Entry<Message> {
    pub fn shortcut(mut self, keys: &'static str) -> Self {
        if let Entry::Item { shortcut, .. } = &mut self {
            *shortcut = Some(keys);
        }
        self
    }

    pub fn checked(mut self, is_checked: bool) -> Self {
        if let Entry::Item { checked, .. } = &mut self {
            *checked = is_checked;
        }
        self
    }
}

/// The menu bar: the logo, then a title for each menu.
///
/// Clicking a title sends `on_open`; while a menu is open, moving over another
/// title sends `on_hover` so the user can slide between menus.
pub fn bar<'a, Message: Clone + 'a>(
    c: Colors,
    logo: &svg::Handle,
    open: Option<Menu>,
    on_open: impl Fn(Menu) -> Message,
    on_hover: impl Fn(Menu) -> Message,
) -> Element<'a, Message> {
    let titles = Menu::ALL.into_iter().map(|menu| {
        let title = button(text(menu.label()).size(13).width(Fill).align_x(Center))
            .width(TITLE_WIDTH)
            .padding([4, 0])
            .style(style::menu_title(c, open == Some(menu)))
            .on_press(on_open(menu));

        if open.is_some() {
            mouse_area(title).on_enter(on_hover(menu)).into()
        } else {
            title.into()
        }
    });

    let logo = container(Svg::new(logo.clone()).width(22).height(22))
        .width(LOGO_WIDTH)
        .center_x(LOGO_WIDTH);

    container(row![logo, row(titles)].align_y(Center).height(BAR_HEIGHT))
        .width(Fill)
        .style(style::menu_bar(c))
        .into()
}

/// The open dropdown, as a layer over the whole window.
///
/// Clicking anywhere outside the dropdown and the menu bar sends `on_close`.
pub fn dropdown<'a, Message: Clone + 'a>(
    c: Colors,
    menu: Menu,
    entries: Vec<Entry<Message>>,
    on_close: Message,
) -> Element<'a, Message> {
    let lines = entries.into_iter().map(|entry| -> Element<'a, Message> {
        match entry {
            Entry::Item {
                label,
                shortcut,
                on_press,
                checked,
            } => button(
                row![
                    text(if checked { "✓" } else { "" }).size(13).width(18),
                    text(label).size(13).width(Fill),
                    text(shortcut.unwrap_or_default())
                        .size(12)
                        .color(c.text_muted),
                ]
                .align_y(Center),
            )
            .width(Fill)
            .padding([4, 8])
            .style(style::menu_item(c))
            .on_press_maybe(on_press)
            .into(),
            Entry::Heading(label) => container(text(label).size(11).color(c.text_muted))
                .padding([4, 8])
                .into(),
            Entry::Separator => container(rule::horizontal(1).style(style::separator(c)))
                .padding([4, 6])
                .into(),
        }
    });

    let dropdown = container(column(lines))
        .width(DROPDOWN_WIDTH)
        .padding(4)
        .style(style::dropdown(c));

    // Below the menu bar, a transparent layer catches clicks outside the dropdown.
    let catcher = column![
        space().height(BAR_HEIGHT),
        mouse_area(space().width(Fill).height(Fill)).on_press(on_close),
    ];

    let placed = column![
        space().height(BAR_HEIGHT),
        row![space().width(menu.offset()), opaque(dropdown)],
    ]
    .width(Length::Shrink);

    stack![catcher, placed].into()
}
