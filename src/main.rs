//! Dungeon Elements: a local-first campaign manager for tabletop RPGs.

mod campaign;
mod menu;
mod settings;
mod style;
mod theme;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{
    Svg, button, center, checkbox, column, container, opaque, row, rule, scrollable, space, stack,
    svg, text, text_editor, text_input,
};
use iced::{Center, Element, Fill, Subscription, Task, window};

use campaign::Entry;
use menu::Menu;
use settings::Settings;
use theme::{Colors, Theme};

/// The logo, drawn in black on white. [`logo`] recolors it to match the theme.
const LOGO: &str = include_str!("../reference/dungeon_elements.svg");

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .window_size((1200.0, 800.0))
        // Closing the window asks about unsaved changes first; see `Message::Exit`.
        .exit_on_close_request(false)
        .run()
}

struct App {
    screen: Screen,
    settings: Settings,
    themes: Vec<Theme>,
    /// The index of the active theme in `themes`.
    theme: usize,
    /// The logo colored for the menu bar, and for the splash screen.
    logos: (svg::Handle, svg::Handle),
    /// The dropdown menu that is open, if any.
    menu: Option<Menu>,
    /// Whether the "About" dialog is showing.
    about: bool,
}

enum Screen {
    /// Asks whether to open a campaign or create a new one.
    Splash {
        new_campaign: Option<NewCampaign>,
        error: Option<String>,
    },
    Workspace(Workspace),
}

impl Default for Screen {
    fn default() -> Self {
        Screen::Splash {
            new_campaign: None,
            error: None,
        }
    }
}

/// The widget id of the title field in the "New campaign" dialog.
const TITLE_INPUT: &str = "new-campaign-title";

/// The fields of the "New campaign" dialog.
struct NewCampaign {
    title: String,
    location: String,
    with_template: bool,
    error: Option<String>,
}

/// An open campaign folder: its file tree and the files open in tabs.
struct Workspace {
    root: PathBuf,
    tree: Vec<Entry>,
    /// The folders in the tree that are expanded.
    expanded: HashSet<PathBuf>,
    tabs: Vec<Tab>,
    /// The `id` of the tab shown in the editor.
    active: Option<u64>,
    next_id: u64,
    status: String,
}

struct Tab {
    id: u64,
    content: text_editor::Content,
    /// The file the text was last saved to, if any.
    path: Option<PathBuf>,
    /// Whether the text has changed since it was last saved.
    modified: bool,
}

impl Tab {
    fn name(&self, show_extensions: bool) -> String {
        self.path.as_deref().map_or("Untitled".into(), |path| {
            display_name(path, show_extensions)
        })
    }
}

