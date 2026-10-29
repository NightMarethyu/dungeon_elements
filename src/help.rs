//! The Help window: every keyboard shortcut, the how-to guides, and what's new in this
//! version, with a search box over all of them.
//!
//! The guides are the Markdown files in `help/<language>/`, built into the app by
//! `build.rs`, so a new guide only takes a new file. Links between them are written
//! `[Links](guide:links)`, with the guide's file name less its number.

use iced::widget::markdown::{self, Uri};
use iced::widget::{Id, button, column, container, row, rule, scrollable, space, text, text_input};
use iced::{Center, Element, Fill, Font, font};

use crate::preview;
use crate::style;
use crate::theme::Colors;

/// Where to report a problem or ask for something.
pub const ISSUES_URL: &str = "https://github.com/NightMarethyu/dungeon_elements/issues/new";
/// The app's home on the web, with its releases and plan.
pub const PROJECT_URL: &str = "https://github.com/NightMarethyu/dungeon_elements";

/// Every guide in `help/`, as `(language, file name, text)`, sorted by language then name.
const GUIDES: &[(&str, &str, &str)] = include!(concat!(env!("OUT_DIR"), "/guides.rs"));
/// The language the guides are shown in. Only English is written so far.
const LANGUAGE: &str = "en";
const CHANGELOG: &str = include_str!("../CHANGELOG.md");

/// The scrollable that shows the page, so a new page starts at its top.
pub const PAGE: &str = "help-page";

/// The keyboard shortcuts, in groups. The tests check that every shortcut a menu shows
/// is here, so this list grows with the app.
pub const SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    (
        "Help",
        &[
            ("F1", "Open the how-to guides"),
            ("Ctrl+/", "Show this list of keyboard shortcuts"),
            ("Escape", "Close a menu or dialog"),
        ],
    ),
    (
        "Notes and tabs",
        &[
            ("Ctrl+N", "Make a new note"),
            ("Ctrl+O", "Open a file"),
            ("Ctrl+S", "Save the note"),
            ("Ctrl+Shift+S", "Save the note somewhere else (Save As)"),
            ("Ctrl+Alt+S", "Save every note with unsaved changes"),
            ("Ctrl+W", "Close the tab"),
        ],
    ),
    (
        "Views",
        &[
            ("Ctrl+R", "Switch between reading and editing the note"),
            ("Ctrl+E", "Switch between the form and the Markdown text"),
        ],
    ),
    (
        "Writing",
        &[
            ("Ctrl+B", "Make the selection (or the word) bold"),
            ("Ctrl+I", "Make the selection (or the word) italic"),
            (
                "Enter",
                "In a list, start the next item; on an empty item, end the list",
            ),
            ("Ctrl+K", "Insert a link, or turn the selection into one"),
            ("Ctrl+Shift+K", "Insert a picture from your computer"),
            ("Ctrl+X", "Cut"),
            ("Ctrl+C", "Copy"),
            ("Ctrl+V", "Paste"),
            ("Ctrl+A", "Select all"),
        ],
    ),
    (
        "Forms",
        &[
            ("Tab", "Go to the next field"),
            ("Shift+Tab", "Go back to the field before"),
            ("Enter", "In a steps field, add the step and start the next"),
        ],
    ),
    (
        "Links",
        &[
            ("[[", "Start a link; notes are suggested as you type"),
            ("Tab", "Insert the suggested note"),
            ("Enter", "In a link field, pick the first suggestion"),
            ("Ctrl+click", "Open the note a link points to"),
        ],
    ),
    (
        "Search",
        &[
            ("Ctrl+Shift+F", "Search your notes"),
            ("Enter", "In the search box, open the first note found"),
            ("Escape", "Leave the search and show the file tree again"),
        ],
    ),
    (
        "Quests",
        &[
            (
                "Ctrl+Shift+Q",
                "Show the Quests list in place of the file tree",
            ),
            ("Escape", "Close the Quests list"),
        ],
    ),
    (
        "Reading view",
        &[
            (
                "Click",
                "Edit a paragraph, list, field or the title in place",
            ),
            ("Escape", "Finish editing and show it formatted again"),
        ],
    ),
    (
        "Dice",
        &[("Enter", "In a die's Count box or the expression box, roll")],
    ),
];

/// A page of the Help window.
#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    Shortcuts,
    WhatsNew,
    /// The guide at this place in the list.
    Guide(usize),
}

#[derive(Debug, Clone)]
pub enum Message {
    Show(Page),
    Search(String),
    /// A link on the page was clicked.
    Link(Uri),
    Close,
}

/// What the app does for the Help window.
pub enum Action {
    /// Shows a new page, scrolled to its top.
    Scroll,
    Close,
    /// Opens a web page in the browser.
    Open(String),
}

struct Guide {
    /// The file name less its number and `.md`, for linking to it.
    id: String,
    title: String,
    text: &'static str,
    content: markdown::Content,
}

pub struct Help {
    page: Page,
    search: String,
    guides: Vec<Guide>,
    /// This version's part of the changelog, if it has one.
    whats_new: Option<markdown::Content>,
}

