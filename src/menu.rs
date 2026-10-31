//! The menu bar and its dropdown menus.
//!
//! iced has no menu widget, so the dropdown is drawn as a layer over the whole
//! window. Each title has a set width, which tells us where to put its dropdown.

use iced::widget::{
    Svg, button, column, container, mouse_area, opaque, row, rule, space, stack, svg, text, tooltip,
};
use iced::{Center, Element, Fill, Length};

use crate::style;
use crate::theme::Colors;

pub const BAR_HEIGHT: f32 = 30.0;
const LOGO_WIDTH: f32 = 40.0;
const TITLE_WIDTH: f32 = 52.0;
const DROPDOWN_WIDTH: f32 = 280.0;
const SUBMENU_WIDTH: f32 = 230.0;
/// The space around the lines of a dropdown, inside its border.
const PANEL_PADDING: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    File,
    Edit,
    View,
    Campaign,
    Tables,
    /// Kept for the map editor (Phase 4), and always greyed out until then.
    Maps,
    Help,
}

impl Menu {
    pub const ALL: [Menu; 7] = [
        Menu::File,
        Menu::Edit,
        Menu::View,
        Menu::Campaign,
        Menu::Tables,
        Menu::Maps,
        Menu::Help,
    ];

    fn label(self) -> &'static str {
        match self {
            Menu::File => "File",
            Menu::Edit => "Edit",
            Menu::View => "View",
            Menu::Campaign => "Campaign",
            Menu::Tables => "Tables",
            Menu::Maps => "Maps",
            Menu::Help => "Help",
        }
    }

    /// The menu that Alt and `letter` opens: the first letter of its title.
    pub fn from_letter(letter: &str) -> Option<Menu> {
        Menu::ALL
            .into_iter()
            .find(|menu| menu.label()[..1].eq_ignore_ascii_case(letter))
    }

    /// How wide the menu's title is: wide enough for its name.
    fn width(self) -> f32 {
        // About 7.5 pixels a letter at the titles' size, and some room either side.
        (self.label().len() as f32 * 7.5 + 18.0).max(TITLE_WIDTH)
    }

    /// How far from the left edge of the window the menu's title starts.
    fn offset(self) -> f32 {
        LOGO_WIDTH
            + Menu::ALL
                .into_iter()
                .take_while(|menu| *menu != self)
                .map(Menu::width)
                .sum::<f32>()
    }

    /// The menu `step` places to the right (or left, for a negative step), skipping
    /// menus that can't be opened, and going round at the ends.
    pub fn beside(self, step: i32, can_open: impl Fn(Menu) -> bool) -> Menu {
        let count = Menu::ALL.len() as i32;
        let mut index = Menu::ALL.iter().position(|menu| *menu == self).unwrap_or(0) as i32;
        for _ in 0..count {
            index = (index + step).rem_euclid(count);
            let menu = Menu::ALL[index as usize];
            if can_open(menu) {
                return menu;
            }
        }
        self
    }
}

/// Where the keyboard is in an open menu, and which submenu is open.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    /// The line the arrow keys are on, if they have been used.
    pub line: Option<usize>,
    /// The open submenu: the line it hangs from.
    pub submenu: Option<usize>,
    /// The line the arrow keys are on in the submenu.
    pub sub_line: Option<usize>,
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
    Separator,
    /// An item that opens another menu beside it.
    Submenu {
        label: String,
        entries: Vec<Entry<Message>>,
    },
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

