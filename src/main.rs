//! Dungeon Elements: a local-first campaign manager for tabletop RPGs.

// Release builds on Windows open without a console window behind the app.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod campaign;
mod check;
mod form;
mod icon;
mod links;
mod menu;
mod note;
mod settings;
mod style;
mod theme;
mod types;
mod types_editor;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{
    Svg, button, center, checkbox, column, container, mouse_area, opaque, pick_list, row, rule,
    scrollable, space, stack, svg, text, text_editor, text_input,
};
use iced::{Center, Element, Fill, Subscription, Task, window};

use campaign::Entry;
use check::Problem;
use form::Form;
use menu::Menu;
use settings::Settings;
use theme::{Colors, Theme};
use types::ContentType;

/// The logo, drawn in black on white. [`logo`] recolors it to match the theme.
const LOGO: &str = include_str!("../reference/dungeon_elements.svg");

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .window(window_settings())
        // Closing the window asks about unsaved changes first; see `Message::Exit`.
        .exit_on_close_request(false)
        .run()
}

fn window_settings() -> window::Settings {
    #[allow(unused_mut)]
    let mut settings = window::Settings {
        size: iced::Size::new(1200.0, 800.0),
        icon: icon::window_icon(),
        ..window::Settings::default()
    };
    // Matches `dungeon_elements.desktop`, so Linux desktops show the right icon in the dock.
    #[cfg(target_os = "linux")]
    {
        settings.platform_specific.application_id = "dungeon_elements".into();
    }
    settings
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
    Workspace(Box<Workspace>),
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
/// The widget id of the name field in the workspace's dialogs.
const DIALOG_INPUT: &str = "dialog-name";
/// The widget id of the text editor that shows a file's text.
const EDITOR: &str = "editor";
/// How many notes the link bar suggests at most.
const SUGGESTIONS: usize = 8;

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
    /// The content type of each folder that has a `_type.yaml`, or what is wrong with it.
    types: HashMap<PathBuf, Result<ContentType, String>>,
    /// What is wrong with the campaign's files, checked when it opens and is refreshed.
    problems: Vec<Problem>,
    /// The folders in the tree that are expanded.
    expanded: HashSet<PathBuf>,
    tabs: Vec<Tab>,
    /// The `id` of the tab shown in the editor.
    active: Option<u64>,
    next_id: u64,
    status: String,
    /// The dialog open over the workspace, if any.
    dialog: Option<Dialog>,
    /// The campaign's notes, for following and rewriting `[[links]]`.
    index: links::Index,
    /// Whether Ctrl (Cmd on macOS) is held, so a click on a link opens it.
    ctrl: bool,
    /// The row of the file tree under the mouse: a file, a folder, or the campaign
    /// folder itself for the space around the tree.
    hovered: Option<PathBuf>,
    /// The note being dragged in the file tree, and whether it has left its row yet.
    drag: Option<(PathBuf, bool)>,
}

/// A dialog over the workspace.
enum Dialog {
    /// Adds a field to the note in the active tab only.
    AddField {
        label: String,
        kind: types::Kind,
        error: Option<String>,
    },
    /// Makes a folder at the top of the campaign, optionally with a content type.
    NewFolder {
        name: String,
        template: Template,
        error: Option<String>,
    },
    /// Edit > Types: changes the fields of every folder's type.
    Types(types_editor::Editor),
    /// Lists the campaign's files with formatting problems.
    Problems,
}

/// What the link bar offers while a link is being typed.
#[derive(Default)]
struct Suggestions {
    /// Notes the link could be, as `(link text, label)`.
    notes: Vec<(String, String)>,
    /// Whether the link is typed in a link field rather than a text editor.
    in_field: bool,
    /// For a link field that links to a folder: the field's index, and the label of a
    /// button that makes the note it names there.
    create: Option<(usize, String)>,
}

/// A content type a new folder can start with.
#[derive(Debug, Clone, PartialEq)]
struct Template {
    label: String,
    /// The `_type.yaml` to write, or `None` for a folder of plain notes.
    yaml: Option<String>,
}

impl std::fmt::Display for Template {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

impl Template {
    fn plain() -> Self {
        Template {
            label: "Plain notes (no fields)".into(),
            yaml: None,
        }
    }
}

struct Tab {
    id: u64,
    /// The file's text, edited directly when there is no form or the source is showing.
    content: text_editor::Content,
    /// The form for a note in a typed folder.
    form: Option<Form>,
    /// Whether the Markdown source shows instead of the form.
    source: bool,
    /// The file the text was last saved to, if any.
    path: Option<PathBuf>,
    /// The folder a new note is saved in.
    folder: PathBuf,
    /// Whether the text has changed since it was last saved.
    modified: bool,
    /// When the file was last changed on disk as of reading or saving it.
    disk: Option<SystemTime>,
    /// Whether the file changed outside the app while this tab had unsaved changes.
    changed_outside: bool,
}

impl Tab {
    fn name(&self, show_extensions: bool) -> String {
        match (&self.path, &self.form) {
            (Some(path), _) => display_name(path, show_extensions),
            (None, Some(form)) => format!("New {}", form.ty.label()),
            (None, None) => "Untitled".into(),
        }
    }

    /// Whether the form shows, rather than the Markdown source.
    fn shows_form(&self) -> bool {
        self.form.is_some() && !self.source
    }

    /// The text to save.
    fn text(&self) -> Result<String, String> {
        match &self.form {
            Some(form) if !self.source => form.to_markdown(),
            _ => Ok(self.content.text()),
        }
    }

    /// Whether the tab holds a note, rather than a `_type.yaml`, so links work in it.
    fn is_note(&self) -> bool {
        self.path.as_deref().is_none_or(campaign::is_markdown)
    }

    /// The note's path, or where it will be saved, for resolving its links.
    fn note_path(&self) -> PathBuf {
        self.path
            .clone()
            .unwrap_or_else(|| self.folder.join("Untitled.md"))
    }