impl Help {
    pub fn new(page: Page) -> Self {
        let guides = GUIDES
            .iter()
            .filter(|(language, ..)| *language == LANGUAGE)
            .map(|(_, file, text)| guide(file, text))
            .collect();

        Help {
            page,
            search: String::new(),
            guides,
            whats_new: whats_new(CHANGELOG, env!("CARGO_PKG_VERSION"))
                .map(|text| markdown::Content::parse(&text)),
        }
    }

    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::Show(page) => self.page = page,
            Message::Search(search) => self.search = search,
            Message::Close => return Some(Action::Close),
            Message::Link(url) => {
                if let Some(id) = url.strip_prefix("guide:") {
                    let index = self.guides.iter().position(|guide| guide.id == id)?;
                    // So the guide shows as picked in the list, even if the search hid it.
                    self.search.clear();
                    self.page = Page::Guide(index);
                } else if url == "help:shortcuts" {
                    self.page = Page::Shortcuts;
                } else if url == "help:whats-new" {
                    self.page = Page::WhatsNew;
                } else if url.starts_with("http://") || url.starts_with("https://") {
                    return Some(Action::Open(url));
                } else {
                    // A link to a file in the repository, like the changelog's link to the plan.
                    return Some(Action::Open(format!("{PROJECT_URL}/blob/main/{url}")));
                }
            }
        }
        Some(Action::Scroll)
    }

    pub fn view(&self, c: Colors) -> Element<'_, Message> {
        let search = self.search.trim().to_lowercase();
        let nav = |label: String, page: Page| {
            let selected = self.page == page;
            button(text(label).size(13))
                .width(Fill)
                .padding([5, 8])
                .style(style::flat(c, selected))
                .on_press(Message::Show(page))
        };

        let mut sidebar = column![
            text_input("Search help", &self.search)
                .on_input(Message::Search)
                .padding(6)
                .size(13)
                .style(style::input(c)),
            space().height(6),
            nav("Keyboard Shortcuts".into(), Page::Shortcuts),
            nav("What's New".into(), Page::WhatsNew),
            space().height(6),
            text("HOW-TO GUIDES").size(11).color(c.text_muted),
        ]
        .spacing(2);
        let mut found = 0;
        for (index, guide) in self.guides.iter().enumerate() {
            if guide.matches(&search) {
                sidebar = sidebar.push(nav(guide.title.clone(), Page::Guide(index)));
                found += 1;
            }
        }
        if found == 0 {
            sidebar = sidebar.push(text("No guides match.").size(12).color(c.text_muted));
        }

        let settings = markdown::Settings::with_text_size(14, preview::markdown_style(c));
        let page: Element<'_, Message> = match &self.page {
            Page::Shortcuts => shortcuts(c, &search),
            Page::WhatsNew => match &self.whats_new {
                Some(content) => column![
                    title(c, format!("What's new in {}", env!("CARGO_PKG_VERSION"))),
                    markdown::view(content.items(), settings).map(Message::Link),
                ]
                .spacing(12)
                .into(),
                None => column![
                    title(c, "What's new".into()),
                    text("This version isn't in the changelog yet.").size(14),
                    button(text("See every release on the web").size(13))
                        .padding(0)
                        .style(style::flat(c, false))
                        .on_press(Message::Link(format!("{PROJECT_URL}/releases"))),
                ]
                .spacing(12)
                .into(),
            },
            Page::Guide(index) => match self.guides.get(*index) {
                Some(guide) => markdown::view(guide.content.items(), settings).map(Message::Link),
                None => text("This guide is missing.").into(),
            },
        };

        let content = column![
            row![
                text("Help").size(22).color(c.text),
                space::horizontal(),
                button(text("Close").size(13))
                    .style(style::primary(c))
                    .on_press(Message::Close),
            ]
            .align_y(Center),
            row![
                scrollable(container(sidebar).padding(iced::padding::right(12))).width(220),
                rule::vertical(1).style(style::separator(c)),
                scrollable(container(page).padding([4, 20]).width(Fill))
                    .id(Id::new(PAGE))
                    .height(Fill),
            ]
            .spacing(8)
            .height(Fill),
        ]
        .spacing(16);

        container(content)
            .width(Fill)
            .max_width(920)
            .height(Fill)
            .max_height(640)
            .padding(20)
            .style(style::dialog(c))
            .into()
    }
}

impl Guide {
    /// Whether `search` (in lowercase) is in its title or text, or is empty.
    fn matches(&self, search: &str) -> bool {
        search.is_empty() || self.text.to_lowercase().contains(search)
    }
}

/// A guide from its file name, like `03-links.md`, and its text.
fn guide(file: &str, text: &'static str) -> Guide {
    let stem = file.strip_suffix(".md").unwrap_or(file);
    let id = stem
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim_start_matches(['-', '_', ' ']);
    let title = text
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .unwrap_or(id)
        .trim();

    Guide {
        id: id.to_string(),
        title: title.to_string(),
        text,
        content: markdown::Content::parse(text),
    }
}

