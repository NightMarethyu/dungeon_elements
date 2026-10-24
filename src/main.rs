//! Dungeon Elements: a local-first campaign manager for tabletop RPGs.

mod campaign;
mod style;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{
    button, center, checkbox, column, container, opaque, row, rule, scrollable, space, stack, text,
    text_editor, text_input,
};
use iced::{Center, Element, Fill, Subscription, Task};

use campaign::Entry;

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title(App::title)
        .theme(style::theme())
        .subscription(App::subscription)
        .window_size((1200.0, 800.0))
        .run()
}

#[derive(Default)]
struct App {
    screen: Screen,
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
    fn name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .map_or("Untitled".into(), |name| {
                name.to_string_lossy().into_owned()
            })
    }
}

#[derive(Debug, Clone)]
enum Message {
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
    fn title(&self) -> String {
        match &self.screen {
            Screen::Splash { .. } => "Dungeon Elements".into(),
            Screen::Workspace(workspace) => {
                format!("{} - Dungeon Elements", workspace.name())
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match self.screen {
            Screen::Splash { .. } => Subscription::none(),
            Screen::Workspace(_) => keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed { key, modifiers, .. } => shortcut(&key, modifiers),
                _ => None,
            }),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
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
        match &self.screen {
            Screen::Splash {
                new_campaign,
                error,
            } => {
                let splash = splash(error.as_deref());

                match new_campaign {
                    Some(form) => modal(splash, new_campaign_dialog(form)),
                    None => splash,
                }
            }
            Screen::Workspace(workspace) => workspace.view(),
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

    fn view(&self) -> Element<'_, Message> {
        let menu_button = |label| {
            button(text(label).size(13))
                .padding([4, 10])
                .style(style::flat(false))
        };

        let menu = container(
            row![
                menu_button("New Note").on_press(Message::NewNote),
                menu_button("Open File...").on_press(Message::OpenFile),
                menu_button("Save").on_press_maybe(self.active.map(|_| Message::Save)),
                menu_button("Save As...").on_press_maybe(self.active.map(|_| Message::SaveAs)),
                space::horizontal(),
                menu_button("Close Campaign").on_press(Message::CloseCampaign),
            ]
            .spacing(2)
            .align_y(Center),
        )
        .padding([2, 6])
        .width(Fill)
        .style(style::panel(style::TITLE_BAR));

        let main = column![self.tab_bar(), self.editor()];

        let body = row![self.sidebar(), rule::vertical(1), main].height(Fill);

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
        .style(style::status_bar);

        column![menu, body, status].into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let header = row![
            text(self.name().to_uppercase())
                .size(11)
                .color(style::MUTED),
            space::horizontal(),
            button(text("⟳").size(13))
                .padding([0, 6])
                .style(style::flat(false))
                .on_press(Message::RefreshTree),
        ]
        .align_y(Center)
        .padding([6, 12]);

        let tree: Element<'_, Message> = if self.tree.is_empty() {
            text("This campaign has no folders or Markdown files yet.")
                .size(13)
                .color(style::MUTED)
                .into()
        } else {
            let mut rows = Vec::new();
            self.tree_rows(&self.tree, 0, &mut rows);
            column(rows).into()
        };

        container(column![
            header,
            scrollable(container(tree).padding([0, 4])).height(Fill)
        ])
        .width(260)
        .height(Fill)
        .style(style::panel(style::SIDEBAR))
        .into()
    }

    /// Adds a row for each entry, and for the contents of expanded folders.
    fn tree_rows<'a>(
        &'a self,
        entries: &'a [Entry],
        depth: u16,
        rows: &mut Vec<Element<'a, Message>>,
    ) {
        let active_path = self.active_tab().and_then(|tab| tab.path.as_ref());

        for entry in entries {
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
                text(&entry.name).size(13)
            ])
            .width(Fill)
            .padding([2, 0])
            .style(style::flat(selected))
            .on_press(message);

            rows.push(row.into());

