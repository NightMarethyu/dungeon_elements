//! View > Theme > Edit Themes...: make a color theme by picking five colors, and change or
//! delete the ones made before, with the whole window showing the colors as they're picked.
//!
//! A theme made here is saved as an ordinary theme file in the user's themes folder (see
//! [`theme::user_folder`]), so nothing about how themes load changes, and nobody has to
//! write one by hand.

use std::path::PathBuf;

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input, tooltip};
use iced::{Center, Color, Element, Fill};

use crate::color_picker::{self, Picker};
use crate::style;
use crate::theme::{self, Colors, Main, Theme};

/// What the picker changes: one of the five main colors, which brings the colors worked
/// out from it along, or any single color by its name in [`theme::ROLES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Main(Main),
    Role(&'static str),
}

impl Target {
    fn get(self, colors: &Colors) -> Color {
        match self {
            Target::Main(main) => main.get(colors),
            Target::Role(role) => colors.get(role).unwrap_or(Color::BLACK),
        }
    }

    fn set(self, colors: &mut Colors, color: Color) {
        match self {
            Target::Main(main) => main.set(colors, color),
            Target::Role(role) => {
                let _ = colors.set(role, Color { a: 1.0, ..color });
            }
        }
    }

    fn label(self) -> &'static str {
        match self {
            Target::Main(main) => main.label(),
            Target::Role(role) => theme::role_label(role),
        }
    }
}

/// The theme being made or changed.
#[derive(Debug, Clone)]
struct Draft {
    name: String,
    colors: Colors,
    /// The theme it changes, by name, or `None` for a new one.
    original: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Show the theme at this place in the list.
    Select(usize),
    NewTheme,
    MakeCopy,
    Name(String),
    /// Pick this color.
    Pick(Target),
    /// Show every color, or only the five main ones.
    ShowAll(bool),
    /// Save a copy of the theme shown somewhere else, to give to someone.
    Share,
    /// Bring in a theme file someone gave you.
    AddTheme,
    Picker(color_picker::Message),
    Save,
    Delete,
    UndoDelete,
    Close,
}

/// What the app has to do after a message.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Nothing,
    /// Close the editor, keeping the theme that was in use.
    Close,
    /// Load the themes again and use the one called this, then close the editor.
    Saved(String),
    /// Load the themes again, since one was deleted, put back or added, and show the one
    /// called this (or the one in use).
    Reload(Option<String>),
    /// Ask where to save a copy of a theme: its file name and what goes in it.
    Share(String, String),
    /// Ask for a theme file to add, and hand it to [`Editor::add`].
    PickFile,
}

pub struct Editor {
    /// The theme shown, as its place in the app's list.
    selected: usize,
    draft: Option<Draft>,
    /// Which color the picker changes.
    picking: Target,
    /// Whether every color is listed, not only the five main ones.
    all: bool,
    /// What happened last, like where a shared copy went.
    notice: Option<String>,
    picker: Picker,
    error: Option<String>,
    /// The last theme deleted, for Undo: its file, what was in it, and its name.
    deleted: Option<(PathBuf, String, String)>,
}

/// Colors offered under the picker, for a quick start.
const SWATCHES: [&str; 16] = [
    "#101014", "#1e1e1e", "#2b2d42", "#24103a", "#0b2e2a", "#f5eedb", "#fbf7ec", "#ffffff",
    "#57c08c", "#39ff14", "#ff6ad5", "#00e5ff", "#ffb000", "#e04f5f", "#7a2e12", "#4a7bd0",
];

impl Editor {
    /// The editor, showing the theme at `current` in `themes`.
    pub fn new(themes: &[Theme], current: usize) -> Self {
        let mut editor = Editor {
            selected: current,
            draft: None,
            picking: Target::Main(Main::Background),
            all: false,
            notice: None,
            picker: Picker::new(Color::BLACK),
            error: None,
            deleted: None,
        };
        editor.select(themes, current);
        editor
    }

    /// The colors to show the app in now: the draft's, or the theme shown.
    pub fn preview(&self, themes: &[Theme]) -> Colors {
        match &self.draft {
            Some(draft) => draft.colors,
            None => themes
                .get(self.selected)
                .or_else(|| themes.first())
                .map(|theme| theme.colors)
                .expect("there are always built-in themes"),
        }
    }