/// The name to show for a file: its filename, without `.md` unless `show_extensions` is on.
///
/// Other extensions always show, so a `_type.yaml` can't be mistaken for a note.
fn display_name(path: &Path, show_extensions: bool) -> String {
    let name = if !show_extensions && campaign::is_markdown(path) {
        path.file_stem()
    } else {
        path.file_name()
    };

    name.map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[derive(Debug, Clone)]
enum Message {
    // Menus and app-wide settings
    MenuOpened(Menu),
    MenuHovered(Menu),
    CloseMenu,
    Escape,
    ThemePicked(String),
    ReloadThemes,
    ToggleExtensions(bool),
    ToggleTypeFiles(bool),
    ShowAbout,
    CloseAbout,
    Exit,
    ExitConfirmed(bool),

    // Splash screen
    OpenCampaign,
    CampaignPicked(Option<PathBuf>),
    ShowNewCampaign,
    CancelNewCampaign,
    TitleChanged(String),
    LocationChanged(String),
    BrowseLocation,
    LocationPicked(Option<PathBuf>),
    TemplateToggled(bool),
    CreateCampaign,

    // Workspace
    ToggleFolder(PathBuf),
    OpenEntry(PathBuf),
    RefreshTree,
    SelectTab(u64),
    CloseTab(u64),
    CloseTabConfirmed(u64, bool),
    CloseCampaign,
    CloseCampaignConfirmed(bool),
    Edit(text_editor::Action),
    NewNote,
    OpenFile,
    FilePicked(Option<PathBuf>),
    Save,
    SaveAs,
    Saved(u64, Result<Option<PathBuf>, String>),
    CloseActiveTab,
}

impl App {
    fn new() -> Self {
        let settings = Settings::load();
        let folder = theme::user_folder();
        if let Some(folder) = &folder {
            // So users can find where their themes go. Not being able to make it is fine.
            let _ = std::fs::create_dir_all(folder);
        }
        let (themes, problems) = theme::load(folder.as_deref());

        let mut app = App {
            screen: Screen::Splash {
                new_campaign: None,
                error: theme_problems(&problems),
            },
            settings,
            themes,
            theme: 0,
            logos: (
                svg::Handle::from_memory(LOGO.as_bytes()),
                svg::Handle::from_memory(LOGO.as_bytes()),
            ),
            menu: None,
            about: false,
        };
        let name = app.settings.theme.clone();
        app.use_theme(&name);
        app
    }

    fn colors(&self) -> Colors {
        self.themes[self.theme].colors
    }

    fn theme(&self) -> iced::Theme {
        self.themes[self.theme].to_iced()
    }

    /// Switches to the theme named `name`, or the default theme if there is none by that name.
    fn use_theme(&mut self, name: &str) {
        self.theme = self
            .themes
            .iter()
            .position(|theme| theme.name == name)
            .or_else(|| {
                self.themes
                    .iter()
                    .position(|theme| theme.name == theme::DEFAULT)
            })
            .unwrap_or(0);

        let c = self.colors();
        self.logos = (logo(c, c.menu_bar), logo(c, c.background));
    }

    /// Shows `message` in the status bar, or under the buttons on the splash screen.
    fn notify(&mut self, message: String) {
        match &mut self.screen {
            Screen::Splash { error, .. } => *error = Some(message),
            Screen::Workspace(workspace) => workspace.status = message,
        }
    }

    fn save_settings(&mut self) {
        if let Err(error) = self.settings.save() {
            self.notify(format!("Could not save settings: {error}"));
        }
    }

    fn has_unsaved_changes(&self) -> bool {
        match &self.screen {
            Screen::Splash { .. } => false,
            Screen::Workspace(workspace) => workspace.tabs.iter().any(|tab| tab.modified),
        }
    }

    fn title(&self) -> String {
        match &self.screen {
            Screen::Splash { .. } => "Dungeon Elements".into(),
            Screen::Workspace(workspace) => {
                format!("{} - Dungeon Elements", workspace.name())
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let keys = match self.screen {
            Screen::Splash { .. } => keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed { key, .. } => escape(&key),
                _ => None,
            }),
            Screen::Workspace(_) => keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed { key, modifiers, .. } => {
                    escape(&key).or_else(|| shortcut(&key, modifiers))
                }
                _ => None,
            }),
        };

        Subscription::batch([keys, window::close_requests().map(|_| Message::Exit)])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MenuOpened(menu) => {
                self.menu = if self.menu == Some(menu) {
                    None
                } else {
                    Some(menu)
                };
                return Task::none();
            }
            Message::MenuHovered(menu) => {
                if self.menu.is_some() {
                    self.menu = Some(menu);
                }
                return Task::none();
            }
            _ => {}
        }

        // Anything else, such as picking a menu item, closes the open menu.
        self.menu = None;

        match message {
            Message::CloseMenu => return Task::none(),
            Message::Escape => {
                if let Screen::Splash { new_campaign, .. } = &mut self.screen
                    && !self.about
                {
                    *new_campaign = None;
                }
                self.about = false;
                return Task::none();
            }
            Message::ThemePicked(name) => {
                self.use_theme(&name);
                self.settings.theme = self.themes[self.theme].name.clone();
                self.save_settings();
                return Task::none();
            }
            Message::ReloadThemes => {
                let folder = theme::user_folder();
                let (themes, problems) = theme::load(folder.as_deref());
                self.themes = themes;
                let name = self.settings.theme.clone();
                self.use_theme(&name);

                let message = theme_problems(&problems).unwrap_or_else(|| match &folder {
                    Some(folder) => format!(
                        "Loaded {} themes (add your own in {})",
                        self.themes.len(),
                        folder.display()
                    ),
                    None => format!("Loaded {} themes", self.themes.len()),
                });
                self.notify(message);
                return Task::none();
            }
            Message::ToggleExtensions(show) => {
                self.settings.show_extensions = show;
                self.save_settings();
                return Task::none();
            }
            Message::ToggleTypeFiles(show) => {
                self.settings.show_type_files = show;
                self.save_settings();
                return Task::none();
            }
            Message::ShowAbout => {
                self.about = true;
                return Task::none();
            }
            Message::CloseAbout => {
                self.about = false;
                return Task::none();
            }
            Message::Exit => {
                return if self.has_unsaved_changes() {
                    Task::perform(
                        confirm_discard("Some files have unsaved changes. Quit anyway?"),
                        Message::ExitConfirmed,
                    )
                } else {
                    iced::exit()
                };
            }
            Message::ExitConfirmed(yes) => {
                return if yes { iced::exit() } else { Task::none() };
            }
            _ => {}
        }

        match &mut self.screen {
            Screen::Splash {
                new_campaign,
                error,
            } => match message {
                Message::OpenCampaign => {
                    Task::perform(pick_folder("Open campaign folder"), Message::CampaignPicked)
                }
                Message::CampaignPicked(Some(root)) => {
                    if root.is_dir() {
                        self.screen = Screen::Workspace(Workspace::open(root));
                    } else {
                        *error = Some(format!("{} is not a folder", root.display()));
                    }
                    Task::none()
                }
                Message::ShowNewCampaign => {
                    *error = None;
                    *new_campaign = Some(NewCampaign {
                        title: String::new(),
                        location: default_location()
                            .map(|path| path.display().to_string())
                            .unwrap_or_default(),
                        with_template: true,
                        error: None,
                    });
                    iced::widget::operation::focus(TITLE_INPUT)
                }
                Message::CancelNewCampaign => {
                    *new_campaign = None;
                    Task::none()
                }
                Message::BrowseLocation => Task::perform(
                    pick_folder("Choose where to save the campaign"),
                    Message::LocationPicked,
                ),
                Message::CreateCampaign => {
                    let Some(form) = new_campaign else {
                        return Task::none();
                    };

                    match campaign::create(
                        Path::new(form.location.trim()),
                        &form.title,
                        form.with_template,
                    ) {
                        Ok(root) => self.screen = Screen::Workspace(Workspace::open(root)),
                        Err(message) => form.error = Some(message),
                    }
                    Task::none()
                }
                message => {
                    if let Some(form) = new_campaign {
                        match message {
                            Message::TitleChanged(title) => form.title = title,
                            Message::LocationChanged(location) => form.location = location,
                            Message::LocationPicked(Some(path)) => {
                                form.location = path.display().to_string();
                            }
                            Message::TemplateToggled(checked) => form.with_template = checked,
                            _ => {}
                        }
                        form.error = None;
                    }
                    Task::none()
                }
            },
            Screen::Workspace(workspace) => {
                if let Message::CloseCampaignConfirmed(true) = message {
                    self.screen = Screen::default();
                    return Task::none();
                }
                workspace.update(message)
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let c = self.colors();

        let bar = menu::bar(
            c,
            &self.logos.0,
            self.menu,
            Message::MenuOpened,
            Message::MenuHovered,
        );

        let body = match &self.screen {
            Screen::Splash { error, .. } => splash(c, &self.logos.1, error.as_deref()),
            Screen::Workspace(workspace) => workspace.view(c, &self.settings),
        };

        let edge = rule::horizontal(c.border_width.max(1.0)).style(style::separator(c));
        let mut view: Element<'_, Message> = column![bar, edge, body].into();

        if let Screen::Splash {
            new_campaign: Some(form),
            ..
        } = &self.screen
        {
            view = modal(view, new_campaign_dialog(c, form));
        }

        if self.about {
            view = modal(view, about_dialog(c, &self.logos.1));
        }

        if let Some(open) = self.menu {
            view = stack![
                view,
                menu::dropdown(c, open, self.menu_entries(open), Message::CloseMenu)
            ]
            .into();
        }

        view
    }

    fn menu_entries(&self, open: Menu) -> Vec<menu::Entry<Message>> {
        use menu::{Entry, item};

        let workspace = match &self.screen {
            Screen::Workspace(workspace) => Some(workspace),
            Screen::Splash { .. } => None,
        };
        let has_tab = workspace.is_some_and(|workspace| workspace.active.is_some());
        let when = |enabled: bool, message: Message| enabled.then_some(message);

        match open {
            Menu::File => match workspace {
                Some(_) => vec![
                    item("New Note", Some(Message::NewNote)).shortcut("Ctrl+N"),
                    item("Open File...", Some(Message::OpenFile)).shortcut("Ctrl+O"),
                    Entry::Separator,
                    item("Save", when(has_tab, Message::Save)).shortcut("Ctrl+S"),
                    item("Save As...", when(has_tab, Message::SaveAs)).shortcut("Ctrl+Shift+S"),
                    Entry::Separator,
                    item("Close Tab", when(has_tab, Message::CloseActiveTab)).shortcut("Ctrl+W"),
                    item("Close Campaign", Some(Message::CloseCampaign)),
                    Entry::Separator,
                    item("Exit", Some(Message::Exit)),
                ],
                None => vec![
                    item("New Campaign...", Some(Message::ShowNewCampaign)),
                    item("Open Campaign...", Some(Message::OpenCampaign)),
                    Entry::Separator,
                    item("Exit", Some(Message::Exit)),
                ],
            },
            Menu::Edit => vec![item("Types... (coming soon)", None)],
            Menu::View => {
                let mut entries = vec![Entry::Heading("Theme".into())];
                entries.extend(self.themes.iter().enumerate().map(|(index, theme)| {
                    item(&theme.name, Some(Message::ThemePicked(theme.name.clone())))
                        .checked(index == self.theme)
                }));
                entries.extend([
                    item("Reload Themes", Some(Message::ReloadThemes)),
                    Entry::Separator,
                    item(
                        "Show File Extensions",
                        Some(Message::ToggleExtensions(!self.settings.show_extensions)),
                    )
                    .checked(self.settings.show_extensions),
                    item(
                        "Show Type Files (.yaml)",
                        Some(Message::ToggleTypeFiles(!self.settings.show_type_files)),
                    )
                    .checked(self.settings.show_type_files),
                    Entry::Separator,
                    item(
                        "Refresh File Tree",
                        when(workspace.is_some(), Message::RefreshTree),
                    ),
                ]);
                entries
            }
            Menu::Help => vec![item("About Dungeon Elements", Some(Message::ShowAbout))],
        }
    }
}