            if let Some(children) = &entry.children
                && self.expanded.contains(&entry.path)
            {
                self.tree_rows(children, depth + 1, rows);
            }
        }
    }

    fn tab_bar(&self) -> Element<'_, Message> {
        let tabs = self.tabs.iter().map(|tab| {
            let active = Some(tab.id) == self.active;
            let close_label = if tab.modified { "●" } else { "×" };

            container(
                row![
                    button(text(tab.name()).size(13))
                        .padding([8, 4])
                        .style(style::tab_button(active))
                        .on_press(Message::SelectTab(tab.id)),
                    button(text(close_label).size(13))
                        .padding([8, 4])
                        .style(style::tab_button(active))
                        .on_press(Message::CloseTab(tab.id)),
                ]
                .align_y(Center),
            )
            .padding([0, 6])
            .style(style::tab(active))
            .into()
        });

        container(
            scrollable(row(tabs).spacing(1)).direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(3).scroller_width(3),
            )),
        )
        .width(Fill)
        .style(style::panel(style::SIDEBAR))
        .into()
    }

    fn editor(&self) -> Element<'_, Message> {
        let Some(tab) = self.active_tab() else {
            return center(
                column![
                    text("No file open").size(20).color(style::MUTED),
                    text("Pick a file in the sidebar, or press Ctrl+N for a new note.")
                        .size(14)
                        .color(style::MUTED),
                ]
                .spacing(8)
                .align_x(Center),
            )
            .style(style::panel(style::EDITOR))
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
            .style(style::editor)
            .into()
    }
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

fn splash(error: Option<&str>) -> Element<'_, Message> {
    let big_button = |label| {
        button(text(label).size(16).width(Fill).align_x(Center))
            .padding([12, 24])
            .width(240)
    };

    let mut content = column![
        text("Dungeon Elements").size(44),
        text("A local-first campaign manager for tabletop RPGs")
            .size(16)
            .color(style::MUTED),
        space().height(24),
        big_button("Open Campaign...")
            .style(style::primary)
            .on_press(Message::OpenCampaign),
        big_button("New Campaign...")
            .style(style::secondary)
            .on_press(Message::ShowNewCampaign),
    ]
    .spacing(12)
    .align_x(Center);

    if let Some(error) = error {
        content = content.push(
            text(error)
                .color(style::theme().palette().danger)
                .width(480)
                .align_x(Center),
        );
    }

    center(content).style(style::panel(style::EDITOR)).into()
}

fn new_campaign_dialog(form: &NewCampaign) -> Element<'_, Message> {
    let label = |label| text(label).size(13).color(style::MUTED);

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
        text("New campaign").size(22),
        column![
            label("Title"),
            text_input("The Sunken Keep", &form.title)
                .id(TITLE_INPUT)
                .on_input(Message::TitleChanged)
                .on_submit(Message::CreateCampaign)
                .padding(8),
        ]
        .spacing(4),
        column![
            label("Location"),
            row![
                text_input("Folder to save the campaign in", &form.location)
                    .on_input(Message::LocationChanged)
                    .on_submit(Message::CreateCampaign)
                    .padding(8),
                button("Browse...")
                    .style(style::secondary)
                    .padding(8)
                    .on_press(Message::BrowseLocation),
            ]
            .spacing(8),
            text(folder_preview).size(12).color(style::MUTED),
        ]
        .spacing(4),
        column![
            checkbox(form.with_template)
                .label("Set up the starter folder structure")
                .on_toggle(Message::TemplateToggled),
            text(format!(
                "Adds the folders: {}",
                campaign::TEMPLATE_FOLDERS.join(", ")
            ))
            .size(12)
            .color(style::MUTED),
        ]
        .spacing(4),
    ]
    .spacing(16);

    if let Some(error) = &form.error {
        content = content.push(text(error).size(13).color(style::theme().palette().danger));
    }

    content = content.push(
        row![
            space::horizontal(),
            button("Cancel")
                .style(style::secondary)
                .on_press(Message::CancelNewCampaign),
            button("Create")
                .style(style::primary)
                .on_press(Message::CreateCampaign),
        ]
        .spacing(8),
    );

    container(content)
        .width(480)
        .padding(24)
        .style(style::dialog)
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
        .set_title("Open Markdown file")
        .add_filter("Markdown", &["md"])
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

/// Appends `.md` to `path` unless it already ends in it, so files are always Markdown.
fn with_md_extension(mut path: PathBuf) -> PathBuf {
    if !campaign::is_markdown(&path) {
        path.as_mut_os_string().push(".md");
    }

    path
}