    /// The note's name from its `name` field, read from the form or the source.
    fn note_name(&self) -> Option<String> {
        let form = self.form.as_ref()?;
        if self.source {
            let note = note::parse(&self.content.text()).ok()?;
            Form::from_note(form.ty.clone(), note).name()
        } else {
            form.name()
        }
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
    NewNoteIn(PathBuf),
    Form(form::Message),
    ToggleSource,
    Modifiers(Modifiers),
    /// The mouse moved onto a row of the file tree.
    TreeEnter(PathBuf),
    TreeExit(PathBuf),
    MousePressed,
    MouseReleased,
    /// Finishes the link being typed with this link text.
    CompleteLink(String),
    /// Opens the note a link points to, creating it if there is none.
    FollowLink(String),
    InsertLink,
    ShowAddField,
    AddFieldLabel(String),
    AddFieldKind(types::Kind),
    AddField,
    RestoreField(String),
    ShowNewFolder,
    NewFolderName(String),
    NewFolderTemplate(Template),
    CreateFolder,
    CloseDialog,
    ShowTypes,
    Types(types_editor::Message),
    ShowProblems,
    OpenProblem(PathBuf),
    WindowFocused,
    ReloadFromDisk,
    KeepMine,
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

        // `dungeon_elements <campaign folder>` opens that campaign right away.
        if let Some(root) = std::env::args_os().nth(1).map(PathBuf::from)
            && root.is_dir()
        {
            app.screen = Screen::Workspace(Box::new(Workspace::open(root)));
        }

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
                keyboard::Event::ModifiersChanged(modifiers) => Some(Message::Modifiers(modifiers)),
                _ => None,
            }),
        };

        // Coming back to the window is when files may have been edited in another app.
        // Mouse buttons are watched too, for dragging notes in the file tree.
        let focus = iced::event::listen_with(|event, _, _| match event {
            iced::Event::Window(window::Event::Focused) => Some(Message::WindowFocused),
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                Some(Message::MousePressed)
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                Some(Message::MouseReleased)
            }
            _ => None,
        });

        Subscription::batch([keys, focus, window::close_requests().map(|_| Message::Exit)])
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
                match &mut self.screen {
                    Screen::Splash { new_campaign, .. } if !self.about => *new_campaign = None,
                    Screen::Workspace(workspace) if !self.about => workspace.dialog = None,
                    _ => {}
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
                        self.screen = Screen::Workspace(Box::new(Workspace::open(root)));
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
                        Ok(root) => {
                            self.screen = Screen::Workspace(Box::new(Workspace::open(root)))
                        }
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

        if let Screen::Workspace(workspace) = &self.screen
            && let Some(dialog) = &workspace.dialog
        {
            view = modal(view, workspace.dialog_view(c, dialog));
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
            Screen::Workspace(workspace) => Some(workspace.as_ref()),
            Screen::Splash { .. } => None,
        };
        let has_tab = workspace.is_some_and(|workspace| workspace.active.is_some());
        let active_tab = workspace.and_then(Workspace::active_tab);
        let has_form = active_tab.is_some_and(|tab| tab.form.is_some());
        let can_link = active_tab.is_some_and(|tab| {
            tab.is_note()
                && (!tab.shows_form()
                    || tab.form.as_ref().is_some_and(|form| form.focus().is_some()))
        });
        let shows_source = active_tab.is_some_and(|tab| tab.form.is_some() && !tab.shows_form());
        let when = |enabled: bool, message: Message| enabled.then_some(message);

        match open {
            Menu::File => match workspace {
                Some(_) => vec![
                    item("New Note", Some(Message::NewNote)).shortcut("Ctrl+N"),
                    item("New Folder...", Some(Message::ShowNewFolder)),
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
            Menu::Edit => vec![
                item("Insert Link", when(can_link, Message::InsertLink)).shortcut("Ctrl+K"),
                Entry::Separator,
                item(
                    "Add Field to This Note...",
                    when(
                        active_tab.is_some_and(Tab::shows_form),
                        Message::ShowAddField,
                    ),
                ),
                Entry::Separator,
                item("Types...", Some(Message::ShowTypes)),
            ],
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
                    item(
                        "Show Markdown Source",
                        when(has_form, Message::ToggleSource),
                    )
                    .shortcut("Ctrl+E")
                    .checked(shows_source),
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
        let tree = campaign::scan(&root);
        let types = load_types(&root, &tree);
        let problems = check::campaign(&root, &tree, &types);
        let index = links::Index::new(&root, markdown_files(&tree));
        let status = match problems_summary(&problems) {
            Some(summary) => format!("Opened {}. {summary}", root.display()),
            None => format!("Opened {}", root.display()),
        };
        Workspace {
            types,
            problems,
            tree,
            status,
            root,
            expanded: HashSet::new(),
            tabs: Vec::new(),
            active: None,
            next_id: 0,
            dialog: None,
            index,
            ctrl: false,
            hovered: None,
            drag: None,
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

    /// Reads the file tree and the folders' content types again, and checks the files.
    fn rescan(&mut self) {
        self.tree = campaign::scan(&self.root);
        self.types = load_types(&self.root, &self.tree);
        self.problems = check::campaign(&self.root, &self.tree, &self.types);
        self.index = links::Index::new(&self.root, markdown_files(&self.tree));
    }

    /// The problems found in the file at `path`.
    fn problems_in<'a>(&'a self, path: &'a Path) -> impl Iterator<Item = &'a Problem> + 'a {
        self.problems
            .iter()
            .filter(move |problem| problem.path == path)
    }

    /// Looks for open files that changed on disk since they were read, for example in Obsidian.
    ///
    /// A tab without unsaved changes is reloaded. One with unsaved changes asks which version to keep.
    fn check_disk(&mut self) {
        let mut messages = Vec::new();

        for index in 0..self.tabs.len() {
            let tab = &self.tabs[index];
            let Some(path) = tab.path.clone() else {
                continue;
            };
            let now = check::modified_time(&path);
            if now == tab.disk || tab.changed_outside {
                continue;
            }
            let name = self.relative(&path);

            if now.is_none() {
                let tab = &mut self.tabs[index];
                tab.disk = None;
                tab.modified = true;
                messages.push(format!(
                    "{name} was deleted outside Dungeon Elements; save to keep it"
                ));
            } else if tab.modified {
                self.tabs[index].changed_outside = true;
                messages.push(format!(
                    "{name} was changed outside Dungeon Elements while you were editing it"
                ));
            } else {
                messages.push(match self.reload_tab(index) {
                    Ok(()) => format!("Reloaded {name}: it was changed outside Dungeon Elements"),
                    Err(problem) => problem,
                });
            }
        }

        if !messages.is_empty() {
            self.status = messages.join(". ");
        }
    }

    /// Reads the file of the tab at `index` again, dropping any unsaved changes.
    fn reload_tab(&mut self, index: usize) -> Result<(), String> {
        let Some(path) = self.tabs[index].path.clone() else {
            return Ok(());
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("Could not read {}: {error}", self.relative(&path)))?;
        let form = if campaign::is_markdown(&path) {
            self.form_for(&path, &text)
        } else {
            Ok(None)
        };

        let tab = &mut self.tabs[index];
        tab.content = text_editor::Content::with_text(&text);
        tab.disk = check::modified_time(&path);
        tab.modified = false;
        tab.changed_outside = false;
        match form {
            Ok(form) => {
                tab.form = form;
                Ok(())
            }
            // The note can't be read as a form any more: show its source.
            Err(problem) => {
                tab.form = None;
                Err(problem)
            }
        }
    }

    /// Rebuilds the form of every open note without unsaved changes, so it shows its new type.
    ///
    /// Notes with unsaved changes keep the old fields until they are reopened.
    fn reload_forms(&mut self) {
        for index in 0..self.tabs.len() {
            let tab = &self.tabs[index];
            let (Some(path), false) = (tab.path.clone(), tab.modified) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Ok(form) = self.form_for(&path, &text) {
                let tab = &mut self.tabs[index];
                tab.content = text_editor::Content::with_text(&text);
                tab.form = form;
                tab.disk = check::modified_time(&path);
            }
        }
    }

    /// Adds a tab and shows it.
    fn add_tab(&mut self, path: Option<PathBuf>, folder: PathBuf, text: &str, form: Option<Form>) {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            content: text_editor::Content::with_text(text),
            form,
            source: false,
            disk: path.as_deref().and_then(check::modified_time),
            path,
            folder,
            modified: false,
            changed_outside: false,
        });
        self.active = Some(id);
    }

    /// The content type for a note: the one its `type:` names, or else its folder's.
    ///
    /// Returns a message instead if the note or the type file can't be read.
    fn form_for(&self, path: &Path, text: &str) -> Result<Option<Form>, String> {
        let folder = path.parent().unwrap_or(&self.root);
        let folder_type = self.types.get(folder);

        let note = match note::parse(text) {
            Ok(note) => note,
            Err(error) if folder_type.is_some() => {
                return Err(format!(
                    "{}: {error}. Showing the Markdown source.",
                    self.relative(path)
                ));
            }
            Err(_) => return Ok(None),
        };

        let named = note.front.get("type").and_then(|id| {
            let id = note::value_to_text(id);
            self.types.values().flatten().find(|ty| ty.id == id)
        });

        let ty = match (named, folder_type) {
            (Some(ty), _) | (None, Some(Ok(ty))) => ty.clone(),
            (None, Some(Err(error))) => {
                return Err(format!(
                    "{} has a problem: {error}",
                    self.relative(&folder.join(types::TYPE_FILE))
                ));
            }
            (None, None) => return Ok(None),
        };

        Ok(Some(Form::from_note(ty, note)))
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
                let form = if campaign::is_markdown(&path) {
                    self.form_for(&path, &text)
                } else {
                    Ok(None)
                };

                let problems: Vec<&str> = self
                    .problems_in(&path)
                    .map(|problem| problem.message.as_str())
                    .collect();
                self.status = match (&form, problems.as_slice()) {
                    (Err(problem), _) => problem.clone(),
                    (Ok(_), []) => format!("Opened {}", self.relative(&path)),
                    (Ok(_), [problem]) => {
                        format!("Opened {}. Problem: {problem}", self.relative(&path))
                    }
                    (Ok(_), [problem, rest @ ..]) => format!(
                        "Opened {}. Problem: {problem} (and {} more in Problems)",
                        self.relative(&path),
                        rest.len()
                    ),
                };

                let folder = path.parent().unwrap_or(&self.root).to_path_buf();
                self.add_tab(Some(path), folder, &text, form.ok().flatten());
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

    fn save(&mut self, id: u64, save_as: bool) -> Task<Message> {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
            return Task::none();
        };

        let text = match tab.text() {
            Ok(text) => text,
            Err(error) => {
                self.status = format!("Could not save: {error}");
                return Task::none();
            }
        };

        // Don't quietly overwrite edits made in another app since the file was read.
        if !save_as
            && let Some(path) = &tab.path
            && (tab.changed_outside || check::modified_time(path) != tab.disk)
            && path.exists()
        {
            let name = self.relative(path);
            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                tab.changed_outside = true;
            }
            self.status = format!(
                "Not saved: {name} was changed outside Dungeon Elements. Choose Reload or Keep Mine above the editor first"
            );
            return Task::none();
        }

        // A note with a name field is saved as "<name>.md" in its folder, without asking.
        if !save_as
            && let Some(form) = &tab.form
            && form.ty.name_field().is_some()
        {
            // Applied right away, so a second Ctrl+S before then can't save a new note twice.
            let old = tab.path.clone();
            let result = save_named(tab, &text, &form.ty.label());
            let renamed = match (old, &result) {
                (Some(old), Ok(new)) if old != *new => Some((old, new.clone())),
                _ => None,
            };
            let task = self.update(Message::Saved(id, result.map(Some)));
            if let Some((old, new)) = renamed {
                let done = format!(
                    "Renamed {} to {}",
                    display_name(&old, false),
                    display_name(&new, false)
                );
                self.relink(&old, &new, &done);
            }
            return task;
        }

        let path = if save_as { None } else { tab.path.clone() };
        let folder = tab
            .path
            .as_ref()
            .and_then(|path| path.parent())
            .unwrap_or(&tab.folder)
            .to_path_buf();

        Task::perform(save(path, folder, text), move |result| {
            Message::Saved(id, result)
        })
    }

    /// The types a new folder can start with: none, the default template's, and the campaign's own.
    fn templates(&self) -> Vec<Template> {
        let mut templates = vec![Template::plain()];

        let starters = types::STARTER_TYPES.iter().filter_map(|(_, yaml)| {
            let ty = types::parse(yaml).ok()?;
            Some((ty.id.clone(), ty.label(), yaml.to_string()))
        });

        let mut folders: Vec<_> = self.types.keys().collect();
        folders.sort();
        let campaign = folders.into_iter().filter_map(|folder| {
            let ty = self.types.get(folder)?.as_ref().ok()?;
            let yaml = std::fs::read_to_string(folder.join(types::TYPE_FILE)).ok()?;
            Some((ty.id.clone(), ty.label(), yaml))
        });

        // The campaign's own version of a type wins over the default one with the same id.
        let mut seen: Vec<(String, Template)> = Vec::new();
        for (id, label, yaml) in campaign.chain(starters) {
            if seen.iter().all(|(other, _)| *other != id) {
                seen.push((
                    id,
                    Template {
                        label,
                        yaml: Some(yaml),
                    },
                ));
            }
        }
        seen.sort_by(|a, b| a.1.label.cmp(&b.1.label));
        templates.extend(seen.into_iter().map(|(_, template)| template));
        templates
    }

    /// Buttons to bring back the type's fields that were removed from the note in the active tab.
    fn removed_fields<'a>(&self, c: Colors) -> Element<'a, Message> {
        let removed = self
            .active_tab()
            .and_then(|tab| tab.form.as_ref())
            .map(Form::removed_labels)
            .unwrap_or_default();

        if removed.is_empty() {
            return space().height(0).into();
        }

        let buttons = removed.into_iter().map(|label| {
            button(text(label.clone()).size(13))
                .style(style::secondary(c))
                .on_press(Message::RestoreField(label))
                .into()
        });

        column![
            text("Bring back a field this note doesn't use")
                .size(13)
                .color(c.text_muted),
            row(buttons).spacing(6).wrap(),
        ]
        .spacing(6)
        .into()
    }

    fn dialog_view<'a>(&'a self, c: Colors, dialog: &'a Dialog) -> Element<'a, Message> {
        if let Dialog::Types(editor) = dialog {
            return editor.view(c).map(Message::Types);
        }
        if let Dialog::Problems = dialog {
            return self.problems_dialog(c);
        }

        let label = |label| text(label).size(13).color(c.text_muted);

        let (title, fields, error, confirm, on_confirm): (_, Element<'a, Message>, _, _, _) =
            match dialog {
                Dialog::AddField { label: name, kind, error } => (
                    "Add a field to this note",
                    column![
                        self.removed_fields(c),
                        column![
                            label("Name"),
                            text_input("Durability", name)
                                .id(DIALOG_INPUT)
                                .on_input(Message::AddFieldLabel)
                                .on_submit(Message::AddField)
                                .padding(8)
                                .style(style::input(c)),
                        ]
                        .spacing(4),
                        column![
                            label("Kind"),
                            pick_list(types::Kind::FOR_NOTE, Some(*kind), Message::AddFieldKind)
                                .padding(8)
                                .width(Fill)
                                .style(style::choice(c))
                                .menu_style(style::choice_menu(c)),
                        ]
                        .spacing(4),
                        text("Only this note gets the field. To give every note in the folder a field, edit the folder's type. Any field can be removed from a note with its Remove button.")
                            .size(12)
                            .color(c.text_muted),
                    ]
                    .spacing(16)
                    .into(),
                    error,
                    "Add",
                    Message::AddField,
                ),
                Dialog::NewFolder {
                    name,
                    template,
                    error,
                } => (
                    "New folder",
                    column![
                        column![
                            label("Name"),
                            text_input("factions", name)
                                .id(DIALOG_INPUT)
                                .on_input(Message::NewFolderName)
                                .on_submit(Message::CreateFolder)
                                .padding(8)
                                .style(style::input(c)),
                        ]
                        .spacing(4),
                        column![
                            label("Notes in this folder"),
                            pick_list(
                                self.templates(),
                                Some(template.clone()),
                                Message::NewFolderTemplate
                            )
                            .padding(8)
                            .width(Fill)
                            .style(style::choice(c))
                            .menu_style(style::choice_menu(c)),
                            text("Picking a type gives every note in the folder its fields. You can change them later in Edit > Types.")
                                .size(12)
                                .color(c.text_muted),
                        ]
                        .spacing(4),
                    ]
                    .spacing(16)
                    .into(),
                    error,
                    "Create",
                    Message::CreateFolder,
                ),
                Dialog::Types(_) | Dialog::Problems => {
                    unreachable!("these dialogs have their own view")
                }
            };

        let mut content = column![text(title).size(22).color(c.text), fields].spacing(16);

        if let Some(error) = error {
            content = content.push(text(error).size(13).color(c.danger));
        }

        content = content.push(
            row![
                space::horizontal(),
                button("Cancel")
                    .style(style::secondary(c))
                    .on_press(Message::CloseDialog),
                button(confirm)
                    .style(style::primary(c))
                    .on_press(on_confirm),
            ]
            .spacing(8),
        );

        container(content)
            .width(440)
            .padding(24)
            .style(style::dialog(c))
            .into()
    }

    /// Lists the files with problems; clicking one opens it.
    fn problems_dialog(&self, c: Colors) -> Element<'_, Message> {
        let rows = self.problems.iter().map(|problem| {
            column![
                button(text(self.relative(&problem.path)).size(13).color(c.accent))
                    .padding(0)
                    .style(style::flat(c, false))
                    .on_press(Message::OpenProblem(problem.path.clone())),
                text(&problem.message).size(13).color(c.text),
            ]
            .spacing(2)
            .into()
        });

        let content = column![
            text("Problems").size(22).color(c.text),
            text("These files were probably edited outside Dungeon Elements, and parts of them can't be shown as intended. Nothing has been changed. Open a file to fix it, or use View > Show Markdown Source.")
                .size(12)
                .color(c.text_muted),
            scrollable(column(rows).spacing(12)).height(iced::Shrink),
            row![
                space::horizontal(),
                button("Close")
                    .style(style::primary(c))
                    .on_press(Message::CloseDialog),
            ],
        ]
        .spacing(16);

        container(content)
            .width(560)
            .max_height(560)
            .padding(24)
            .style(style::dialog(c))
            .into()
    }

    /// Opens a new note in `folder`: a blank form if the folder has a content type.
    fn new_note_in(&mut self, folder: PathBuf) {
        let form = match self.types.get(&folder) {
            Some(Ok(ty)) => Some(Form::blank(ty.clone())),
            Some(Err(error)) => {
                self.status = format!(
                    "{} has a problem: {error}",
                    self.relative(&folder.join(types::TYPE_FILE))
                );
                None
            }
            None => None,
        };

        if let Some(form) = &form {
            self.status = format!("New {} created (not saved)", form.ty.label());
        }

        self.expanded.insert(folder.clone());
        self.add_tab(None, folder, "", form);
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
                self.rescan();
                self.status = problems_summary(&self.problems)
                    .unwrap_or_else(|| "Refreshed; no problems found".into());
                self.check_disk();
                Task::none()
            }
            Message::NewNoteIn(folder) => {
                self.new_note_in(folder);
                Task::none()
            }
            Message::ShowAddField => {
                if !self.active_tab().is_some_and(Tab::shows_form) {
                    return Task::none();
                }
                self.dialog = Some(Dialog::AddField {
                    label: String::new(),
                    kind: types::Kind::Text,
                    error: None,
                });
                iced::widget::operation::focus(DIALOG_INPUT)
            }
            Message::AddFieldLabel(value) => {
                if let Some(Dialog::AddField { label, error, .. }) = &mut self.dialog {
                    *label = value;
                    *error = None;
                }
                Task::none()
            }
            Message::AddFieldKind(value) => {
                if let Some(Dialog::AddField { kind, .. }) = &mut self.dialog {
                    *kind = value;
                }
                Task::none()
            }
            Message::AddField => {
                let Some(Dialog::AddField { label, kind, error }) = &mut self.dialog else {
                    return Task::none();
                };
                let Some(tab) = self.tabs.iter_mut().find(|tab| Some(tab.id) == self.active) else {
                    return Task::none();
                };
                let Some(form) = &mut tab.form else {
                    return Task::none();
                };

                match form.add_field(label, *kind) {
                    Ok(()) => {
                        tab.modified = true;
                        self.status = format!("Added the field {:?} to this note", label.trim());
                        self.dialog = None;
                    }
                    Err(message) => *error = Some(message),
                }
                Task::none()
            }
            Message::RestoreField(name) => {
                if let Some(Dialog::AddField { label, .. }) = &mut self.dialog {
                    *label = name;
                }
                self.update(Message::AddField)
            }
            Message::ShowNewFolder => {
                self.dialog = Some(Dialog::NewFolder {
                    name: String::new(),
                    template: Template::plain(),
                    error: None,
                });
                iced::widget::operation::focus(DIALOG_INPUT)
            }
            Message::NewFolderName(value) => {
                if let Some(Dialog::NewFolder { name, error, .. }) = &mut self.dialog {
                    *name = value;
                    *error = None;
                }
                Task::none()
            }
            Message::NewFolderTemplate(value) => {
                if let Some(Dialog::NewFolder { template, .. }) = &mut self.dialog {
                    *template = value;
                }
                Task::none()
            }
            Message::CreateFolder => {
                let Some(Dialog::NewFolder {
                    name,
                    template,
                    error,
                }) = &mut self.dialog
                else {
                    return Task::none();
                };

                match campaign::create_folder(&self.root, name, template.yaml.as_deref()) {
                    Ok(path) => {
                        self.status = format!("Created the folder {}", self.relative(&path));
                        self.dialog = None;
                        self.rescan();
                    }
                    Err(message) => *error = Some(message),
                }
                Task::none()
            }
            Message::CloseDialog => {
                self.dialog = None;
                Task::none()
            }
            Message::ShowTypes => {
                let mut folders = Vec::new();
                collect_folders(&self.tree, &mut folders);
                let mut editor = types_editor::Editor::new(&self.root, &folders, &self.types);
                if let Some(tab) = self.active_tab() {
                    editor.select(&tab.folder);
                }
                self.dialog = Some(Dialog::Types(editor));
                Task::none()
            }
            Message::Types(types_editor::Message::Cancel) => {
                self.dialog = None;
                Task::none()
            }
            Message::Types(types_editor::Message::Save) => {
                let Some(Dialog::Types(editor)) = &mut self.dialog else {
                    return Task::none();
                };
                match editor.save() {
                    Ok(saved) => {
                        self.dialog = None;
                        self.rescan();
                        self.reload_forms();
                        self.status = match saved.len() {
                            1 => format!("Saved the type of {}", self.relative(&saved[0])),
                            count => format!("Saved the types of {count} folders"),
                        };
                    }
                    Err(message) => editor.error = Some(message),
                }
                Task::none()
            }
            Message::Types(message) => {
                if let Some(Dialog::Types(editor)) = &mut self.dialog {
                    editor.update(message);
                }
                Task::none()
            }
            Message::Form(form::Message::FollowLink(index)) => self.follow_link_field(index),
            Message::Form(message) => {
                let click = matches!(
                    message,
                    form::Message::Long(_, text_editor::Action::Click(_))
                );
                let ctrl = self.ctrl;
                if let Some(tab) = self.active_tab_mut()
                    && let Some(form) = &mut tab.form
                {
                    tab.modified |= form.update(message);
                    if click
                        && ctrl
                        && let Some(target) = form.link_under_cursor()
                    {
                        return self.follow_link(&target);
                    }
                }
                Task::none()
            }
            Message::TreeEnter(path) => {
                if let Some((dragged, moved)) = &mut self.drag
                    && *dragged != path
                {
                    *moved = true;
                }
                self.hovered = Some(path);
                Task::none()
            }
            Message::TreeExit(path) => {
                if self.hovered.as_ref() == Some(&path) {
                    self.hovered = None;
                }
                Task::none()
            }
            Message::MousePressed => {
                self.drag = self
                    .hovered
                    .clone()
                    .filter(|path| campaign::is_markdown(path))
                    .map(|path| (path, false));
                Task::none()
            }
            Message::MouseReleased => {
                if let Some((path, true)) = self.drag.take()
                    && let Some(folder) = self.drop_folder()
                {
                    self.move_note(&path, &folder);
                }
                Task::none()
            }
            Message::Modifiers(modifiers) => {
                self.ctrl = modifiers.command();
                Task::none()
            }
            Message::CompleteLink(link) => {
                let Some(tab) = self.active_tab_mut() else {
                    return Task::none();
                };
                if tab.shows_form() {
                    let Some(form) = &mut tab.form else {
                        return Task::none();
                    };
                    let focus = form.focus();
                    tab.modified |= form.complete_link(&link);
                    match focus {
                        Some(index) => iced::widget::operation::focus(form::field_id(index)),
                        None => Task::none(),
                    }
                } else {
                    links::complete(&mut tab.content, &link);
                    tab.modified = true;
                    iced::widget::operation::focus(EDITOR)
                }
            }
            Message::FollowLink(target) => self.follow_link(&target),
            Message::InsertLink => {
                let Some(tab) = self.active_tab_mut() else {
                    return Task::none();
                };
                if !tab.is_note() {
                    return Task::none();
                }
                if tab.shows_form() {
                    let Some(form) = &mut tab.form else {
                        return Task::none();
                    };
                    let focus = form.focus();
                    if form.insert_link() {
                        tab.modified = true;
                        if let Some(index) = focus {
                            return iced::widget::operation::focus(form::field_id(index));
                        }
                    } else {
                        self.status =
                            "Click in a text box first, then press Ctrl+K to add a link there"
                                .into();
                    }
                    Task::none()
                } else {
                    links::insert(&mut tab.content);
                    tab.modified = true;
                    iced::widget::operation::focus(EDITOR)
                }
            }
            Message::ToggleSource => {
                let Some(tab) = self.tabs.iter_mut().find(|tab| Some(tab.id) == self.active) else {
                    return Task::none();
                };
                let Some(form) = &mut tab.form else {
                    return Task::none();
                };

                if tab.source {
                    match note::parse(&tab.content.text()) {
                        Ok(note) => {
                            *form = Form::from_note(form.ty.clone(), note);
                            tab.source = false;
                        }
                        Err(error) => self.status = format!("Can't show the form: {error}"),
                    }
                } else {
                    match form.to_markdown() {
                        Ok(text) => {
                            tab.content = text_editor::Content::with_text(&text);
                            tab.source = true;
                        }
                        Err(error) => self.status = format!("Can't show the source: {error}"),
                    }
                }
                Task::none()
            }
            Message::SelectTab(id) => {
                self.active = Some(id);
                Task::none()
            }
            Message::ShowProblems => {
                self.dialog = Some(Dialog::Problems);
                Task::none()
            }
            Message::OpenProblem(path) => {
                self.dialog = None;
                if campaign::is_yaml(&path) {
                    // Type files open as text; the tree only lists them when they are shown.
                    let folder = path.parent().map(Path::to_path_buf);
                    if let Some(folder) = folder {
                        self.expanded.insert(folder);
                    }
                }
                self.open_file(path);
                Task::none()
            }
            Message::WindowFocused => {
                // Ctrl may have been let go in another window.
                self.ctrl = false;
                self.rescan();
                self.check_disk();
                Task::none()
            }
            Message::ReloadFromDisk => {
                if let Some(index) = self.tabs.iter().position(|tab| Some(tab.id) == self.active) {
                    let name = self.tabs[index]
                        .path
                        .as_deref()
                        .map(|path| self.relative(path))
                        .unwrap_or_default();
                    self.status = match self.reload_tab(index) {
                        Ok(()) => format!("Reloaded {name}"),
                        Err(problem) => problem,
                    };
                }
                Task::none()
            }
            Message::KeepMine => {
                if let Some(tab) = self.active_tab_mut() {
                    tab.changed_outside = false;
                    tab.disk = tab.path.as_deref().and_then(check::modified_time);
                    self.status = "Keeping your version; saving will replace the file".into();
                }
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
                let click = matches!(action, text_editor::Action::Click(_));
                let ctrl = self.ctrl;
                if let Some(tab) = self.active_tab_mut() {
                    tab.modified |= action.is_edit();
                    if !tab.is_note() {
                        tab.content.perform(action);
                        return Task::none();
                    }
                    links::perform(&mut tab.content, action);
                    if click
                        && ctrl
                        && let Some(target) = links::under_cursor(&tab.content)
                    {
                        return self.follow_link(&target);
                    }
                }
                Task::none()
            }
            Message::NewNote => {
                self.add_tab(None, self.root.clone(), "", None);
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
                    tab.folder = path.parent().unwrap_or(&self.root).to_path_buf();
                    tab.disk = check::modified_time(&path);
                    tab.path = Some(path);
                    tab.modified = false;
                    tab.changed_outside = false;
                }
                self.rescan();
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

        let mut status_row = row![
            text(self.name()).size(12),
            space::horizontal(),
            text(match &self.drag {
                Some((path, true)) => format!(
                    "Drop {} on a folder to move it there",
                    display_name(path, false)
                ),
                _ => self.status.clone(),
            })
            .size(12)
            .wrapping(text::Wrapping::None),
        ]
        .spacing(16)
        .align_y(Center);

        if !self.problems.is_empty() {
            let files = problem_files(&self.problems);
            status_row = status_row.push(
                button(
                    text(format!(
                        "⚠ {files} {}",
                        if files == 1 {
                            "problem file"
                        } else {
                            "problem files"
                        }
                    ))
                    .size(12)
                    .color(c.status_text),
                )
                .padding([0, 6])
                .style(style::flat(c, false))
                .on_press(Message::ShowProblems),
            );
        }

        let status = container(status_row)
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

        // The space around the tree is the campaign folder, for dropping notes there.
        let root_area = |content: Element<'a, Message>| -> Element<'a, Message> {
            mouse_area(content)
                .on_enter(Message::TreeEnter(self.root.clone()))
                .on_exit(Message::TreeExit(self.root.clone()))
                .into()
        };
        let drop_root =
            matches!(self.drag, Some((_, true))) && self.drop_folder().as_ref() == Some(&self.root);
        let header = container(header)
            .width(Fill)
            .style(style::panel(if drop_root {
                c.selection
            } else {
                c.sidebar
            }))
            .into();

        container(column![
            root_area(header),
            stack![
                scrollable(column![tree, root_area(space().height(Fill).into())].padding([0, 4]))
                    .height(Fill),
                corner_button(c, Message::ShowNewFolder),
            ]
            .height(Fill)
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
            // While a note is dragged, the folder it would land in is highlighted.
            let drop_here = matches!(self.drag, Some((_, true)))
                && self.drop_folder().as_ref() == Some(&entry.path);
            let (arrow, message, selected) = match &entry.children {
                Some(_) if self.expanded.contains(&entry.path) => {
                    ("▾", Message::ToggleFolder(entry.path.clone()), drop_here)
                }
                Some(_) => ("▸", Message::ToggleFolder(entry.path.clone()), drop_here),
                None => (
                    "",
                    Message::OpenEntry(entry.path.clone()),
                    active_path == Some(&entry.path),
                ),
            };

            // A collapsed folder shows that something inside it has a problem.
            let has_problem = match entry.children {
                None => self.problems_in(&entry.path).next().is_some(),
                Some(_) => {
                    !self.expanded.contains(&entry.path)
                        && self
                            .problems
                            .iter()
                            .any(|problem| problem.path.starts_with(&entry.path))
                }
            };
            let warning = has_problem.then(|| text(" ⚠").size(12).color(c.warning));

            let label = button(
                row![indent, text(arrow).size(13).width(14), text(name).size(13)].push(warning),
            )
            .width(Fill)
            .padding([2, 0])
            .style(style::flat(c, selected))
            .on_press(message);

            let row: Element<'a, Message> = match entry.children {
                Some(_) => row![
                    label,
                    button(text("+").size(14))
                        .padding([0, 8])
                        .style(style::flat(c, false))
                        .on_press(Message::NewNoteIn(entry.path.clone())),
                ]
                .align_y(Center)
                .into(),
                None => label.into(),
            };

            rows.push(
                mouse_area(row)
                    .on_enter(Message::TreeEnter(entry.path.clone()))
                    .on_exit(Message::TreeExit(entry.path.clone()))
                    .into(),
            );

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
        let suggestions = self.link_suggestions();
        let editor = self.editor_body(c, &suggestions.notes);

        let mut view = column![];
        if self.active_tab().is_some_and(|tab| tab.changed_outside) {
            view = view.push(changed_outside_banner(c));
        }
        view.push(editor).push(self.link_bar(c, suggestions)).into()
    }

    /// The notes the link being typed in the active tab could be, as `(link text, label)`.
    ///
    /// The flag is true when the link is typed in a link field rather than a text editor.
    fn link_suggestions(&self) -> Suggestions {
        let none = Suggestions::default();
        let Some(tab) = self.active_tab().filter(|tab| tab.is_note()) else {
            return none;
        };
        let form = tab.form.as_ref().filter(|_| tab.shows_form());
        let query = match form {
            Some(form) => form.link_query(),
            None => links::query(&tab.content).map(|query| (query, false)),
        };
        let Some((query, in_field)) = query else {
            return none;
        };

        // A link field lists the notes of the folder it links to first.
        let field = form.and_then(|form| {
            let index = form.focused_link_field()?;
            Some((index, form.link_field(index)?.0))
        });
        let folder = field.and_then(|(_, field)| self.link_folder(field));

        let notes = self
            .index
            .suggest(&query, &tab.note_path(), folder.as_deref(), SUGGESTIONS);
        // A link field that already names a note needs no suggestions.
        if in_field && self.index.resolve(&query, &tab.note_path()).is_some() {
            return none;
        }

        // ...and offers to make the note it names, in that folder.
        let create = field.zip(folder.as_ref()).map(|((index, _), folder)| {
            let kind = match self.types.get(folder) {
                Some(Ok(ty)) => ty.label(),
                _ => "note".into(),
            };
            (
                index,
                format!(
                    "+ New {kind}: {} (in {})",
                    query.trim(),
                    self.relative(folder)
                ),
            )
        });

        Suggestions {
            notes,
            in_field,
            create,
        }
    }

    /// The folder link field `field` links to: the one its type names, or else a folder
    /// whose type has the field's name (so a `location` field links to Locations).
    fn link_folder(&self, field: &types::Field) -> Option<PathBuf> {
        if let Some(folder) = &field.folder {
            let path = self.root.join(folder.trim_matches('/'));
            return path.is_dir().then_some(path);
        }

        let names = [field.key.to_lowercase(), field.label().to_lowercase()];
        let mut matches: Vec<&PathBuf> = self
            .types
            .iter()
            .filter(|(_, ty)| {
                ty.as_ref().is_ok_and(|ty| {
                    names.contains(&ty.id.to_lowercase())
                        || names.contains(&ty.label().to_lowercase())
                })
            })
            .map(|(folder, _)| folder)
            .collect();
        matches.sort();
        matches.first().map(|folder| folder.to_path_buf())
    }

    /// The colors `[[links]]` are shown in, in text editors.
    fn link_colors(&self, c: Colors) -> links::Colors {
        links::Colors {
            link: c.accent,
            missing: c.text_muted,
            bold: c.accent == c.text,
            names: self.index.names(),
        }
    }

    /// The bar under the editor: notes to link to while a `[[link]]` is being typed, or a
    /// button to open the link the cursor is in.
    fn link_bar<'a>(&self, c: Colors, suggestions: Suggestions) -> Element<'a, Message> {
        let Suggestions {
            notes: suggestions,
            in_field,
            create,
        } = suggestions;
        let bar = |content| {
            container(content)
                .padding([6, 16])
                .width(Fill)
                .style(style::panel(c.sidebar))
                .into()
        };

        if !suggestions.is_empty() || create.is_some() {
            let suggestions_count = suggestions.len();
            let chips = suggestions.into_iter().map(|(link, label)| {
                button(text(label).size(13))
                    .padding([3, 10])
                    .style(style::secondary(c))
                    .on_press(Message::CompleteLink(link))
                    .into()
            });
            let create = create.map(|(index, label)| {
                button(text(label).size(13))
                    .padding([3, 10])
                    .style(style::secondary(c))
                    .on_press(Message::Form(form::Message::FollowLink(index)))
            });
            let hint = match (suggestions_count, in_field) {
                (0, _) => "",
                (_, true) => "Enter picks the first",
                (_, false) => "Tab inserts the first",
            };
            let chips = row(chips).push(create);
            return bar(row![
                text("Link to").size(13).color(c.text_muted),
                chips.spacing(6).wrap(),
                space::horizontal(),
                text(hint).size(12).color(c.text_muted),
            ]
            .spacing(10)
            .align_y(Center));
        }

        let Some(tab) = self.active_tab().filter(|tab| tab.is_note()) else {
            return space().height(0).into();
        };
        let target = match &tab.form {
            Some(form) if tab.shows_form() => form.link_under_cursor(),
            _ => links::under_cursor(&tab.content),
        };
        let Some(target) = target else {
            return space().height(0).into();
        };

        let exists = self.index.resolve(&target, &tab.note_path()).is_some();
        let (action, hint) = if exists {
            (format!("Open {target}"), "or Ctrl+click the link")
        } else {
            (
                format!("Create {target}"),
                "No note has this name yet; this makes one in the campaign folder",
            )
        };
        bar(row![
            text(format!("[[{target}]]")).size(13).color(if exists {
                c.accent
            } else {
                c.text_muted
            }),
            button(text(action).size(13))
                .padding([3, 10])
                .style(style::secondary(c))
                .on_press(Message::FollowLink(target)),
            text(hint).size(12).color(c.text_muted),
        ]
        .spacing(10)
        .align_y(Center))
    }

    /// Opens the note link field `index` of the active tab points to. If there is none,
    /// makes it in the folder the field links to, with that folder's type.
    fn follow_link_field(&mut self, index: usize) -> Task<Message> {
        let Some(form) = self.active_tab().and_then(|tab| tab.form.as_ref()) else {
            return Task::none();
        };
        let Some((field, target)) = form.link_field(index) else {
            return Task::none();
        };
        if target.trim().is_empty() {
            return Task::none();
        }
        let target = target.trim().to_string();
        let folder = self.link_folder(field);
        self.follow_link_into(&target, folder)
    }

    /// Opens the note `target` links to from the active tab. If there is none, makes an
    /// empty one in the campaign folder (or the subfolder the link names), as Obsidian does.
    fn follow_link(&mut self, target: &str) -> Task<Message> {
        self.follow_link_into(target, None)
    }

    /// Like [`Self::follow_link`], but a missing note is made in `folder`, if given.
    fn follow_link_into(&mut self, target: &str, folder: Option<PathBuf>) -> Task<Message> {
        let from = self
            .active_tab()
            .map(Tab::note_path)
            .unwrap_or_else(|| self.root.join("Untitled.md"));

        if let Some(path) = self.index.resolve(target, &from) {
            let path = path.to_path_buf();
            self.open_file(path);
            return Task::none();
        }

        let ty = folder
            .as_ref()
            .and_then(|folder| self.types.get(folder)?.as_ref().ok())
            .cloned();
        match create_linked_note(&self.root, target, folder.as_deref(), ty.as_ref()) {
            Ok(path) => {
                self.rescan();
                if let Some(folder) = path.parent() {
                    self.expanded.insert(folder.to_path_buf());
                }
                self.open_file(path.clone());
                self.status = match ty {
                    Some(ty) => format!(
                        "Created {} as a new {}: no note had that name yet",
                        self.relative(&path),
                        ty.label()
                    ),
                    None => format!(
                        "Created {}: no note had that name yet",
                        self.relative(&path)
                    ),
                };
            }
            Err(message) => self.status = message,
        }
        Task::none()
    }

    /// Rewrites `[[links]]` across the campaign after the note at `old` was renamed to `new`,
    /// so they still point to it. Open tabs are updated too, unsaved changes and all.
    ///
    /// The status bar says `done`, then how many links changed.
    fn relink(&mut self, old: &Path, new: &Path, done: &str) {
        let notes = markdown_files(&self.tree);
        let after = links::Index::new(&self.root, notes.iter().cloned());
        let before = links::Index::new(
            &self.root,
            notes.iter().map(|path| {
                if path == new {
                    old.to_path_buf()
                } else {
                    path.clone()
                }
            }),
        );
        let rename = links::Rename {
            old,
            new,
            before: &before,
            after: &after,
        };

        let mut changed_links = 0;
        let mut changed_notes = 0;
        let mut problems = Vec::new();

        for path in &notes {
            let tab = self
                .tabs
                .iter()
                .position(|tab| tab.path.as_ref() == Some(path));
            let was = check::modified_time(path);

            let Ok(text) = std::fs::read_to_string(path) else {
                problems.push(self.relative(path));
                continue;
            };
            if let Some((text, count)) = links::relink_note(&text, path, &rename) {
                if std::fs::write(path, text).is_err() {
                    problems.push(self.relative(path));
                    continue;
                }
                changed_links += count;
                changed_notes += 1;

                if let Some(index) = tab {
                    if self.tabs[index].modified {
                        // Only the links changed on disk, so don't treat it as an outside edit.
                        if self.tabs[index].disk == was {
                            self.tabs[index].disk = check::modified_time(path);
                        }
                    } else {
                        let _ = self.reload_tab(index);
                    }
                }
            }
        }

        // Unsaved changes in open tabs get their links rewritten as well.
        for index in 0..self.tabs.len() {
            let tab = &self.tabs[index];
            if !tab.modified || !tab.is_note() {
                continue;
            }
            let path = tab.note_path();
            let Ok(text) = tab.text() else {
                problems.push(tab.name(true));
                continue;
            };
            let Some((text, _)) = links::relink_note(&text, &path, &rename) else {
                continue;
            };

            let tab = &mut self.tabs[index];
            match &tab.form {
                Some(form) if !tab.source => {
                    if let Ok(note) = note::parse(&text) {
                        tab.form = Some(Form::from_note(form.ty.clone(), note));
                    }
                }
                _ => tab.content = text_editor::Content::with_text(&text),
            }
        }

        let renamed = done;
        self.status = match (changed_links, changed_notes) {
            (0, _) => format!("{renamed}; no links needed updating"),
            (1, _) => format!("{renamed} and updated 1 link"),
            (links, 1) => format!("{renamed} and updated {links} links in 1 note"),
            (links, notes) => format!("{renamed} and updated {links} links in {notes} notes"),
        };
        if !problems.is_empty() {
            self.status.push_str(&format!(
                ". Could not update the links in {}",
                problems.join(", ")
            ));
        }
    }

    /// The folder a note dragged in the file tree would be dropped in: the folder under
    /// the mouse, or the folder of the file under it.
    fn drop_folder(&self) -> Option<PathBuf> {
        let hovered = self.hovered.as_ref()?;
        if hovered.is_dir() {
            Some(hovered.clone())
        } else {
            hovered.parent().map(Path::to_path_buf)
        }
    }

    /// Moves the note at `path` into `folder`. It takes the folder's type, keeping every
    /// field that has something in it (fields the new type doesn't have become the note's
    /// own), and links to it are rewritten where they need to be.
    fn move_note(&mut self, path: &Path, folder: &Path) {
        let Some(name) = path.file_name() else {
            return;
        };
        if path.parent() == Some(folder) || !folder.starts_with(&self.root) {
            return;
        }
        let target = folder.join(name);
        let label = display_name(path, false);
        let where_to = match folder.strip_prefix(&self.root) {
            Ok(relative) if relative.as_os_str().is_empty() => "the campaign folder".to_string(),
            Ok(relative) => relative.display().to_string(),
            Err(_) => folder.display().to_string(),
        };

        if let Some(tab) = self
            .tabs
            .iter()
            .find(|tab| tab.path.as_deref() == Some(path))
            && tab.modified
        {
            self.status = format!("Save {label} before moving it");
            return;
        }
        if target.exists() {
            self.status =
                format!("Can't move {label}: {where_to} already has a note with that name");
            return;
        }

        let ty = match self.types.get(folder) {
            Some(Ok(ty)) => Some(ty.clone()),
            _ => None,
        };
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                self.status = format!("Could not read {label}: {error}");
                return;
            }
        };
        let retyped = ty.as_ref().and_then(|ty| retype(&text, ty, &label));

        if let Err(error) = std::fs::rename(path, &target) {
            self.status = format!("Could not move {label}: {error}");
            return;
        }
        if let Some(retyped) = &retyped
            && let Err(error) = std::fs::write(&target, retyped)
        {
            self.status = format!("Moved {label}, but could not give it its new type: {error}");
        }

        self.expanded.insert(folder.to_path_buf());
        self.rescan();
        for index in 0..self.tabs.len() {
            if self.tabs[index].path.as_deref() == Some(path) {
                self.tabs[index].path = Some(target.clone());
                self.tabs[index].folder = folder.to_path_buf();
                let _ = self.reload_tab(index);
            }
        }

        let done = match (&ty, retyped.is_some()) {
            (Some(ty), true) => {
                let kind = ty.label();
                let article =
                    if kind.starts_with(['A', 'E', 'I', 'O', 'U', 'a', 'e', 'i', 'o', 'u']) {
                        "an"
                    } else {
                        "a"
                    };
                format!("Moved {label} to {where_to} as {article} {kind}")
            }
            _ => format!("Moved {label} to {where_to}"),
        };
        self.relink(path, &target, &done);
    }

    fn editor_body(&self, c: Colors, suggestions: &[(String, String)]) -> Element<'_, Message> {
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

        let accept = suggestions
            .first()
            .map(|(link, _)| Message::CompleteLink(link.clone()));

        if let Some(form) = &tab.form
            && !tab.source
        {
            let form =
                container(form.view(c, Message::Form, shortcut, self.link_colors(c), accept))
                    .width(Fill)
                    .height(Fill)
                    .style(style::panel(c.background));

            return stack![form, corner_button(c, Message::ShowAddField)].into();
        }

        let editor = text_editor(&tab.content)
            .id(EDITOR)
            .placeholder("Start writing...")
            .on_action(Message::Edit)
            .key_binding(move |key_press| {
                if let Some(binding) = links::tab_binding(&key_press, &accept) {
                    return Some(binding);
                }
                // Every editor sees every key press, so only the focused one handles shortcuts.
                if key_press.modifiers.command()
                    && matches!(key_press.status, text_editor::Status::Focused { .. })
                    && let Some(message) = shortcut(&key_press.key, key_press.modifiers)
                {
                    return Some(text_editor::Binding::Custom(message));
                }
                text_editor::Binding::from_key_press(key_press)
            })
            .padding(16)
            .height(Fill)
            .style(style::editor(c));

        if tab.is_note() {
            editor
                .highlight_with::<links::Highlighter>(self.link_colors(c), links::format)
                .into()
        } else {
            editor.into()
        }
    }
}