    fn select(&mut self, themes: &[Theme], index: usize) {
        let Some(theme) = themes.get(index) else {
            return;
        };
        self.selected = index;
        self.error = None;
        // A theme made by the user can be changed straight away; a built-in one is copied.
        self.draft = theme.path.is_some().then(|| Draft {
            name: theme.name.clone(),
            colors: theme.colors,
            original: Some(theme.name.clone()),
        });
        self.pick(self.picking, themes);
    }

    fn pick(&mut self, target: Target, themes: &[Theme]) {
        self.picking = target;
        self.picker = Picker::new(target.get(&self.preview(themes)));
    }

    fn start(&mut self, name: String, colors: Colors, themes: &[Theme]) {
        self.draft = Some(Draft {
            name,
            colors,
            original: None,
        });
        self.error = None;
        self.pick(self.picking, themes);
    }

    pub fn update(&mut self, message: Message, themes: &[Theme]) -> Outcome {
        match message {
            Message::Select(index) => {
                self.notice = None;
                self.select(themes, index);
            }
            Message::NewTheme => {
                // A new theme starts from Dark, or Light when a light one is showing.
                let light = !theme::is_dark(self.preview(themes).background);
                let base = if light {
                    theme::LIGHT_NAME
                } else {
                    theme::DEFAULT
                };
                let colors = themes
                    .iter()
                    .find(|theme| theme.name == base)
                    .map(|theme| theme.colors)
                    .unwrap_or_else(|| self.preview(themes));
                self.start(free_name("My theme", themes), colors, themes);
            }
            Message::MakeCopy => {
                let Some(theme) = themes.get(self.selected) else {
                    return Outcome::Nothing;
                };
                let colors = self.preview(themes);
                let name = free_name(&format!("{} copy", theme.name), themes);
                self.start(name, colors, themes);
            }
            Message::Name(name) => {
                if let Some(draft) = &mut self.draft {
                    draft.name = name;
                    self.error = None;
                }
            }
            Message::Pick(target) => self.pick(target, themes),
            Message::ShowAll(all) => {
                self.all = all;
                if !all && matches!(self.picking, Target::Role(_)) {
                    self.pick(Target::Main(Main::Background), themes);
                }
            }
            Message::Share => {
                let name = match &self.draft {
                    Some(draft) if !draft.name.trim().is_empty() => draft.name.trim().to_string(),
                    _ => themes
                        .get(self.selected)
                        .map(|theme| theme.name.clone())
                        .unwrap_or_default(),
                };
                let yaml = theme::to_yaml(&name, &self.preview(themes));
                return Outcome::Share(theme::file_name(&name), yaml);
            }
            Message::AddTheme => return Outcome::PickFile,
            Message::Picker(message) => {
                if let (Some(color), Some(draft)) = (self.picker.update(message), &mut self.draft) {
                    self.picking.set(&mut draft.colors, color);
                }
            }
            Message::Save => return self.save(themes),
            Message::Delete => return self.delete(themes),
            Message::UndoDelete => {
                let Some((path, text, name)) = self.deleted.take() else {
                    return Outcome::Nothing;
                };
                if let Err(error) = std::fs::write(&path, text) {
                    self.error = Some(format!("Could not put {name} back: {error}"));
                    return Outcome::Nothing;
                }
                return Outcome::Reload(Some(name));
            }
            Message::Close => return Outcome::Close,
        }
        Outcome::Nothing
    }

    /// Says `notice` under the buttons, like where a shared copy was saved.
    pub fn notify(&mut self, notice: String) {
        self.notice = Some(notice);
    }

    /// Adds the theme file at `path` to the user's themes, under a name no theme has, and
    /// shows it.
    pub fn add(&mut self, path: &std::path::Path, themes: &[Theme]) -> Outcome {
        let Some(folder) = theme::user_folder() else {
            self.error = Some("Your settings folder could not be found.".into());
            return Outcome::Nothing;
        };
        self.add_to(&folder, path, themes)
    }