/// An item that opens `entries` in a menu beside it.
pub fn submenu<Message>(label: impl Into<String>, entries: Vec<Entry<Message>>) -> Entry<Message> {
    Entry::Submenu {
        label: label.into(),
        entries,
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

    /// Whether the arrow keys stop on this line.
    fn can_pick(&self) -> bool {
        match self {
            Entry::Item { on_press, .. } => on_press.is_some(),
            Entry::Submenu { entries, .. } => !entries.is_empty(),
            Entry::Separator => false,
        }
    }

    fn height(&self) -> f32 {
        match self {
            Entry::Item { .. } | Entry::Submenu { .. } => 25.0,
            Entry::Separator => 9.0,
        }
    }
}

/// The line the arrow keys go to from `from`, a line down (or up), skipping lines that
/// can't be picked and going round at the ends.
pub fn step<Message>(entries: &[Entry<Message>], from: Option<usize>, down: bool) -> Option<usize> {
    let count = entries.len();
    let mut index = match from {
        Some(index) => index,
        // The first press lands on the first (or last) line.
        None if down => count.checked_sub(1)?,
        None => 0,
    };
    for _ in 0..count {
        index = if down {
            (index + 1) % count
        } else {
            (index + count - 1) % count
        };
        if entries[index].can_pick() {
            return Some(index);
        }
    }
    None
}

/// The entries of the submenu on line `index`, if there is one there.
pub fn submenu_entries<Message>(
    entries: &mut [Entry<Message>],
    index: usize,
) -> Option<Vec<Entry<Message>>> {
    match entries.get_mut(index) {
        Some(Entry::Submenu { entries, .. }) => Some(std::mem::take(entries)),
        _ => None,
    }
}

/// Whether line `index` opens a submenu.
pub fn is_submenu<Message>(entries: &[Entry<Message>], index: usize) -> bool {
    matches!(entries.get(index), Some(Entry::Submenu { entries, .. }) if !entries.is_empty())
}

/// What picking line `index` sends, if it is an item that can be picked.
pub fn take_press<Message>(entries: Vec<Entry<Message>>, index: usize) -> Option<Message> {
    match entries.into_iter().nth(index) {
        Some(Entry::Item { on_press, .. }) => on_press,
        _ => None,
    }
}

/// The menu bar: the logo, then a title for each menu.
///
/// Clicking a title sends `on_open`; while a menu is open, moving over another
/// title sends `on_hover` so the user can slide between menus. Titles `can_open`
/// says no to are greyed out, with `why` as their hover text, if it gives one.
pub fn bar<'a, Message: Clone + 'a>(
    c: Colors,
    logo: &svg::Handle,
    open: Option<Menu>,
    can_open: impl Fn(Menu) -> bool,
    why: impl Fn(Menu) -> Option<&'static str>,
    on_open: impl Fn(Menu) -> Message,
    on_hover: impl Fn(Menu) -> Message,
) -> Element<'a, Message> {
    let titles = Menu::ALL.into_iter().map(|menu| -> Element<'a, Message> {
        let enabled = can_open(menu);
        let label = text(menu.label())
            .size(13)
            .width(Fill)
            .align_x(Center)
            .color_maybe((!enabled).then_some(c.text_muted));
        let title = button(label)
            .width(menu.width())
            .padding([4, 0])
            .style(style::menu_title(c, open == Some(menu)))
            .on_press_maybe(enabled.then(|| on_open(menu)));

        let title: Element<'a, Message> = if open.is_some() && enabled {
            mouse_area(title).on_enter(on_hover(menu)).into()
        } else {
            title.into()
        };
        match why(menu) {
            Some(why) if !enabled => tooltip(
                title,
                container(text(why).size(12))
                    .padding([2, 6])
                    .style(style::dropdown(c)),
                tooltip::Position::Bottom,
            )
            .into(),
            _ => title,
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

/// The open dropdown, as a layer over the whole window, with its open submenu beside it.
///
/// Clicking anywhere outside the dropdown and the menu bar sends `on_close`. Moving over
/// a submenu's line sends `on_submenu` with the line, and moving over another line sends
/// it with `None`, which closes the submenu.
pub fn dropdown<'a, Message: Clone + 'a>(
    c: Colors,
    menu: Menu,
    mut entries: Vec<Entry<Message>>,
    cursor: Cursor,
    on_close: Message,
    on_submenu: impl Fn(Option<usize>) -> Message,
) -> Element<'a, Message> {
    let sub = cursor.submenu.and_then(|index| {
        let top: f32 = entries[..index].iter().map(Entry::height).sum();
        let entries = submenu_entries(&mut entries, index)?;
        Some((top, entries))
    });
    let dropdown = lines(
        c,
        entries,
        DROPDOWN_WIDTH,
        cursor.line,
        cursor.submenu,
        Some(&on_submenu),
    );

    // Below the menu bar, a transparent layer catches clicks outside the dropdown.
    let catcher = column![
        space().height(BAR_HEIGHT),
        mouse_area(space().width(Fill).height(Fill)).on_press(on_close),
    ];

    let mut beside = row![space().width(menu.offset()), opaque(dropdown)];
    if let Some((top, entries)) = sub {
        let submenu = lines(c, entries, SUBMENU_WIDTH, cursor.sub_line, None, None);
        // The submenu's first line lines up with the line it hangs from.
        beside = beside.push(column![space().height(top), opaque(submenu)]);
    }
    let placed = column![space().height(BAR_HEIGHT), beside].width(Length::Shrink);

    stack![catcher, placed].into()
}

/// The box of a dropdown or right-click menu, `width` wide, with a line per entry.
pub fn panel<'a, Message: Clone + 'a>(
    c: Colors,
    entries: Vec<Entry<Message>>,
    width: f32,
) -> Element<'a, Message> {
    lines(c, entries, width, None, None, None)
}

/// [`panel`], with the line the arrow keys are on lit up, and the submenu that is open.
fn lines<'a, Message: Clone + 'a>(
    c: Colors,
    entries: Vec<Entry<Message>>,
    width: f32,
    lit: Option<usize>,
    open: Option<usize>,
    on_submenu: Option<&dyn Fn(Option<usize>) -> Message>,
) -> Element<'a, Message> {
    let lines = entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| -> Element<'a, Message> {
            let style = style::menu_item(c, lit == Some(index) || open == Some(index));
            let line: Element<'a, Message> = match entry {
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
                .style(style)
                .on_press_maybe(on_press)
                .into(),
                Entry::Submenu { label, entries } => button(
                    row![
                        space().width(18),
                        text(label).size(13).width(Fill),
                        text("▸").size(12),
                    ]
                    .align_y(Center),
                )
                .width(Fill)
                .padding([4, 8])
                .style(style)
                .on_press_maybe(
                    // An open submenu's entries were taken out to draw it beside.
                    (!entries.is_empty() || open == Some(index))
                        .then(|| on_submenu.map(|on_submenu| on_submenu(Some(index))))
                        .flatten(),
                )
                .into(),
                Entry::Separator => container(rule::horizontal(1).style(style::separator(c)))
                    .padding([4, 6])
                    .into(),
            };
            // Moving onto a line opens its submenu, or closes the open one.
            match on_submenu {
                Some(on_submenu) => {
                    let hovered = matches!(open, Some(open) if open == index);
                    let mut area = mouse_area(line);
                    if !hovered {
                        area = area.on_enter(on_submenu(Some(index)));
                    }
                    area.into()
                }
                None => line,
            }
        });

    container(column(lines))
        .width(width)
        .padding(PANEL_PADDING)
        .style(style::dropdown(c))
        .into()
}

/// About how tall [`panel`] is with `entries`, for keeping a menu inside the window.
pub fn panel_height<Message>(entries: &[Entry<Message>]) -> f32 {
    let lines: f32 = entries.iter().map(Entry::height).sum();
    lines + PANEL_PADDING * 2.0 + 2.0
}