/// The text of a note moved into a folder of type `ty`: its front matter names the new
/// type, and gets a name (`name`, from the filename) if the type wants one.
///
/// Every other key and section is kept as it is: what the new type doesn't have shows
/// as the note's own fields. Returns `None` if the front matter can't be read.
fn retype(text: &str, ty: &ContentType, name: &str) -> Option<String> {
    let (front, body) = note::split_front_matter(text).ok()?;
    let mut front = match front {
        Some(yaml) => match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(yaml).ok()? {
            serde_yaml_ng::Value::Mapping(front) => front,
            serde_yaml_ng::Value::Null => serde_yaml_ng::Mapping::new(),
            _ => return None,
        },
        None => serde_yaml_ng::Mapping::new(),
    };

    // `type` goes first, as the form writes it.
    let mut retyped = serde_yaml_ng::Mapping::new();
    retyped.insert("type".into(), ty.id.clone().into());
    // The removed fields were the old type's.
    front.shift_remove(form::REMOVED_KEY);
    front.shift_remove("type");
    if ty.name_field().is_some() && !front.contains_key("name") {
        retyped.insert("name".into(), name.into());
    }
    retyped.extend(front);

    let yaml = serde_yaml_ng::to_string(&retyped).ok()?;
    let body = body.trim_start_matches(['\r', '\n']);
    Some(if body.is_empty() {
        format!("---\n{yaml}---\n")
    } else {
        format!("---\n{yaml}---\n\n{body}")
    })
}