impl Workspace {
    fn open(root: PathBuf) -> Self {
        Workspace {
            tree: campaign::scan(&root),
            status: format!("Opened {}", root.display()),
            root,
            expanded: HashSet::new(),
            tabs: Vec::new(),
            active: None,
            next_id: 0,
        }
    }

    fn name(&self) -> String {
        self.root.file_name().map_or_else(
            || self.root.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    /// Shows `path` relative to the campaign folder when it is inside it.
    fn relative(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .display()
            .to_string()
    }

    fn active_tab(&self) -> Option<&Tab> {
        self.tabs.iter().find(|tab| Some(tab.id) == self.active)
    }

    fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| Some(tab.id) == self.active)
    }

    /// Adds a tab and shows it.
    fn add_tab(&mut self, path: Option<PathBuf>, text: &str) {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            content: text_editor::Content::with_text(text),
            path,
            modified: false,
        });
        self.active = Some(id);
    }

    /// Shows `path` in a tab, reading it from disk unless it is already open.
    fn open_file(&mut self, path: PathBuf) {
        if let Some(tab) = self
            .tabs
            .iter()
            .find(|tab| tab.path.as_ref() == Some(&path))
        {
            self.active = Some(tab.id);
            return;
        }

        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.status = format!("Opened {}", self.relative(&path));
                self.add_tab(Some(path), &text);
            }
            Err(error) => self.status = format!("Could not open {}: {error}", path.display()),
        }
    }

    fn remove_tab(&mut self, id: u64) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        self.tabs.remove(index);

        if self.active == Some(id) {
            // Show the tab that slid into this position, or the one before it.
            self.active = self
                .tabs
                .get(index)
                .or_else(|| self.tabs.last())
                .map(|tab| tab.id);
        }
    }

    fn save(&self, id: u64, save_as: bool) -> Task<Message> {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
            return Task::none();
        };

        let path = if save_as { None } else { tab.path.clone() };
        let folder = tab
            .path
            .as_ref()
            .and_then(|path| path.parent())
            .unwrap_or(&self.root)
            .to_path_buf();

        Task::perform(save(path, folder, tab.content.text()), move |result| {
            Message::Saved(id, result)
        })
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleFolder(path) => {
                if !self.expanded.remove(&path) {
                    self.expanded.insert(path);
                }
                Task::none()
            }
            Message::OpenEntry(path) => {
                self.open_file(path);
                Task::none()
            }
            Message::RefreshTree => {
                self.tree = campaign::scan(&self.root);
                Task::none()
            }
            Message::SelectTab(id) => {
                self.active = Some(id);
                Task::none()
            }
            Message::CloseTab(id) => {
                let modified = self.tabs.iter().any(|tab| tab.id == id && tab.modified);

                if modified {
                    Task::perform(
                        confirm_discard("This file has unsaved changes. Close it anyway?"),
                        move |yes| Message::CloseTabConfirmed(id, yes),
                    )
                } else {
                    self.remove_tab(id);
                    Task::none()
                }
            }
            Message::CloseTabConfirmed(id, true) => {
                self.remove_tab(id);
                Task::none()
            }
            Message::CloseActiveTab => match self.active {
                Some(id) => self.update(Message::CloseTab(id)),
                None => Task::none(),
            },
            Message::CloseCampaign => {
                if self.tabs.iter().any(|tab| tab.modified) {
                    Task::perform(
                        confirm_discard(
                            "Some files have unsaved changes. Close the campaign anyway?",
                        ),
                        Message::CloseCampaignConfirmed,
                    )
                } else {
                    Task::done(Message::CloseCampaignConfirmed(true))
                }
            }
            Message::Edit(action) => {
                if let Some(tab) = self.active_tab_mut() {
                    tab.modified |= action.is_edit();
                    tab.content.perform(action);
                }
                Task::none()
            }
            Message::NewNote => {
                self.add_tab(None, "");
                Task::none()
            }
            Message::OpenFile => {
                Task::perform(pick_markdown(self.root.clone()), Message::FilePicked)
            }
            Message::FilePicked(Some(path)) => {
                self.open_file(path);
                Task::none()
            }
            Message::Save => match self.active {
                Some(id) => self.save(id, false),
                None => Task::none(),
            },
            Message::SaveAs => match self.active {
                Some(id) => self.save(id, true),
                None => Task::none(),
            },
            Message::Saved(id, Ok(Some(path))) => {
                self.status = format!("Saved {}", self.relative(&path));
                if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                    tab.path = Some(path);
                    tab.modified = false;
                }
                self.tree = campaign::scan(&self.root);
                Task::none()
            }
            Message::Saved(_, Err(error)) => {
                self.status = format!("Could not save: {error}");
                Task::none()
            }
            // Dialogs the user cancelled, and splash screen messages.
            _ => Task::none(),
        }
    }

    fn view<'a>(&'a self, c: Colors, settings: &Settings) -> Element<'a, Message> {
        let main = column![self.tab_bar(c, settings), self.editor(c)];

        let body = row![
            self.sidebar(c, settings),
            rule::vertical(c.border_width.max(1.0)).style(style::separator(c)),
            main
        ]
        .height(Fill);

        let status = container(
            row![
                text(self.name()).size(12),
                space::horizontal(),
                text(&self.status).size(12).wrapping(text::Wrapping::None),
            ]
            .spacing(16),
        )
        .padding([3, 10])
        .width(Fill)
        .clip(true)
        .style(style::status_bar(c));

        column![body, status].into()
    }

    fn sidebar<'a>(&'a self, c: Colors, settings: &Settings) -> Element<'a, Message> {
        let header = row![
            text(self.name().to_uppercase())
                .size(11)
                .color(c.text_muted),
            space::horizontal(),
            button(text("⟳").size(13))
                .padding([0, 6])
                .style(style::flat(c, false))
                .on_press(Message::RefreshTree),
        ]
        .align_y(Center)
        .padding([6, 12]);

        let tree: Element<'_, Message> = if self.tree.is_empty() {
            text("This campaign has no folders or Markdown files yet.")
                .size(13)
                .color(c.text_muted)
                .into()
        } else {
            let mut rows = Vec::new();
            self.tree_rows(c, settings, &self.tree, 0, &mut rows);
            column(rows).into()
        };

        container(column![
            header,
            scrollable(container(tree).padding([0, 4])).height(Fill)
        ])
        .width(260)
        .height(Fill)
        .style(style::panel(c.sidebar))
        .into()
    }

    /// Adds a row for each entry, and for the contents of expanded folders.
    fn tree_rows<'a>(
        &'a self,
        c: Colors,
        settings: &Settings,
        entries: &'a [Entry],
        depth: u16,
        rows: &mut Vec<Element<'a, Message>>,
    ) {
        let active_path = self.active_tab().and_then(|tab| tab.path.as_ref());

        for entry in entries {
            if entry.children.is_none()
                && !settings.show_type_files
                && campaign::is_yaml(&entry.path)
            {
                continue;
            }

            let name = match entry.children {
                Some(_) => entry.name.clone(),
                None => display_name(&entry.path, settings.show_extensions),
            };

            let indent = space().width(f32::from(8 + depth * 14));
            let (arrow, message, selected) = match &entry.children {
                Some(_) if self.expanded.contains(&entry.path) => {
                    ("▾", Message::ToggleFolder(entry.path.clone()), false)
                }
                Some(_) => ("▸", Message::ToggleFolder(entry.path.clone()), false),
                None => (
                    "",
                    Message::OpenEntry(entry.path.clone()),
                    active_path == Some(&entry.path),
                ),
            };

            let row = button(row![
                indent,
                text(arrow).size(13).width(14),
                text(name).size(13)
            ])
            .width(Fill)
            .padding([2, 0])
            .style(style::flat(c, selected))
            .on_press(message);

            rows.push(row.into());

            if let Some(children) = &entry.children
                && self.expanded.contains(&entry.path)
            {
                self.tree_rows(c, settings, children, depth + 1, rows);
            }
        }
    }

    fn tab_bar(&self, c: Colors, settings: &Settings) -> Element<'_, Message> {
        let tabs = self.tabs.iter().map(|tab| {
            let active = Some(tab.id) == self.active;
            let close_label = if tab.modified { "●" } else { "×" };

            container(
                row![
                    button(text(tab.name(settings.show_extensions)).size(13))
                        .padding([8, 4])
                        .style(style::tab_button(c, active))
                        .on_press(Message::SelectTab(tab.id)),
                    button(text(close_label).size(13))
                        .padding([8, 4])
                        .style(style::tab_button(c, active))
                        .on_press(Message::CloseTab(tab.id)),
                ]
                .align_y(Center),
            )
            .padding([0, 6])
            .style(style::tab(c, active))
            .into()
        });

        container(
            scrollable(row(tabs).spacing(1)).direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(3).scroller_width(3),
            )),
        )
        .width(Fill)
        .style(style::panel(c.sidebar))
        .into()
    }

    fn editor(&self, c: Colors) -> Element<'_, Message> {
        let Some(tab) = self.active_tab() else {
            return center(
                column![
                    text("No file open").size(20).color(c.text_muted),
                    text("Pick a file in the sidebar, or press Ctrl+N for a new note.")
                        .size(14)
                        .color(c.text_muted),
                ]
                .spacing(8)
                .align_x(Center),
            )
            .style(style::panel(c.background))
            .into();
        };

        text_editor(&tab.content)
            .placeholder("Start writing...")
            .on_action(Message::Edit)
            .key_binding(|key_press| {
                if key_press.modifiers.command()
                    && let Some(message) = shortcut(&key_press.key, key_press.modifiers)
                {
                    return Some(text_editor::Binding::Custom(message));
                }
                text_editor::Binding::from_key_press(key_press)
            })
            .padding(16)
            .height(Fill)
            .style(style::editor(c))
            .into()
    }
}