    /// [`Editor::add`], into `folder`.
    fn add_to(
        &mut self,
        folder: &std::path::Path,
        path: &std::path::Path,
        themes: &[Theme],
    ) -> Outcome {
        let file = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let theme = match std::fs::read_to_string(path)
            .map_err(|error| error.to_string())
            .and_then(|yaml| theme::parse_user(&yaml, themes))
        {
            Ok(theme) if !theme.name.trim().is_empty() => theme,
            Ok(_) => {
                self.error = Some(format!("{file} is a theme with no name."));
                return Outcome::Nothing;
            }
            Err(error) => {
                self.error = Some(format!(
                    "{file} isn't a theme Dungeon Elements can read: {error}"
                ));
                return Outcome::Nothing;
            }
        };
        // A friend's Midnight beside your own Midnight becomes Midnight 2.
        let name = free_name(theme.name.trim(), themes);
        match write(folder, &name, &theme.colors, None) {
            Ok(_) => Outcome::Reload(Some(name)),
            Err(error) => {
                self.error = Some(error);
                Outcome::Nothing
            }
        }
    }

    /// After the themes are loaded again: shows the one called `name`, if there is one.
    pub fn reloaded(&mut self, themes: &[Theme], name: Option<&str>) {
        let index = name
            .and_then(|name| themes.iter().position(|theme| theme.name == name))
            .unwrap_or(0);
        self.select(themes, index);
    }

    fn save(&mut self, themes: &[Theme]) -> Outcome {
        let Some(draft) = &self.draft else {
            return Outcome::Nothing;
        };
        let name = draft.name.trim().to_string();
        if let Some(problem) = name_problem(&name, draft.original.as_deref(), themes) {
            self.error = Some(problem);
            return Outcome::Nothing;
        }
        let Some(folder) = theme::user_folder() else {
            self.error = Some(
                "Your settings folder could not be found, so the theme can't be saved.".into(),
            );
            return Outcome::Nothing;
        };
        let old = draft
            .original
            .as_ref()
            .and_then(|old| themes.iter().find(|theme| &theme.name == old))
            .and_then(|theme| theme.path.clone());
        match write(&folder, &name, &draft.colors, old.as_deref()) {
            Ok(_) => Outcome::Saved(name),
            Err(error) => {
                self.error = Some(error);
                Outcome::Nothing
            }
        }
    }