/// Lists every Markdown file in the tree.
fn markdown_files(entries: &[Entry]) -> Vec<PathBuf> {
    fn visit(entries: &[Entry], files: &mut Vec<PathBuf>) {
        for entry in entries {
            match &entry.children {
                Some(children) => visit(children, files),
                None if campaign::is_markdown(&entry.path) => files.push(entry.path.clone()),
                None => {}
            }
        }
    }

    let mut files = Vec::new();
    visit(entries, &mut files);
    files
}

/// Makes an empty note for a link to `target`, which names no note yet: `<target>.md`
/// in `folder` (or else the campaign folder `root`), or in the folder the target's path
/// names. With a type `ty`, the note gets its `type:` and its name filled in.
fn create_linked_note(
    root: &Path,
    target: &str,
    folder: Option<&Path>,
    ty: Option<&ContentType>,
) -> Result<PathBuf, String> {
    let target = target.trim().replace('\\', "/");
    let target = target.trim_matches('/');
    let target = target
        .strip_suffix(".md")
        .or_else(|| target.strip_suffix(".MD"))
        .unwrap_or(target);
    if Path::new(target).extension().is_some_and(|extension| {
        // Links can point to images and other files, which aren't notes to create.
        ["png", "jpg", "jpeg", "gif", "svg", "webp", "pdf", "canvas"]
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
    }) {
        return Err(format!("There is no file called {target} in this campaign"));
    }

    let mut path = match folder {
        Some(folder) if !target.contains('/') => folder.to_path_buf(),
        _ => root.to_path_buf(),
    };
    for part in target.split('/') {
        campaign::validate_name(part.trim())
            .map_err(|error| format!("Can't make a note for [[{target}]]: {error}"))?;
        path.push(part.trim());
    }
    let folder = path.parent().unwrap_or(root);
    if !folder.is_dir() {
        return Err(format!(
            "Can't make a note for [[{target}]]: there is no folder {}",
            folder.strip_prefix(root).unwrap_or(folder).display()
        ));
    }

    let text = match ty {
        Some(ty) => {
            let mut front = serde_yaml_ng::Mapping::new();
            front.insert("type".into(), ty.id.clone().into());
            if ty.name_field().is_some() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                front.insert("name".into(), name.into_owned().into());
            }
            note::to_markdown(&front, &[], "")?
        }
        None => String::new(),
    };

    path.as_mut_os_string().push(".md");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| format!("Could not create {}: {error}", path.display()))?;
    std::io::Write::write_all(&mut file, text.as_bytes())
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    Ok(path)
}