/// Maps the Escape key, which closes menus and dialogs.
fn escape(key: &Key) -> Option<Message> {
    matches!(key, Key::Named(keyboard::key::Named::Escape)).then_some(Message::Escape)
}

/// Sums up the theme files that could not be loaded, if any.
fn theme_problems(problems: &[String]) -> Option<String> {
    (!problems.is_empty())
        .then(|| format!("Some themes could not be loaded. {}", problems.join("; ")))
}

/// The logo in the theme's colors: black lines become the text color, and white fills become `background`.
fn logo(c: Colors, background: iced::Color) -> svg::Handle {
    let svg = LOGO
        .replace("#000000", &theme::to_hex(c.text))
        .replace("#ffffff", &theme::to_hex(background));

    svg::Handle::from_memory(svg.into_bytes())
}

/// Maps the workspace keyboard shortcuts to their messages.
fn shortcut(key: &Key, modifiers: Modifiers) -> Option<Message> {
    if !modifiers.command() {
        return None;
    }

    let Key::Character(c) = key.as_ref() else {
        return None;
    };

    match c.to_ascii_lowercase().as_str() {
        "n" => Some(Message::NewNote),
        "o" => Some(Message::OpenFile),
        "s" if modifiers.shift() => Some(Message::SaveAs),
        "s" => Some(Message::Save),
        "w" => Some(Message::CloseActiveTab),
        _ => None,
    }
}