    fn delete(&mut self, themes: &[Theme]) -> Outcome {
        let Some(theme) = themes.get(self.selected) else {
            return Outcome::Nothing;
        };
        let Some(path) = &theme.path else {
            return Outcome::Nothing;
        };
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                self.error = Some(format!("Could not read {}: {error}", theme.name));
                return Outcome::Nothing;
            }
        };
        if let Err(error) = std::fs::remove_file(path) {
            self.error = Some(format!("Could not delete {}: {error}", theme.name));
            return Outcome::Nothing;
        }
        self.deleted = Some((path.clone(), text, theme.name.clone()));
        self.draft = None;
        Outcome::Reload(None)
    }

    pub fn view<'a>(
        &'a self,
        c: Colors,
        themes: &'a [Theme],
        in_use: usize,
    ) -> Element<'a, Message> {
        let label = |words: &'static str| text(words).size(12).color(c.text_muted);

        // The themes, built-in first, as the View > Theme menu lists them.
        let mut list = column![label("THEMES")].spacing(2);
        for (index, theme) in themes.iter().enumerate() {
            let mut name = theme.name.clone();
            if index == in_use {
                name.push_str("  (in use)");
            }
            list = list.push(
                button(text(name).size(13))
                    .width(Fill)
                    .padding([5, 8])
                    .style(style::flat(c, index == self.selected))
                    .on_press(Message::Select(index)),
            );
        }
        list = list.push(Space::new().height(8)).push(
            row![
                button(text("New Theme").size(13))
                    .padding([5, 10])
                    .style(style::secondary(c))
                    .on_press(Message::NewTheme),
                tooltip(
                    button(text("Add Theme...").size(13))
                        .padding([5, 10])
                        .style(style::secondary(c))
                        .on_press(Message::AddTheme),
                    container(text("Bring in a theme file someone gave you").size(12))
                        .padding(6)
                        .style(style::dropdown(c)),
                    tooltip::Position::Bottom,
                ),
            ]
            .spacing(6)
            .wrap(),
        );
        if let Some((_, _, name)) = &self.deleted {
            list = list.push(
                column![
                    text(format!("Deleted {name}."))
                        .size(12)
                        .color(c.text_muted),
                    button(text("Undo").size(12))
                        .padding([3, 8])
                        .style(style::secondary(c))
                        .on_press(Message::UndoDelete),
                ]
                .spacing(4),
            );
        }
        let list = container(scrollable(list).height(Fill)).width(190);

        let shown = &themes[self.selected.min(themes.len() - 1)];
        let colors = self.preview(themes);
        let editing = self.draft.is_some();

        // The name, or a built-in theme's note.
        let top: Element<'a, Message> = match &self.draft {
            Some(draft) => column![
                label("Name"),
                text_input("My theme", &draft.name)
                    .on_input(Message::Name)
                    .on_submit(Message::Save)
                    .padding(8)
                    .style(style::input(c)),
            ]
            .spacing(4)
            .into(),
            None => column![
                text(&shown.name).size(18).color(c.text),
                text("A built-in theme can't be changed. Make a Copy to start a theme of your own from it.")
                    .size(12)
                    .color(c.text_muted),
            ]
            .spacing(4)
            .into(),
        };

        // The five main colors, or every color: click one to pick it.
        let row_for = |target: Target, hint: Option<&'static str>| {
            let color = target.get(&colors);
            let picked = editing && target == self.picking;
            let size = if hint.is_some() { 28 } else { 20 };
            let swatch = container(Space::new())
                .width(size)
                .height(size)
                .style(color_picker::swatch_style(color, c));
            let mut words = column![text(target.label()).size(13).color(c.text)]
                .spacing(1)
                .width(Fill);
            if let Some(hint) = hint {
                words = words.push(text(hint).size(11).color(c.text_muted));
            }
            let line = row![
                swatch,
                words,
                text(theme::to_hex(color)).size(12).color(c.text_muted)
            ]
            .spacing(10)
            .align_y(Center);
            button(line)
                .width(Fill)
                .padding(if hint.is_some() { [5, 8] } else { [3, 8] })
                .style(style::flat(c, picked))
                .on_press_maybe(editing.then_some(Message::Pick(target)))
        };
        let colors_list: Element<'a, Message> = if self.all {
            let mut every = column![
                text("Changing one of the five main colors works the colors that come from it out again.")
                    .size(11)
                    .color(c.text_muted)
            ]
            .spacing(2);
            for role in theme::ROLES {
                every = every.push(row_for(Target::Role(role), None));
            }
            scrollable(every.padding(iced::Padding::ZERO.right(8)))
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new().width(3).scroller_width(3),
                ))
                .height(Fill)
                .into()
        } else {
            column(
                Main::ALL
                    .into_iter()
                    .map(|main| row_for(Target::Main(main), Some(main.hint())).into()),
            )
            .spacing(4)
            .into()
        };
        let switch = |words: &'static str, all: bool| {
            button(text(words).size(12))
                .padding([2, 8])
                .style(style::flat(c, self.all == all))
                .on_press(Message::ShowAll(all))
        };
        let colors_header = row![
            label("Colors"),
            Space::new().width(Fill),
            switch("Main five", false),
            switch("Every color", true),
        ]
        .spacing(4)
        .align_y(Center);

        let problems = theme::readability(&colors);
        let readability: Element<'a, Message> = if problems.is_empty() {
            text("All the text is easy to read.")
                .size(12)
                .color(c.text_muted)
                .into()
        } else {
            column(problems.into_iter().map(|problem| {
                text(format!("⚠ {problem}"))
                    .size(12)
                    .color(c.warning)
                    .into()
            }))
            .spacing(2)
            .into()
        };

        let middle = column![top, column![colors_header, colors_list].spacing(6)]
            .spacing(14)
            .width(300)
            .height(Fill);

        let swatches: Vec<Color> = SWATCHES
            .iter()
            .filter_map(|code| theme::parse_color(code))
            .collect();
        let picker: Element<'a, Message> = if editing {
            column![
                text(format!("Pick: {}", self.picking.label()))
                    .size(12)
                    .color(c.text_muted),
                self.picker.view(c, 260.0, &swatches).map(Message::Picker),
                Space::new().height(6),
                readability,
            ]
            .spacing(6)
            .width(260)
            .into()
        } else {
            column![readability].width(260).into()
        };

        // Delete for a theme of the user's own; Make a Copy for any theme shown.
        let mut buttons = row![].spacing(8).align_y(Center);
        if shown.path.is_some() && self.draft.as_ref().is_some_and(|d| d.original.is_some()) {
            buttons = buttons.push(
                button(text("Delete"))
                    .style(style::secondary(c))
                    .on_press(Message::Delete),
            );
        }
        if self.draft.as_ref().is_none_or(|d| d.original.is_some()) {
            buttons = buttons.push(
                button(text("Make a Copy"))
                    .style(style::secondary(c))
                    .on_press(Message::MakeCopy),
            );
        }
        buttons = buttons.push(tooltip(
            button(text("Share..."))
                .style(style::secondary(c))
                .on_press(Message::Share),
            container(text("Save a copy of this theme to give to someone").size(12))
                .padding(6)
                .style(style::dropdown(c)),
            tooltip::Position::Top,
        ));
        buttons = buttons.push(Space::new().width(Fill));
        if let Some(error) = &self.error {
            buttons = buttons.push(text(error).size(12).color(c.danger));
        } else if let Some(notice) = &self.notice {
            buttons = buttons.push(text(notice).size(12).color(c.text_muted));
        }
        buttons = buttons
            .push(
                button(text("Cancel"))
                    .style(style::secondary(c))
                    .on_press(Message::Close),
            )
            .push(
                button(text("Save"))
                    .style(style::primary(c))
                    .on_press_maybe(editing.then_some(Message::Save)),
            );

        let content = column![
            text("Edit Themes").size(22).color(c.text),
            text("The window behind shows each color as you pick it. Save keeps the theme and uses it; Cancel goes back to the one you had.")
                .size(12)
                .color(c.text_muted),
            row![list, middle, picker].spacing(20).height(Fill),
            buttons,
        ]
        .spacing(12);

        container(content)
            .width(860)
            .height(560)
            .padding(24)
            .style(style::dialog(c))
            .into()
    }
}