/// Lists every folder in the tree, parents before their subfolders.
fn collect_folders(entries: &[Entry], folders: &mut Vec<PathBuf>) {
    for entry in entries {
        if let Some(children) = &entry.children {
            folders.push(entry.path.clone());
            collect_folders(children, folders);
        }
    }
}

/// Reads the `_type.yaml` of every folder in the campaign.
fn load_types(root: &Path, tree: &[Entry]) -> HashMap<PathBuf, Result<ContentType, String>> {
    fn visit(entries: &[Entry], types: &mut HashMap<PathBuf, Result<ContentType, String>>) {
        for entry in entries {
            if let Some(children) = &entry.children {
                if let Some(ty) = types::load(&entry.path) {
                    types.insert(entry.path.clone(), ty);
                }
                visit(children, types);
            }
        }
    }

    let mut types = HashMap::new();
    if let Some(ty) = types::load(root) {
        types.insert(root.to_path_buf(), ty);
    }
    visit(tree, &mut types);
    types
}

/// Saves a note as "<name>.md" in its folder, renaming the file if the name changed.
fn save_named(tab: &Tab, text: &str, label: &str) -> Result<PathBuf, String> {
    let name = tab
        .note_name()
        .ok_or_else(|| format!("Give the {label} a name before saving"))?;
    campaign::validate_name(&name)?;

    let folder = tab
        .path
        .as_ref()
        .and_then(|path| path.parent())
        .unwrap_or(&tab.folder);
    let target = folder.join(format!("{name}.md"));

    match &tab.path {
        Some(old) if *old == target => {}
        // Renaming only changes the case of the name: the file is the same on Windows.
        Some(old)
            if old.to_string_lossy().to_lowercase() == target.to_string_lossy().to_lowercase() =>
        {
            std::fs::rename(old, &target).map_err(|error| error.to_string())?;
        }
        _ if target.exists() => {
            return Err(format!("{name}.md already exists in this folder"));
        }
        Some(old) => {
            std::fs::rename(old, &target).map_err(|error| error.to_string())?;
        }
        None => {}
    }

    std::fs::write(&target, text).map_err(|error| error.to_string())?;
    Ok(target)
}