fn splash<'a>(c: Colors, logo: &svg::Handle, error: Option<&'a str>) -> Element<'a, Message> {
    let big_button = |label| {
        button(text(label).size(16).width(Fill).align_x(Center))
            .padding([12, 24])
            .width(240)
    };

    let mut content = column![
        Svg::new(logo.clone()).width(180).height(180),
        text("Dungeon Elements").size(44).color(c.text),
        text("A local-first campaign manager for tabletop RPGs")
            .size(16)
            .color(c.text_muted),
        space().height(24),
        big_button("Open Campaign...")
            .style(style::primary(c))
            .on_press(Message::OpenCampaign),
        big_button("New Campaign...")
            .style(style::secondary(c))
            .on_press(Message::ShowNewCampaign),
    ]
    .spacing(12)
    .align_x(Center);

    if let Some(error) = error {
        content = content.push(text(error).color(c.danger).width(560).align_x(Center));
    }

    center(content).style(style::panel(c.background)).into()
}

/// The "About" dialog, opened from Help > About.
fn about_dialog<'a>(c: Colors, logo: &svg::Handle) -> Element<'a, Message> {
    let themes_folder = theme::user_folder().map_or_else(
        || "Your config folder could not be found.".into(),
        |folder| format!("Your themes folder: {}", folder.display()),
    );

    let content = column![
        Svg::new(logo.clone()).width(96).height(96),
        text("Dungeon Elements").size(24).color(c.text),
        text(format!("Version {}", env!("CARGO_PKG_VERSION")))
            .size(13)
            .color(c.text_muted),
        text("A local-first campaign manager for tabletop RPGs. Your campaigns are plain Markdown files that stay on your drive.")
            .size(13)
            .color(c.text)
            .align_x(Center),
        text(themes_folder).size(12).color(c.text_muted).align_x(Center),
        button("Close")
            .style(style::primary(c))
            .on_press(Message::CloseAbout),
    ]
    .spacing(10)
    .align_x(Center);

    container(content)
        .width(420)
        .padding(24)
        .style(style::dialog(c))
        .into()
}