/// Why `name` can't name a theme (or the one called `old`, being changed), or `None`.
fn name_problem(name: &str, old: Option<&str>, themes: &[Theme]) -> Option<String> {
    if name.trim().is_empty() {
        return Some("Give the theme a name.".into());
    }
    let taken = themes.iter().find(|theme| {
        theme.name.eq_ignore_ascii_case(name.trim())
            && old.is_none_or(|old| !old.eq_ignore_ascii_case(&theme.name))
    })?;
    Some(if theme_is_built_in(taken) {
        format!("{} is a built-in theme's name.", taken.name)
    } else {
        format!("There is a theme called {} already.", taken.name)
    })
}

fn theme_is_built_in(theme: &Theme) -> bool {
    theme.path.is_none()
}

/// `base`, or `base 2`, `base 3`... whichever no theme has.
fn free_name(base: &str, themes: &[Theme]) -> String {
    let taken = |name: &str| {
        themes
            .iter()
            .any(|theme| theme.name.eq_ignore_ascii_case(name))
    };
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|name| !taken(name))
        .expect("some number is free")
}

/// Saves the theme `name` with `colors` in `folder`, and removes its `old` file if it was
/// saved under another name. Gives the new file.
fn write(
    folder: &std::path::Path,
    name: &str,
    colors: &Colors,
    old: Option<&std::path::Path>,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(folder)
        .map_err(|error| format!("Could not make {}: {error}", folder.display()))?;
    let mut path = folder.join(theme::file_name(name));
    // Another theme's file can have the name this one's would (Pink? and Pink!).
    let mut n = 2;
    while path.exists() && old.is_none_or(|old| old != path) {
        let file = theme::file_name(&format!("{name} {n}"));
        path = folder.join(file);
        n += 1;
    }
    std::fs::write(&path, theme::to_yaml(name, colors))
        .map_err(|error| format!("Could not save {}: {error}", path.display()))?;
    if let Some(old) = old.filter(|old| *old != path) {
        let _ = std::fs::remove_file(old);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_colors_can_be_fine_tuned() {
        let themes = theme::built_in();
        let mut editor = Editor::new(&themes, 1);
        editor.update(Message::MakeCopy, &themes);
        editor.update(Message::ShowAll(true), &themes);
        editor.update(Message::Pick(Target::Role("border")), &themes);
        editor.update(
            Message::Picker(color_picker::Message::Code("#ff0000".into())),
            &themes,
        );
        let colors = editor.preview(&themes);
        assert_eq!(theme::to_hex(colors.border), "#ff0000");
        // Only that color changes.
        assert_eq!(colors.hover, themes[1].colors.hover);

        // Going back to the main five picks one of them.
        editor.update(Message::ShowAll(false), &themes);
        assert_eq!(editor.picking, Target::Main(Main::Background));
    }

    #[test]
    fn themes_are_shared_and_added() {
        let folder =
            std::env::temp_dir().join(format!("dungeon_elements_shared_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let themes = theme::built_in();

        // Share... gives the shown theme's file, even a built-in one.
        let neon = themes.iter().position(|t| t.name == "Neon").unwrap();
        let mut editor = Editor::new(&themes, neon);
        let Outcome::Share(file, yaml) = editor.update(Message::Share, &themes) else {
            panic!("Share should ask where to save");
        };
        assert_eq!(file, "Neon.yaml");
        let given = folder.join("from a friend.yaml");
        std::fs::write(&given, yaml).unwrap();

        // Adding it beside the built-in Neon makes it Neon 2.
        let user = folder.join("themes");
        assert_eq!(
            editor.add_to(&user, &given, &themes),
            Outcome::Reload(Some("Neon 2".into()))
        );
        let (loaded, problems) = theme::load(Some(&user));
        assert!(problems.is_empty(), "{problems:?}");
        let added = loaded.iter().find(|t| t.name == "Neon 2").unwrap();
        assert_eq!(added.colors.accent, themes[neon].colors.accent);

        // A file that isn't a theme says so.
        std::fs::write(&given, "hello: there\n").unwrap();
        assert_eq!(editor.add_to(&user, &given, &themes), Outcome::Nothing);
        assert!(editor.error.as_ref().unwrap().contains("isn't a theme"));

        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn names_must_be_new() {
        let themes = theme::built_in();
        assert!(name_problem(" ", None, &themes).is_some());
        assert_eq!(
            name_problem("dark", None, &themes).unwrap(),
            "Dark is a built-in theme's name."
        );
        assert_eq!(name_problem("Midnight", None, &themes), None);
        assert_eq!(free_name("Dark", &themes), "Dark 2");
    }

    #[test]
    fn themes_are_made_changed_and_deleted() {
        let folder =
            std::env::temp_dir().join(format!("dungeon_elements_themes_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let (themes, _) = theme::load(None);

        // A new theme, with one color changed, saved where themes load from.
        let mut editor = Editor::new(&themes, 1);
        assert!(editor.draft.is_none(), "Dark is built in");
        editor.update(Message::NewTheme, &themes);
        editor.update(Message::Name("Midnight".into()), &themes);
        editor.update(Message::Pick(Target::Main(Main::Accent)), &themes);
        editor.update(
            Message::Picker(color_picker::Message::Code("#ff6ad5".into())),
            &themes,
        );
        assert_eq!(theme::to_hex(editor.preview(&themes).accent), "#ff6ad5");
        let draft = editor.draft.clone().unwrap();
        let path = write(&folder, &draft.name, &draft.colors, None).unwrap();
        let (themes, problems) = theme::load(Some(&folder));
        assert!(problems.is_empty(), "{problems:?}");
        let midnight = themes.iter().position(|t| t.name == "Midnight").unwrap();
        assert_eq!(theme::to_hex(themes[midnight].colors.accent), "#ff6ad5");

        // Renaming it moves its file.
        editor.reloaded(&themes, Some("Midnight"));
        assert_eq!(
            editor.draft.as_ref().unwrap().original.as_deref(),
            Some("Midnight")
        );
        let renamed = write(&folder, "Late night", &draft.colors, Some(&path)).unwrap();
        assert!(!path.exists());
        assert!(renamed.exists());
        let (themes, _) = theme::load(Some(&folder));
        let late = themes.iter().position(|t| t.name == "Late night").unwrap();

        // Deleting it can be undone.
        editor.reloaded(&themes, Some("Late night"));
        assert_eq!(editor.selected, late);
        assert_eq!(
            editor.update(Message::Delete, &themes),
            Outcome::Reload(None)
        );
        assert!(!renamed.exists());
        assert_eq!(
            editor.update(Message::UndoDelete, &themes),
            Outcome::Reload(Some("Late night".into()))
        );
        assert!(renamed.exists());

        let _ = std::fs::remove_dir_all(folder);
    }
}