/// This version's part of `changelog`, without its `## [version]` heading.
fn whats_new(changelog: &str, version: &str) -> Option<String> {
    let heading = format!("## [{version}]");
    let start = changelog.find(&heading)?;
    let rest = &changelog[start..];
    let body = &rest[rest.find('\n')? + 1..];
    let end = body.find("\n## ").unwrap_or(body.len());
    Some(body[..end].trim().to_string())
}

fn title<'a>(c: Colors, title: String) -> Element<'a, Message> {
    text(title)
        .size(26)
        .color(c.text)
        .font(Font {
            weight: font::Weight::Bold,
            ..Font::DEFAULT
        })
        .into()
}

/// The page of keyboard shortcuts, only those that mention `search` if it isn't empty.
fn shortcuts<'a>(c: Colors, search: &str) -> Element<'a, Message> {
    let mut page = column![
        title(c, "Keyboard shortcuts".into()),
        text("On a Mac, use Cmd where it says Ctrl.")
            .size(13)
            .color(c.text_muted),
    ]
    .spacing(8);

    let mut found = false;
    for (group, keys) in SHORTCUTS {
        let rows: Vec<Element<'a, Message>> = keys
            .iter()
            .filter(|(keys, what)| {
                search.is_empty()
                    || group.to_lowercase().contains(search)
                    || keys.to_lowercase().contains(search)
                    || what.to_lowercase().contains(search)
            })
            .map(|(keys, what)| {
                row![
                    container(
                        container(text(*keys).size(12).font(Font::MONOSPACE))
                            .padding([2, 6])
                            .style(style::card(c))
                    )
                    .width(150),
                    text(*what).size(13).color(c.text),
                ]
                .align_y(Center)
                .into()
            })
            .collect();
        if rows.is_empty() {
            continue;
        }
        found = true;
        page = page.push(space().height(8));
        page = page.push(text(*group).size(16).color(c.text).font(Font {
            weight: font::Weight::Bold,
            ..Font::DEFAULT
        }));
        page = page.push(column(rows).spacing(6));
    }
    if !found {
        page = page.push(text("No shortcuts match.").size(13).color(c.text_muted));
    }

    page.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guides_have_titles_and_ids() {
        let help = Help::new(Page::Shortcuts);
        assert!(!help.guides.is_empty());
        assert_eq!(help.guides[0].id, "getting-started");
        assert_eq!(help.guides[0].title, "Getting started");
    }

    #[test]
    fn links_between_guides_point_at_guides() {
        let help = Help::new(Page::Shortcuts);
        for guide in &help.guides {
            for (at, _) in guide.text.match_indices("](guide:") {
                let rest = &guide.text[at + "](guide:".len()..];
                let id = &rest[..rest.find(')').unwrap()];
                assert!(
                    help.guides.iter().any(|other| other.id == id),
                    "{} links to a guide that doesn't exist: {id}",
                    guide.title
                );
            }
        }
    }

    #[test]
    fn following_a_guide_link_shows_it() {
        let mut help = Help::new(Page::Shortcuts);
        help.update(Message::Link("guide:links".into()));
        let Page::Guide(index) = help.page else {
            panic!("no guide shown");
        };
        assert_eq!(help.guides[index].id, "links");
    }

    #[test]
    fn whats_new_is_one_version() {
        let changelog = "# Changelog\n\n## [Unreleased]\n\n## [0.2.0] - 2026-10-27\n\nNew.\n\n### Links\n\n- One\n\n## [0.1.0] - 2026-10-25\n\nOld.\n";
        assert_eq!(
            whats_new(changelog, "0.2.0").as_deref(),
            Some("New.\n\n### Links\n\n- One")
        );
        assert_eq!(whats_new(changelog, "9.9.9"), None);
        assert!(whats_new(CHANGELOG, env!("CARGO_PKG_VERSION")).is_some());
    }

    /// Every shortcut a menu shows is in the list, so it never falls behind the menus.
    #[test]
    fn every_menu_shortcut_is_listed() {
        let listed: Vec<&str> = SHORTCUTS
            .iter()
            .flat_map(|(_, keys)| keys.iter().map(|(keys, _)| *keys))
            .collect();
        let source = include_str!("main.rs");
        let mut found = 0;
        for (at, _) in source.match_indices("shortcut(") {
            let rest = &source[at..];
            let line = &rest[..rest.find('\n').unwrap_or(rest.len())];
            // `.shortcut("Ctrl+S")`, or `shortcut(item(...), "Ctrl+S")` in the right-click menus.
            let Some(start) = line.rfind(", \"").map(|at| at + 3).or_else(|| {
                line.strip_prefix("shortcut(\"")
                    .map(|_| "shortcut(\"".len())
            }) else {
                continue;
            };
            let Some(keys) = line[start..].split('"').next() else {
                continue;
            };
            found += 1;
            assert!(listed.contains(&keys), "{keys} isn't in help::SHORTCUTS");
        }
        assert!(found > 10, "only found {found} menu shortcuts");
    }
}