fn new_campaign_dialog(c: Colors, form: &NewCampaign) -> Element<'_, Message> {
    let label = |label| text(label).size(13).color(c.text_muted);

    let folder_preview = if form.title.trim().is_empty() {
        String::new()
    } else {
        format!(
            "Creates {}",
            Path::new(form.location.trim())
                .join(form.title.trim())
                .display()
        )
    };

    let mut content = column![
        text("New campaign").size(22).color(c.text),
        column![
            label("Title"),
            text_input("The Sunken Keep", &form.title)
                .id(TITLE_INPUT)
                .on_input(Message::TitleChanged)
                .on_submit(Message::CreateCampaign)
                .padding(8)
                .style(style::input(c)),
        ]
        .spacing(4),
        column![
            label("Location"),
            row![
                text_input("Folder to save the campaign in", &form.location)
                    .on_input(Message::LocationChanged)
                    .on_submit(Message::CreateCampaign)
                    .padding(8)
                    .style(style::input(c)),
                button("Browse...")
                    .style(style::secondary(c))
                    .padding(8)
                    .on_press(Message::BrowseLocation),
            ]
            .spacing(8),
            text(folder_preview).size(12).color(c.text_muted),
        ]
        .spacing(4),
        column![
            checkbox(form.with_template)
                .label("Set up the starter folder structure")
                .on_toggle(Message::TemplateToggled)
                .style(style::check(c)),
            text(format!(
                "Adds the folders: {}",
                campaign::TEMPLATE_FOLDERS.join(", ")
            ))
            .size(12)
            .color(c.text_muted),
        ]
        .spacing(4),
    ]
    .spacing(16);

    if let Some(error) = &form.error {
        content = content.push(text(error).size(13).color(c.danger));
    }

    content = content.push(
        row![
            space::horizontal(),
            button("Cancel")
                .style(style::secondary(c))
                .on_press(Message::CancelNewCampaign),
            button("Create")
                .style(style::primary(c))
                .on_press(Message::CreateCampaign),
        ]
        .spacing(8),
    );

    container(content)
        .width(480)
        .padding(24)
        .style(style::dialog(c))
        .into()
}