/// The number of files in `problems`.
fn problem_files(problems: &[Problem]) -> usize {
    let files: HashSet<&Path> = problems
        .iter()
        .map(|problem| problem.path.as_path())
        .collect();
    files.len()
}

/// A line for the status bar saying how many files have problems, if any do.
fn problems_summary(problems: &[Problem]) -> Option<String> {
    match problem_files(problems) {
        0 => None,
        1 => Some("1 file has formatting problems (click ⚠ to see them)".into()),
        files => Some(format!(
            "{files} files have formatting problems (click ⚠ to see them)"
        )),
    }
}

/// Shown above a file that changed on disk while it had unsaved changes.
fn changed_outside_banner<'a>(c: Colors) -> Element<'a, Message> {
    container(
        row![
            text("This file was changed outside Dungeon Elements since you opened it.")
                .size(13)
                .color(c.text),
            space::horizontal(),
            button(text("Reload").size(13))
                .style(style::secondary(c))
                .on_press(Message::ReloadFromDisk),
            button(text("Keep Mine").size(13))
                .style(style::primary(c))
                .on_press(Message::KeepMine),
        ]
        .spacing(8)
        .align_y(Center),
    )
    .padding([8, 16])
    .width(Fill)
    .style(style::banner(c))
    .into()
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
        "e" => Some(Message::ToggleSource),
        "k" => Some(Message::InsertLink),
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
                "Adds these folders, each with its own note fields: {}",
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

/// The round + button in the bottom right corner of a pane, like in the layout plan.
fn corner_button<'a>(c: Colors, on_press: Message) -> Element<'a, Message> {
    container(
        button(text("+").size(28).center().width(Fill).height(Fill))
            .width(48)
            .height(48)
            .padding(0)
            .style(style::round(c))
            .on_press(on_press),
    )
    .width(Fill)
    .height(Fill)
    .align_right(Fill)
    .align_bottom(Fill)
    .padding(16)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moved_notes_take_the_new_type_and_keep_their_data() {
        let lore = types::parse(
            "type: lore\nfields:\n  - key: name\n  - key: category\n    kind: choice\n    options: [Myth]\n",
        )
        .unwrap();
        let item = types::parse(types::STARTER_TYPES[0].1).unwrap();
        assert_eq!(item.id, "item");

        let text = "---\ntype: lore\nname: The Tide Crown\ncategory: Myth\nremoved_fields:\n- secrets\n---\n\nIntro.\n\n## History\n\nSunk.\n";
        let moved = retype(text, &item, "The Tide Crown").unwrap();
        assert_eq!(
            moved,
            "---\ntype: item\nname: The Tide Crown\ncategory: Myth\n---\n\nIntro.\n\n## History\n\nSunk.\n"
        );

        // In the form, the category is a field of the note's own, next to the item's.
        let form = Form::from_note(item, note::parse(&moved).unwrap());
        assert_eq!(form.name().as_deref(), Some("The Tide Crown"));
        assert!(form.to_markdown().unwrap().contains("category: Myth"));

        // A plain note gets front matter, with its filename as its name.
        let moved = retype("Just text.\n", &lore, "Old Song").unwrap();
        assert_eq!(
            moved,
            "---\ntype: lore\nname: Old Song\n---\n\nJust text.\n"
        );
    }

    #[test]
    fn linked_notes_are_made_in_the_fields_folder() {
        let root =
            std::env::temp_dir().join(format!("dungeon_elements_linked_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let locations = root.join("locations");
        std::fs::create_dir_all(&locations).unwrap();
        let ty = types::parse("type: location\nfields:\n  - key: name\n").unwrap();

        let path =
            create_linked_note(&root, "Smuggler's Cove", Some(&locations), Some(&ty)).unwrap();
        assert_eq!(path, locations.join("Smuggler's Cove.md"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "---\ntype: location\nname: Smuggler's Cove\n---\n"
        );
        assert!(create_linked_note(&root, "Smuggler's Cove", Some(&locations), Some(&ty)).is_err());

        // Body links have no folder: the note goes in the campaign folder, empty.
        let path = create_linked_note(&root, "Drowned Vault", None, None).unwrap();
        assert_eq!(path, root.join("Drowned Vault.md"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");

        std::fs::remove_dir_all(root).unwrap();
    }
}