/// Shows `dialog` centered over a dimmed `base`, blocking clicks to `base`.
fn modal<'a>(base: Element<'a, Message>, dialog: Element<'a, Message>) -> Element<'a, Message> {
    stack![base, opaque(center(opaque(dialog)).style(style::backdrop))].into()
}

/// The folder the "New campaign" dialog suggests: the user's Documents folder, or their home folder.
fn default_location() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)?;

    let documents = home.join("Documents");
    Some(if documents.is_dir() { documents } else { home })
}

async fn pick_folder(title: &'static str) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(title)
        .pick_folder()
        .await
        .map(|handle| handle.path().to_path_buf())
}

async fn pick_markdown(folder: PathBuf) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Open file")
        .add_filter("Markdown", &["md"])
        .add_filter("Type files (YAML)", &["yaml", "yml"])
        .set_directory(folder)
        .pick_file()
        .await
        .map(|handle| handle.path().to_path_buf())
}

/// Asks a yes/no question about discarding unsaved changes. Returns true for yes.
async fn confirm_discard(description: &'static str) -> bool {
    rfd::AsyncMessageDialog::new()
        .set_title("Unsaved changes")
        .set_description(description)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        .await
        == rfd::MessageDialogResult::Yes
}

/// Writes `text` to `path`, asking the user where to save first (starting in `folder`) if there is no path yet.
///
/// Returns `Ok(None)` if the user cancels the save dialog.
async fn save(
    path: Option<PathBuf>,
    folder: PathBuf,
    text: String,
) -> Result<Option<PathBuf>, String> {
    let path = match path {
        Some(path) => path,
        None => {
            let picked = rfd::AsyncFileDialog::new()
                .set_title("Save as Markdown")
                .add_filter("Markdown", &["md"])
                .add_filter("Type files (YAML)", &["yaml", "yml"])
                .set_directory(folder)
                .set_file_name("untitled.md")
                .save_file()
                .await;

            match picked {
                Some(handle) => with_md_extension(handle.path().to_path_buf()),
                None => return Ok(None),
            }
        }
    };

    std::fs::write(&path, text).map_err(|error| error.to_string())?;

    Ok(Some(path))
}

/// Appends `.md` to `path` unless it already ends in it, so notes are always Markdown.
///
/// YAML files, like a folder's `_type.yaml`, keep their extension.
fn with_md_extension(mut path: PathBuf) -> PathBuf {
    if !campaign::is_markdown(&path) && !campaign::is_yaml(&path) {
        path.as_mut_os_string().push(".md");
    }

    path
}
