//! Dungeon Elements: a local-first campaign manager for tabletop RPGs.

// Release builds on Windows open without a console window behind the app.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod backlinks;
mod campaign;
mod check;
mod code;
mod commands;
mod context_menu;
mod date;
mod dice;
mod fix;
mod form;
mod format;
mod help;
mod icon;
mod links;
mod marks;
mod menu;
mod note;
mod preview;
mod quests;
mod recent;
mod search;
mod settings;
mod style;
mod table_editor;
mod table_picker;
mod tables;
mod tags;
mod templates;
mod theme;
mod types;
mod types_editor;
mod walkthrough;
mod walkthrough_editor;
mod walkthrough_tab;
mod watch;
mod web;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{
    Svg, button, center, checkbox, column, container, mouse_area, opaque, pick_list, row, rule,
    scrollable, space, stack, svg, text, text_editor, text_input, tooltip,
};
use iced::{Center, Element, Fill, Point, Size, Subscription, Task, window};

use campaign::Entry;
use check::Problem;
use commands::Command;
use form::Form;
use menu::Menu;
use preview::Preview;
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
    /// The dice roller's icons, in the theme's colors.
    dice: dice::Icons,
    /// The dropdown menu that is open, if any.
    menu: Option<Menu>,
    /// Where the arrow keys are in the open menu, and its open submenu.
    menu_cursor: menu::Cursor,
    /// What to do once the campaign has closed: File > New Campaign or Open Campaign
    /// while a campaign is open.
    after_close: Option<Message>,
    /// Whether the "About" dialog is showing.
    about: bool,
    /// The Help window, if it is open.
    help: Option<help::Help>,
    /// The window's size, for keeping right-click menus inside it.
    window: Size,
}

enum Screen {
    /// Asks whether to open a campaign or create a new one.
    Splash {
        new_campaign: Option<Box<NewCampaign>>,
        open_campaign: Option<OpenCampaign>,
        error: Option<String>,
    },
    Workspace(Box<Workspace>),
}

impl Default for Screen {
    fn default() -> Self {
        Screen::Splash {
            new_campaign: None,
            open_campaign: None,
            error: None,
        }
    }
}

/// The widget id of the title field in the "New campaign" dialog.
const TITLE_INPUT: &str = "new-campaign-title";
/// The widget id of the box that finds a campaign in the "Open campaign" dialog.
const OPEN_FILTER: &str = "open-campaign-filter";
/// The widget id of the name field in the workspace's dialogs.
const DIALOG_INPUT: &str = "dialog-name";
/// The folder New Roll Table puts tables in to start with, made when one is.
const TABLES_FOLDER: &str = "tables";
/// The widget id of the text editor that shows a file's text.
const EDITOR: &str = "editor";
/// The editor for a block of the reading view, and the input for one of its fields.
const BLOCK_EDITOR: &str = "block-editor";
const FIELD_EDITOR: &str = "field-editor";
/// How many notes the link bar suggests at most.
const SUGGESTIONS: usize = 8;

/// How many notes the walkthrough offers to pick from at most.
const PICKS: usize = 24;

/// The fields of the "New campaign" dialog.
struct NewCampaign {
    title: String,
    location: String,
    /// The templates to start from: the starter folders, none, and the user's own.
    templates: Vec<templates::Template>,
    template: templates::Template,
    /// The folders the picked template makes, or why it can't be read.
    structure: Result<templates::Structure, String>,
    /// Whether to start the walkthrough once the campaign is made.
    walkthrough: bool,
    error: Option<String>,
}

impl NewCampaign {
    /// Picks `template`, reading the folders it makes.
    fn pick(&mut self, template: templates::Template) {
        self.structure = template.structure();
        self.template = template;
    }

    /// Whether the walkthrough has questions for the folders the picked template makes:
    /// the template's own questions, or else the built-in ones.
    fn has_walkthrough(&self) -> bool {
        let Ok(structure) = &self.structure else {
            return false;
        };
        let typed: Vec<&str> = structure
            .folders
            .iter()
            .filter(|(_, ty)| ty.is_some())
            .map(|(folder, _)| folder.as_str())
            .collect();
        match &structure.walkthrough {
            Some(yaml) => walkthrough::parse(yaml).is_ok_and(|own| own.asks_about(&typed)),
            None => walkthrough::Questions::built_in().asks_about(&typed),
        }
    }
}

/// The "Open campaign" dialog: the campaigns opened before, to pick one from.
struct OpenCampaign {
    campaigns: Vec<recent::Campaign>,
    /// Text typed to find a campaign by name or folder.
    filter: String,
}

impl OpenCampaign {
    /// The campaigns whose name or folder has the text typed in them.
    fn shown(&self) -> impl Iterator<Item = &recent::Campaign> {
        let filter = self.filter.trim().to_lowercase();
        self.campaigns.iter().filter(move |campaign| {
            campaign
                .path
                .to_string_lossy()
                .to_lowercase()
                .contains(&filter)
        })
    }
}

/// An open campaign folder: its file tree and the files open in tabs.
struct Workspace {
    root: PathBuf,
    tree: Vec<Entry>,
    /// The content type of each typed folder, or what is wrong with it. A subfolder's
    /// has its parent's fields too (see [`types::resolve`]).
    types: types::Types,
    /// What each folder's own `_type.yaml` says, without what it inherits. Only these
    /// are written back to the files.
    own_types: types::Types,
    /// What is wrong with the campaign's files, checked when it opens and whenever its
    /// files change. Notes with unsaved changes are checked as they are in their tab.
    problems: Vec<Problem>,
    /// Notes renamed or deleted outside the app since the campaign opened, so links to
    /// them show up as broken.
    gone: check::Gone,
    /// Broken links the user chose to leave as they are: the note and the link's target,
    /// [`links::normalize`]d. Kept until the campaign is closed.
    ignored: HashSet<(PathBuf, String)>,
    /// A hash of each note's text as of the last scan, to tell a note renamed outside
    /// the app from one deleted while another was made.
    fingerprints: HashMap<PathBuf, u64>,
    /// The names to show in the file tree and tabs for notes whose name has characters a
    /// file name can't hold, like `Chapter 1: The Inn` for `Chapter 1 - The Inn.md`.
    titles: HashMap<PathBuf, String>,
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
    /// What each note links to, for the notes that link to the one in the reading view.
    /// Open notes count as their tabs have them, unsaved changes and all.
    backlinks: backlinks::Backlinks,
    /// Whether Ctrl (Cmd on macOS) is held, so a click on a link opens it.
    ctrl: bool,
    /// The row of the file tree under the mouse: a file, a folder, or the campaign
    /// folder itself for the space around the tree.
    hovered: Option<PathBuf>,
    /// The note being dragged in the file tree, and whether it has left its row yet.
    drag: Option<(PathBuf, bool)>,
    /// The dice roller along the bottom of the main pane, when it is open.
    dice: dice::Roller,
    /// Whether notes open in the reading view rather than an editor.
    open_in_reading: bool,
    /// Pictures from the web shown in the reading view, by their URL.
    web: web::Pictures,
    /// The right-click menu that is open, and where in the window.
    context: Option<(Target, Point)>,
    /// The block of the reading view the mouse last went over, and the one it was over
    /// when the right-click menu opened.
    hovered_block: Option<usize>,
    menu_block: Option<usize>,
    /// How a File > Save All is going, until every note in it is saved or not.
    save_all: Option<SaveAll>,
    /// The tags that have notes, offered in tags fields.
    tags: form::KnownTags,
    /// The tags fields of notes being saved in the background, and what the notes said
    /// before, by tab, to bring their tags' notes up to date once they are saved.
    tag_saves: HashMap<u64, (Vec<(String, String)>, String)>,
    /// The sidebar's search, which shows its results in place of the file tree.
    search: Search,
    /// The Quests list, shown in the sidebar in place of the file tree when it is open.
    quests: quests::Quests,
    /// The Roll Tables list, shown in the sidebar in place of the file tree when it is open.
    tables: tables::List,
    /// Which roll tables roll each column on their own.
    table_settings: tables::Settings,
    /// The [`Self::step_table_keys`], so steps that link to a roll table can roll it.
    /// Kept up to date by [`Self::rescan`].
    table_keys: BTreeSet<String>,
    /// The folder the last note made from a roll went in.
    roll_folder: Option<PathBuf>,
    /// The world-building walkthrough, while its tab is open.
    walkthrough: Option<walkthrough_tab::Walkthrough>,
}

/// What the sidebar's search box holds, and the notes it found.
#[derive(Debug, Default)]
struct Search {
    query: String,
    /// The folder searched in (with its subfolders), from a folder's **Search in
    /// Folder**, or `None` for the whole campaign.
    folder: Option<PathBuf>,
    hits: Vec<search::Hit>,
}

impl Search {
    /// Whether the sidebar shows results instead of the file tree.
    fn active(&self) -> bool {
        !self.query.trim().is_empty() || self.folder.is_some()
    }
}

/// The sidebar's search box, for Ctrl+Shift+F.
const SEARCH: &str = "search-notes";
/// At most this many results are shown, so a search for one letter stays quick to draw.
const MAX_HITS: usize = 200;

/// A fix for one of the active note's problems, from the dialog [`Message::FixProblems`] opens.
#[derive(Debug, Clone)]
enum Fix {
    /// Points the links to `target` at the note `to` (a link text) instead.
    Relink { target: String, to: String },
    /// Makes the note `target` names.
    MakeNote(String),
    /// Turns the links to `target` into plain text.
    Unlink(String),
    /// Leaves the links to `target` as they are and stops warning about them.
    Ignore(String),
    /// Moves long text field `key` from the front matter to its `## ` section.
    MoveToSection(String),
    /// Sets the note's `type:` to its folder's type, or removes it (`None`).
    SetType(Option<String>),
    /// Sets choice field `key` to `value`.
    SetOption { key: String, value: String },
    /// Shows the note's Markdown source, to fix it by hand.
    ShowSource,
}

/// What a right-click menu is for.
#[derive(Debug, Clone, PartialEq)]
enum Target {
    /// A note, file or folder in the file tree, or the campaign folder for the space
    /// around the tree.
    Tree(PathBuf),
    Tab(u64),
    /// The text editor that was right-clicked: the note's, a long text field's, or a
    /// block's in the reading view.
    Editor,
    /// The active note's form, outside its long text fields.
    Form,
    Reading,
    /// A table in the Roll Tables list.
    Table(tables::Pick),
}

/// A folder to move something into, in the "Move" dialog.
#[derive(Debug, Clone, PartialEq)]
struct FolderChoice {
    path: PathBuf,
    label: String,
}

impl std::fmt::Display for FolderChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// A File > Save All in progress.
#[derive(Default)]
struct SaveAll {
    /// The notes not saved, or given up on, yet.
    pending: usize,
    saved: usize,
    /// Notes whose save dialog was cancelled.
    skipped: usize,
    /// Why notes were not saved, one line each.
    problems: Vec<String>,
}

/// A dialog over the workspace.
enum Dialog {
    /// Adds a field to the note in the active tab only.
    AddField {
        label: String,
        kind: types::Kind,
        error: Option<String>,
    },
    /// Makes a folder in `parent`, optionally with a content type.
    NewFolder {
        parent: PathBuf,
        name: String,
        template: Template,
        error: Option<String>,
    },
    /// Edit > Types: changes the fields of every folder's type.
    Types(types_editor::Editor),
    /// Edit > Walkthrough Questions: changes the campaign's own walkthrough questions.
    Walkthrough(walkthrough_editor::Editor),
    /// Lists the campaign's files with formatting problems.
    Problems,
    /// Offers fixes for the problems of the note at this path, which is the active tab.
    Fix(PathBuf),
    /// Renames the note or folder at `path`.
    Rename {
        path: PathBuf,
        name: String,
        error: Option<String>,
    },
    /// Moves the note or folder at `path` into another folder.
    Move {
        path: PathBuf,
        folder: Option<FolderChoice>,
        error: Option<String>,
    },
    /// Offers to bring the tag notes up to date with tags changed in another app.
    Tags(Vec<TagChange>),
    /// File > New Roll Table: makes a note with an empty roll table, or one filled with
    /// a list pasted in.
    NewTable {
        name: String,
        folder: FolderChoice,
        die: &'static str,
        columns: Vec<String>,
        /// How many rows, as typed; empty for one per number on the die.
        rows: String,
        pasted: text_editor::Content,
        error: Option<String>,
    },
    /// Asks before putting a roll into a field that has something in it already.
    ReplaceField {
        /// The field's index in the active tab's form.
        index: usize,
        label: String,
        value: String,
        /// Whether it is a long text field, which can take the roll on a new line.
        long: bool,
    },
    /// Makes a note named after a roll table's result, in a folder picked here.
    NoteFromRoll {
        name: String,
        folder: FolderChoice,
        error: Option<String>,
    },
}

/// How a note's tags differ from its tag notes, after its tags were changed in another app.
#[derive(Debug, Clone)]
struct TagChange {
    note: PathBuf,
    /// The tags (with their tag folders) whose notes don't list it yet.
    add: Vec<(String, String)>,
    /// The tag notes that list it, though it no longer has the tag.
    remove: Vec<(String, String)>,
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
    /// In the walkthrough: buttons that make the note named in one of the folders its
    /// answer links to, as `(folder, name, label)`.
    make: Vec<(PathBuf, String, String)>,
}

/// A content type a new folder can start with.
#[derive(Debug, Clone, PartialEq)]
struct Template {
    label: String,
    /// The `_type.yaml` to write, or `None` for a folder of plain notes (or, in a typed
    /// folder, notes of its type).
    yaml: Option<String>,
    /// Whether the folder is made a kind of the type of the folder it is in, with a
    /// `_type.yaml` named after it.
    kind_of: bool,
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
            kind_of: false,
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
    /// The formatted note, while the reading view shows instead of the form or source.
    preview: Option<Preview>,
    /// What is being edited in the reading view, if anything.
    edit: Option<ReadEdit>,
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

/// Something being edited in place in the reading view.
enum ReadEdit {
    /// Block `index` of the preview (or new text at the end, when `index` is the number of
    /// blocks), which is at `range` in the note's text. Its text goes back into the note
    /// when editing it ends.
    Block {
        index: usize,
        range: std::ops::Range<usize>,
        content: text_editor::Content,
    },
    /// The field with this front matter key, edited straight in the form.
    Field(String),
    /// The roll table that is block `index`, in the table editor.
    Table {
        index: usize,
        editor: Box<table_editor::Editor>,
        /// For a table just added: the note's text before it, and whether it had unsaved
        /// changes, for Cancel to put back.
        added: Option<(String, bool)>,
    },
}

impl Tab {
    fn name(&self, show_extensions: bool) -> String {
        match (&self.path, &self.form) {
            (Some(path), _) => display_name(path, show_extensions),
            (None, Some(form)) => format!("New {}", form.ty.label()),
            (None, None) => "Untitled".into(),
        }
    }

    /// Whether block `index` of the reading view is a roll table.
    fn is_table_block(&self, index: usize) -> bool {
        let Some(preview) = &self.preview else {
            return false;
        };
        preview.block_range(index).is_some_and(|range| {
            tables::find(preview.source())
                .iter()
                .any(|table| table.range.start == range.start)
        })
    }

    /// Whether the form shows, rather than the Markdown source or the reading view.
    fn shows_form(&self) -> bool {
        self.form.is_some() && !self.source && !self.reading()
    }

    /// Whether the reading view shows the note formatted, instead of an editor.
    fn reading(&self) -> bool {
        self.preview.is_some()
    }

    /// Replaces the note's text, reading it into the form again if it shows one.
    fn set_text(&mut self, text: &str) {
        match &self.form {
            Some(form) if !self.source => match note::parse(text) {
                Ok(note) => self.form = Some(Form::from_note(form.ty.clone(), note)),
                // Text the form can't read stays as it is, in the source.
                Err(_) => {
                    self.content = text_editor::Content::with_text(text);
                    self.source = true;
                }
            },
            _ => self.content = text_editor::Content::with_text(text),
        }
    }

    /// The note formatted for the reading view.
    fn make_preview(&self, root: &Path, index: &links::Index) -> Preview {
        let text = self.text().unwrap_or_else(|_| self.content.text());
        let ty = self.form.as_ref().map(|form| &form.ty);
        Preview::new(&text, ty, &self.note_path(), root, index)
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

/// A key that moves about an open menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arrow {
    Up,
    Down,
    Left,
    Right,
    Enter,
}

#[derive(Debug, Clone)]
enum Message {
    // Menus and app-wide settings
    MenuOpened(Menu),
    MenuHovered(Menu),
    /// Alt and a menu's first letter: opens it, ready for the arrow keys.
    MenuKey(Menu),
    /// The mouse moved onto a line of the open menu: opens its submenu, if it has one.
    SubmenuHovered(Option<usize>),
    /// An arrow key, Enter or Space while a menu is open.
    MenuArrow(Arrow),
    CloseMenu,
    /// A menu item or its keyboard shortcut.
    Run(Command),
    /// Campaign > Export Campaign Structure...
    ExportStructure,
    Escape,
    ThemePicked(String),
    ReloadThemes,
    ToggleExtensions(bool),
    ToggleTypeFiles(bool),
    ToggleToolbar(bool),
    ShowAbout,
    CloseAbout,
    /// Opens the Help window at a page.
    ShowHelp(help::Page),
    Help(help::Message),
    /// Opens a web page from the Help menu.
    OpenWeb(&'static str),
    Exit,
    ExitConfirmed(bool),

    // Splash screen
    /// Lists the campaigns opened before, or asks for a folder if there are none.
    OpenCampaign,
    /// Asks for a campaign folder to open.
    BrowseCampaign,
    CampaignPicked(Option<PathBuf>),
    /// Opens a campaign from the list of those opened before.
    OpenKnown(PathBuf),
    /// Takes a campaign off the list of those opened before.
    ForgetCampaign(PathBuf),
    OpenFilter(String),
    /// Enter in the "Open campaign" dialog: opens the first campaign it shows.
    OpenFirstCampaign,
    CancelOpenCampaign,
    ShowNewCampaign,
    CancelNewCampaign,
    TitleChanged(String),
    LocationChanged(String),
    BrowseLocation,
    LocationPicked(Option<PathBuf>),
    TemplatePicked(templates::Template),
    /// New Campaign's Start the walkthrough box.
    ToggleNewWalkthrough(bool),
    /// Asks for a template (`.zip`) or type (`.yaml`) file to import.
    ImportTemplate,
    ImportPicked(Option<PathBuf>),
    ShowTemplatesFolder,
    CreateCampaign,

    // Workspace
    ToggleFolder(PathBuf),
    NewNoteIn(PathBuf),
    Form(form::Message),
    ToggleSource,
    /// Edits the active tab's note in its Markdown source (true) or its form (false).
    SetSource(bool),
    /// Switches the active tab between the reading view and editing.
    ToggleReading,
    /// Shows the active tab in the reading view (true) or an editor (false).
    SetReading(bool),
    ToggleOpenInReading(bool),
    /// A link in the reading view was clicked.
    PreviewLink(String),
    /// A task box in the reading view was clicked; its `[ ]` is here in the note's text.
    ToggleTask(usize),
    /// Edits block `index` of the reading view in place.
    EditBlock(usize),
    BlockEdit(text_editor::Action),
    /// Edits the field with this front matter key in the reading view.
    EditField(String),
    /// Adds text at the end of the note in the reading view.
    AddText,
    /// The mouse went over this block of the reading view.
    HoverBlock(usize),
    /// Adds a roll table after the block the reading view's right-click menu was opened
    /// on, and opens the table editor on it.
    AddTableHere,
    /// Tables > Add Roll Table Here: at the end of the note in the reading view.
    AddTableAtEnd,
    /// Puts what was edited in the reading view back into the note.
    FinishEditing,
    Modifiers(Modifiers),
    /// The mouse moved onto a row of the file tree.
    TreeEnter(PathBuf),
    TreeExit(PathBuf),
    MousePressed,
    MouseReleased,
    /// Finishes the link being typed with this link text.
    CompleteLink(String),
    /// Tab (or Shift+Tab, true) in the form: moves to the next (or the one before) field.
    TabField(bool),
    /// Tab moved the focus to this input of the form's [`form::Form::tab_ids`].
    FieldTabbed(usize),
    /// Opens the note a link points to, creating it if there is none.
    FollowLink(String),
    InsertLink,
    /// Formats the text being typed in, from the formatting toolbar or Ctrl+B and Ctrl+I.
    Format(format::Command),
    /// Asks for a picture to add to the note.
    InsertPicture,
    /// Copies this picture into the campaign and embeds it in the note.
    AddPicture(Option<PathBuf>),
    /// A picture from the web finished downloading (or failed to).
    WebPicture(String, web::Picture),
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
    /// Brings the tag notes up to date with the tags changed in another app, as the
    /// dialog offering it lists them.
    UpdateTagNotes,
    ShowTypes,
    Types(types_editor::Message),
    /// Edit > Walkthrough Questions.
    ShowWalkthroughEditor,
    WalkthroughEditor(walkthrough_editor::Message),
    /// A type or the campaign's structure was exported, to here (or not, if cancelled).
    Exported(Result<Option<PathBuf>, String>),
    TypesImportPicked(Option<PathBuf>),
    ShowProblems,
    OpenProblem(PathBuf),
    /// Opens the note and a dialog offering fixes for its problems.
    FixProblems(PathBuf),
    Fix(Fix),
    WindowFocused,
    /// Files in the campaign folder changed, here or in another app.
    FilesChanged,
    ReloadFromDisk,
    KeepMine,
    OpenEntry(PathBuf),
    RefreshTree,
    ExpandAll,
    CollapseAll,
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
    SaveAll,
    Saved(u64, Result<Option<PathBuf>, String>),
    CloseActiveTab,
    ToggleDice,
    Dice(dice::Message),
    /// Rolls dice from a note's field: the field's label, and the dice.
    RollField(String, String),

    // Right-click menus
    WindowResized(Size),
    OpenContextMenu(Target, Point),
    CloseContextMenu,
    Cut,
    Copy,
    Paste,
    Pasted(Option<String>),
    SelectAll,
    /// Opens a note in the reading view.
    OpenRead(PathBuf),
    ShowNewFolderIn(PathBuf),
    /// Opens Edit > Types at this folder's type.
    EditType(PathBuf),
    ShowRename(PathBuf),
    RenameName(String),
    Rename,
    ShowMove(PathBuf),
    MoveFolder(FolderChoice),
    Move,
    Delete(PathBuf),
    DeleteConfirmed(PathBuf, bool),
    /// Copies a `[[link]]` to this note.
    CopyLink(PathBuf),
    ShowInFileManager(PathBuf),
    /// Expands the folders down to this file in the file tree.
    RevealInTree(PathBuf),
    SaveTab(u64),
    CloseTabs(Vec<u64>),
    CloseTabsConfirmed(Vec<u64>, bool),

    // Search
    SearchChanged(String),
    /// Searches only this folder and its subfolders.
    SearchIn(PathBuf),
    /// Searches the whole campaign again.
    SearchEverywhere,
    ClearSearch,
    // The world-building walkthrough
    /// Opens the walkthrough in its tab, or shows the tab.
    ShowWalkthrough,
    Walkthrough(walkthrough_tab::Message),
    // Quests
    ShowQuests,
    Quests(quests::Message),
    // Roll tables
    ShowTables,
    Tables(tables::Message),
    /// The reading view's Roll button above the roll table at this place in the note.
    RollTable(usize),
    /// Remembers the table picked in an Inspire Me dropdown: where (see
    /// [`form::pick_key`]), and the table.
    PickTable(String, String),
    /// Edit Table on the roll table that is block `.0` of the reading view.
    EditTable(usize),
    TableEditor(table_editor::Message),
    /// The reading view's roll button of the field with this key, which rolls on its table.
    RollIntoField(String, String),
    /// Puts the roll waiting in the Replace dialog into its field: in place of what is
    /// there, or on a new line after it (true).
    FillField(bool),
    /// The reading view's toggle for rolling each column of the table at this place.
    TableEachColumn(usize, bool),
    /// Opens New Roll Table, in this folder if given.
    ShowNewTable(Option<PathBuf>),
    NewTableName(String),
    NewTableFolder(FolderChoice),
    NewTableDie(&'static str),
    NewTableColumn(usize, String),
    NewTableAddColumn,
    NewTableRemoveColumn(usize),
    NewTableRows(String),
    NewTablePasted(text_editor::Action),
    CreateTable,
    NoteFromRollName(String),
    NoteFromRollFolder(FolderChoice),
    CreateNoteFromRoll,
    /// Ctrl+Shift+F: puts the cursor in the search box.
    FocusSearch,
    /// Enter in the search box: opens the first note found.
    OpenFirstHit,
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
                open_campaign: None,
                error: theme_problems(&problems),
            },
            settings,
            themes,
            theme: 0,
            logos: (
                svg::Handle::from_memory(LOGO.as_bytes()),
                svg::Handle::from_memory(LOGO.as_bytes()),
            ),
            dice: dice::Icons::new(theme::built_in()[0].colors),
            menu: None,
            menu_cursor: menu::Cursor::default(),
            after_close: None,
            about: false,
            help: None,
            window: window_settings().size,
        };
        let name = app.settings.theme.clone();
        app.use_theme(&name);

        // `dungeon_elements <campaign folder>` opens that campaign right away.
        if let Some(root) = std::env::args_os().nth(1).map(PathBuf::from)
            && root.is_dir()
        {
            app.open_workspace(root);
        }

        app
    }

    /// Opens the campaign at `root`, and puts it at the top of Open Campaign's list.
    fn open_workspace(&mut self, root: PathBuf) {
        // Not being able to remember it doesn't stop it opening.
        let _ = recent::remember(&root);
        self.screen = Screen::Workspace(Box::new(Workspace::open(
            root,
            self.settings.open_in_reading,
        )));
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
        self.dice = dice::Icons::new(c);
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

    /// Opens the Help window if it is closed, and passes it `message`.
    fn update_help(&mut self, message: help::Message) -> Task<Message> {
        let help = self
            .help
            .get_or_insert_with(|| help::Help::new(help::Page::Shortcuts));
        match help.update(message) {
            Some(help::Action::Scroll) => iced::widget::operation::snap_to(
                help::PAGE,
                iced::widget::scrollable::RelativeOffset::START,
            ),
            Some(help::Action::Close) => {
                self.help = None;
                Task::none()
            }
            Some(help::Action::Open(url)) => {
                if let Err(error) = open_in_browser(&url) {
                    self.notify(format!("Could not open {url}: {error}"));
                }
                Task::none()
            }
            None => Task::none(),
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
                keyboard::Event::KeyPressed { key, modifiers, .. } => escape(&key)
                    .or_else(|| help_key(&key, modifiers))
                    .or_else(|| menu_key(&key, modifiers)),
                _ => None,
            }),
            Screen::Workspace(_) => keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed { key, modifiers, .. } => escape(&key)
                    .or_else(|| help_key(&key, modifiers))
                    .or_else(|| shortcut(&key, modifiers))
                    .or_else(|| tab_key(&key, modifiers)),
                keyboard::Event::ModifiersChanged(modifiers) => Some(Message::Modifiers(modifiers)),
                _ => None,
            }),
        };

        // The arrow keys move about an open menu.
        let menu_keys = if self.menu.is_some() {
            keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed { key, .. } => arrow_key(&key),
                _ => None,
            })
        } else {
            Subscription::none()
        };

        // Coming back to the window is when files may have been edited in another app.
        // Mouse buttons are watched too, for dragging notes in the file tree.
        let focus = iced::event::listen_with(|event, _, _| match event {
            iced::Event::Window(window::Event::Focused) => Some(Message::WindowFocused),
            iced::Event::Window(
                window::Event::Resized(size) | window::Event::Opened { size, .. },
            ) => Some(Message::WindowResized(size)),
            iced::Event::Window(window::Event::FileDropped(path)) => {
                Some(Message::AddPicture(Some(path)))
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                Some(Message::MousePressed)
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                Some(Message::MouseReleased)
            }
            _ => None,
        });

        let files = match &self.screen {
            Screen::Workspace(workspace) => {
                Subscription::run_with(workspace.root.clone(), watch::changes)
                    .map(|()| Message::FilesChanged)
            }
            Screen::Splash { .. } => Subscription::none(),
        };

        // The search box keeps Escape to itself; during a search, Escape still leaves it.
        let searching = match &self.screen {
            Screen::Workspace(workspace) if workspace.search.active() => {
                iced::event::listen_with(|event, status, _| match event {
                    iced::Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                        if status == iced::event::Status::Captured =>
                    {
                        escape(&key)
                    }
                    _ => None,
                })
            }
            _ => Subscription::none(),
        };

        Subscription::batch([
            keys,
            menu_keys,
            focus,
            files,
            searching,
            window::close_requests().map(|_| Message::Exit),
        ])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MenuOpened(menu) => {
                if let Screen::Workspace(workspace) = &mut self.screen {
                    workspace.context = None;
                }
                self.menu = if self.menu == Some(menu) {
                    None
                } else {
                    Some(menu)
                };
                self.menu_cursor = menu::Cursor::default();
                return Task::none();
            }
            Message::MenuHovered(menu) => {
                if self.menu.is_some() && self.menu != Some(menu) {
                    self.menu = Some(menu);
                    self.menu_cursor = menu::Cursor::default();
                }
                return Task::none();
            }
            Message::MenuKey(menu) => {
                if !self.can_open(menu) {
                    return Task::none();
                }
                if let Screen::Workspace(workspace) = &mut self.screen {
                    workspace.context = None;
                }
                if self.menu == Some(menu) {
                    self.menu = None;
                    return Task::none();
                }
                self.menu = Some(menu);
                let entries = self.menu_entries(menu);
                self.menu_cursor = menu::Cursor {
                    line: menu::step(&entries, None, true),
                    ..menu::Cursor::default()
                };
                // The text box being typed in would take the arrow keys for itself.
                return iced::advanced::widget::operate(
                    iced::advanced::widget::operation::focusable::unfocus(),
                );
            }
            Message::SubmenuHovered(line) => {
                if let Some(open) = self.menu {
                    let entries = self.menu_entries(open);
                    let line = line.filter(|line| menu::is_submenu(&entries, *line));
                    if self.menu_cursor.submenu != line {
                        self.menu_cursor.submenu = line;
                        self.menu_cursor.sub_line = None;
                    }
                }
                return Task::none();
            }
            Message::MenuArrow(arrow) => {
                let Some(open) = self.menu else {
                    return Task::none();
                };
                return self.menu_arrow(open, arrow);
            }
            Message::Run(command) => {
                return match self.command(command).0 {
                    Some(message) => self.update(message),
                    None => Task::none(),
                };
            }
            _ => {}
        }

        // Anything else, such as picking a menu item, closes the open menu. Messages that
        // only track the mouse and keyboard don't: the click that opens a menu sends one.
        let tracking = matches!(
            message,
            Message::Modifiers(_)
                | Message::TreeEnter(_)
                | Message::TreeExit(_)
                | Message::MousePressed
                | Message::MouseReleased
                | Message::WindowResized(_)
        );
        let mut closed = false;
        if !tracking {
            closed = self.menu.take().is_some();
            self.menu_cursor = menu::Cursor::default();
            if let Screen::Workspace(workspace) = &mut self.screen {
                closed |= workspace.context.take().is_some();
            }
        }

        match message {
            Message::CloseMenu | Message::CloseContextMenu => return Task::none(),
            Message::WindowResized(size) => {
                self.window = size;
                return Task::none();
            }
            // Escape closes an open menu, and only that.
            Message::Escape if closed => return Task::none(),
            // Tab moves between the form's fields, not under the Help or About window.
            Message::TabField(_) if self.help.is_some() || self.about => return Task::none(),
            Message::Escape if self.help.is_some() => {
                self.help = None;
                return Task::none();
            }
            Message::Escape => {
                match &mut self.screen {
                    Screen::Splash {
                        new_campaign,
                        open_campaign,
                        ..
                    } if !self.about => {
                        *new_campaign = None;
                        *open_campaign = None;
                    }
                    Screen::Workspace(workspace) if !self.about => {
                        let editing = workspace.active_tab().is_some_and(|tab| tab.edit.is_some());
                        // Escape leaves a search, once there is nothing else to close.
                        if workspace.dialog.take().is_none() {
                            if workspace.search.active() && !editing {
                                workspace.search = Search::default();
                            } else if workspace.quests.open && !editing {
                                workspace.quests.open = false;
                            } else if workspace.tables.open && !editing {
                                workspace.tables.open = false;
                            } else {
                                workspace.finish_editing();
                            }
                        }
                    }
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
            Message::ToggleToolbar(show) => {
                self.settings.show_toolbar = show;
                self.save_settings();
                return Task::none();
            }
            Message::ToggleOpenInReading(on) => {
                self.settings.open_in_reading = on;
                self.save_settings();
                if let Screen::Workspace(workspace) = &mut self.screen {
                    workspace.open_in_reading = on;
                }
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
            Message::ShowHelp(page) => {
                self.about = false;
                return self.update_help(help::Message::Show(page));
            }
            Message::Help(message) => return self.update_help(message),
            Message::OpenWeb(url) => {
                if let Err(error) = open_in_browser(url) {
                    self.notify(format!("Could not open {url}: {error}"));
                }
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
                open_campaign,
                error,
            } => match message {
                Message::OpenCampaign => {
                    *error = None;
                    let campaigns = recent::load();
                    // With none to list yet, the folder picker is all there is.
                    if campaigns.is_empty() {
                        return self.update(Message::BrowseCampaign);
                    }
                    *open_campaign = Some(OpenCampaign {
                        campaigns,
                        filter: String::new(),
                    });
                    iced::widget::operation::focus(OPEN_FILTER)
                }
                Message::BrowseCampaign => {
                    Task::perform(pick_folder("Open campaign folder"), Message::CampaignPicked)
                }
                Message::CampaignPicked(Some(root)) => {
                    if root.is_dir() {
                        self.open_workspace(root);
                    } else {
                        *error = Some(format!("{} is not a folder", root.display()));
                    }
                    Task::none()
                }
                Message::OpenKnown(root) => {
                    if root.is_dir() {
                        self.open_workspace(root);
                    } else {
                        *error = Some(format!(
                            "{} can't be found. It may have been moved, renamed or deleted, or be on a drive that isn't connected.",
                            root.display()
                        ));
                        *open_campaign = None;
                    }
                    Task::none()
                }
                Message::ForgetCampaign(root) => {
                    if let Err(message) = recent::forget(&root) {
                        *error = Some(format!("Could not update the list: {message}"));
                    }
                    if let Some(open) = open_campaign {
                        open.campaigns.retain(|campaign| campaign.path != root);
                    }
                    Task::none()
                }
                Message::OpenFilter(value) => {
                    if let Some(open) = open_campaign {
                        open.filter = value;
                    }
                    Task::none()
                }
                Message::OpenFirstCampaign => {
                    match open_campaign.as_ref().and_then(|open| open.shown().next()) {
                        Some(campaign) => {
                            let root = campaign.path.clone();
                            self.update(Message::OpenKnown(root))
                        }
                        None => Task::none(),
                    }
                }
                Message::CancelOpenCampaign => {
                    *open_campaign = None;
                    Task::none()
                }
                Message::ShowNewCampaign => {
                    *error = None;
                    *open_campaign = None;
                    let mut form = NewCampaign {
                        title: String::new(),
                        location: default_location()
                            .map(|path| path.display().to_string())
                            .unwrap_or_default(),
                        templates: templates::list(),
                        template: templates::Template::Starter,
                        structure: Ok(templates::Structure::default()),
                        walkthrough: true,
                        error: None,
                    };
                    form.pick(templates::Template::Starter);
                    *new_campaign = Some(Box::new(form));
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
                Message::ImportTemplate => Task::perform(pick_import(), Message::ImportPicked),
                Message::ShowTemplatesFolder => {
                    if let Some(folder) = templates::templates_folder() {
                        let _ = std::fs::create_dir_all(&folder);
                        if let Err(message) = open_folder(&folder) {
                            *error =
                                Some(format!("Could not open {}: {message}", folder.display()));
                        }
                    }
                    Task::none()
                }
                Message::CreateCampaign => {
                    let Some(form) = new_campaign else {
                        return Task::none();
                    };
                    let structure = match &form.structure {
                        Ok(structure) => structure,
                        Err(message) => {
                            form.error = Some(message.clone());
                            return Task::none();
                        }
                    };

                    let walkthrough = form.walkthrough && form.has_walkthrough();
                    match campaign::create(Path::new(form.location.trim()), &form.title, structure)
                    {
                        Ok(root) => {
                            self.open_workspace(root);
                            if walkthrough && let Screen::Workspace(workspace) = &mut self.screen {
                                workspace.show_walkthrough();
                                return workspace.focus_walkthrough();
                            }
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
                            Message::TemplatePicked(template) => form.pick(template),
                            Message::ToggleNewWalkthrough(on) => form.walkthrough = on,
                            Message::ImportPicked(Some(file)) => match templates::import(&file) {
                                Ok(templates::Imported::Template(name, _)) => {
                                    form.templates = templates::list();
                                    if let Some(template) = form
                                        .templates
                                        .iter()
                                        .find(|template| template.to_string() == name)
                                        .cloned()
                                    {
                                        form.pick(template);
                                    }
                                }
                                Ok(templates::Imported::Type(label)) => {
                                    form.error = Some(format!(
                                        "Saved the type {label}. New folders can start with it, in any campaign."
                                    ));
                                    return Task::none();
                                }
                                Err(message) => {
                                    form.error = Some(format!("Could not import it: {message}"));
                                    return Task::none();
                                }
                            },
                            _ => {}
                        }
                        form.error = None;
                    }
                    Task::none()
                }
            },
            Screen::Workspace(workspace) => {
                match message {
                    Message::CloseCampaignConfirmed(true) => {
                        self.screen = Screen::default();
                        return match self.after_close.take() {
                            Some(next) => self.update(next),
                            None => Task::none(),
                        };
                    }
                    Message::CloseCampaignConfirmed(false) => {
                        self.after_close = None;
                        return Task::none();
                    }
                    // A new or other campaign takes this one's place, once it has closed.
                    Message::ShowNewCampaign | Message::OpenCampaign => {
                        self.after_close = Some(message);
                        return self.update(Message::CloseCampaign);
                    }
                    _ => {}
                }
                let edit = !tracking;
                let task = workspace.update(message);
                if edit {
                    workspace.recheck_active();
                    if workspace.active_tab().is_some_and(|tab| tab.modified) {
                        workspace.refresh_quests();
                        workspace.refresh_tables();
                    }
                }
                workspace.refresh_preview();
                workspace.set_form_tables();
                Task::batch([task, workspace.fetch_pictures()])
            }
        }
    }

    fn view<'a>(&'a self) -> Element<'a, Message> {
        let c = self.colors();

        let bar = menu::bar(
            c,
            &self.logos.0,
            self.menu,
            |menu| self.can_open(menu),
            |menu| match menu {
                Menu::Maps => Some("Maps are coming with the map editor"),
                Menu::Campaign | Menu::Tables => Some("Open a campaign first"),
                _ => None,
            },
            Message::MenuOpened,
            Message::MenuHovered,
        );

        let body = match &self.screen {
            Screen::Splash { error, .. } => splash(c, &self.logos.1, error.as_deref()),
            Screen::Workspace(workspace) => workspace.view(c, &self.settings, &self.dice),
        };

        let edge = rule::horizontal(c.border_width.max(1.0)).style(style::separator(c));
        let view = column![bar, edge, body];

        let workspace = match &self.screen {
            Screen::Workspace(workspace) => Some(workspace.as_ref()),
            Screen::Splash { .. } => None,
        };
        let dialog = match &self.screen {
            Screen::Splash {
                new_campaign: Some(form),
                ..
            } => Some(modal(new_campaign_dialog(c, form))),
            Screen::Splash {
                open_campaign: Some(open),
                ..
            } => Some(modal(open_campaign_dialog(c, open))),
            Screen::Workspace(workspace) => workspace
                .dialog
                .as_ref()
                .map(|dialog| modal(workspace.dialog_view(c, dialog))),
            Screen::Splash { .. } => None,
        };
        let about = self.about.then(|| modal(about_dialog(c, &self.logos.1)));
        let help = self.help.as_ref().map(|help| {
            modal(
                container(help.view(c).map(Message::Help))
                    .padding(24)
                    .center(Fill)
                    .into(),
            )
        });
        let context = workspace.and_then(|workspace| {
            let (target, position) = workspace.context.as_ref()?;
            Some(context_menu::view(
                c,
                *position,
                self.window,
                workspace.context_entries(target),
                Message::CloseContextMenu,
            ))
        });
        let dropdown = self.menu.map(|open| {
            menu::dropdown(
                c,
                open,
                self.menu_entries(open),
                self.menu_cursor,
                Message::CloseMenu,
                Message::SubmenuHovered,
            )
        });

        // Every layer is always there, empty when closed, so opening one doesn't make
        // iced build the window again from scratch, which would lose the focus, the
        // selection and how far things are scrolled.
        let layer = |layer: Option<Element<'a, Message>>| layer.unwrap_or_else(|| space().into());
        stack![
            view,
            layer(dialog),
            layer(about),
            layer(help),
            layer(context),
            layer(dropdown),
        ]
        .into()
    }

    /// Whether `menu` can be opened now: Maps waits for the map editor, and Campaign
    /// and Tables for a campaign.
    fn can_open(&self, menu: Menu) -> bool {
        match menu {
            Menu::Maps => false,
            Menu::Campaign | Menu::Tables => matches!(self.screen, Screen::Workspace(_)),
            _ => true,
        }
    }

    /// What `command` does now, whether it can be picked from a menu now, and whether it
    /// shows a tick. The keyboard runs the message even when the menu greys the item out,
    /// so a shortcut can still say why it did nothing.
    fn command(&self, command: Command) -> (Option<Message>, bool, bool) {
        let workspace = match &self.screen {
            Screen::Workspace(workspace) => Some(workspace.as_ref()),
            Screen::Splash { .. } => None,
        };
        let open = workspace.is_some();
        let active_tab = workspace.and_then(Workspace::active_tab);
        let has_tab = active_tab.is_some();
        let has_unsaved =
            workspace.is_some_and(|workspace| workspace.tabs.iter().any(|tab| tab.modified));
        let has_form = active_tab.is_some_and(|tab| tab.form.is_some());
        let is_note = active_tab.is_some_and(Tab::is_note);
        let reading = active_tab.is_some_and(Tab::reading);
        let text = workspace.and_then(Workspace::focused_text);
        let selected = text.is_some_and(|text| text.selection().is_some());
        let can_link = active_tab.is_some_and(|tab| {
            tab.is_note()
                && !tab.reading()
                && (!tab.shows_form()
                    || tab.form.as_ref().is_some_and(|form| form.focus().is_some()))
        });
        let shows_source =
            active_tab.is_some_and(|tab| tab.form.is_some() && tab.source && !tab.reading());
        let shows_form = active_tab.is_some_and(Tab::shows_form);
        let quests = workspace.is_some_and(|workspace| workspace.quests.open);
        let tables = workspace.is_some_and(|workspace| workspace.tables.open);
        let dice = workspace.is_some_and(|workspace| workspace.dice.open);
        let settings = &self.settings;

        // Most commands need a campaign open; these are the message and whether it can
        // be picked, then the tick.
        let (message, enabled, checked) = match command {
            Command::NewNote => (Message::NewNote, open, false),
            Command::NewFolder => (Message::ShowNewFolder, open, false),
            Command::OpenFile => (Message::OpenFile, open, false),
            Command::Save => (Message::Save, has_tab, false),
            Command::SaveAs => (Message::SaveAs, has_tab, false),
            Command::SaveAll => (Message::SaveAll, has_unsaved, false),
            Command::CloseTab => (Message::CloseActiveTab, has_tab, false),
            Command::CloseCampaign => (Message::CloseCampaign, open, false),
            Command::NewCampaign => (Message::ShowNewCampaign, true, false),
            Command::OpenCampaign => (Message::OpenCampaign, true, false),
            Command::Exit => (Message::Exit, true, false),
            Command::Cut => (Message::Cut, selected, false),
            Command::Copy => (Message::Copy, selected, false),
            Command::Paste => (Message::Paste, text.is_some(), false),
            Command::SelectAll => (Message::SelectAll, text.is_some(), false),
            Command::InsertLink => (Message::InsertLink, can_link, false),
            Command::InsertPicture => (Message::InsertPicture, is_note, false),
            Command::AddField => (Message::ShowAddField, shows_form, false),
            Command::FindInNotes => (Message::FocusSearch, open, false),
            Command::ReadingView => (Message::ToggleReading, is_note, reading),
            Command::EditAsForm => (Message::SetSource(false), has_form, shows_form),
            Command::EditAsMarkdown => (Message::ToggleSource, has_form, shows_source),
            Command::Quests => (
                if quests {
                    Message::Quests(quests::Message::Close)
                } else {
                    Message::ShowQuests
                },
                open,
                quests,
            ),
            Command::RollTables => (
                if tables {
                    Message::Tables(tables::Message::Close)
                } else {
                    Message::ShowTables
                },
                open,
                tables,
            ),
            Command::DiceRoller => (Message::ToggleDice, open, dice),
            Command::FormattingToolbar => (
                Message::ToggleToolbar(!settings.show_toolbar),
                true,
                settings.show_toolbar,
            ),
            Command::ShowExtensions => (
                Message::ToggleExtensions(!settings.show_extensions),
                true,
                settings.show_extensions,
            ),
            Command::ShowTypeFiles => (
                Message::ToggleTypeFiles(!settings.show_type_files),
                true,
                settings.show_type_files,
            ),
            Command::OpenInReading => (
                Message::ToggleOpenInReading(!settings.open_in_reading),
                true,
                settings.open_in_reading,
            ),
            Command::ReloadThemes => (Message::ReloadThemes, true, false),
            Command::RefreshTree => (Message::RefreshTree, open, false),
            Command::ExpandAll => (Message::ExpandAll, open, false),
            Command::CollapseAll => (Message::CollapseAll, open, false),
            Command::Walkthrough => (Message::ShowWalkthrough, open, false),
            Command::WalkthroughQuestions => (Message::ShowWalkthroughEditor, open, false),
            Command::NoteTypes => (Message::ShowTypes, open, false),
            Command::ExportStructure => (Message::ExportStructure, open, false),
            Command::ShowCampaignFolder => match workspace {
                Some(workspace) => (
                    Message::ShowInFileManager(workspace.root.clone()),
                    true,
                    false,
                ),
                None => return (None, false, false),
            },
            Command::NewRollTable => (Message::ShowNewTable(None), open, false),
            Command::AddRollTableHere => (Message::AddTableAtEnd, is_note && reading, false),
            Command::HowTo => (Message::ShowHelp(help::Page::Guide(0)), true, false),
            Command::Shortcuts => (Message::ShowHelp(help::Page::Shortcuts), true, false),
            Command::WhatsNew => (Message::ShowHelp(help::Page::WhatsNew), true, false),
            Command::ReportProblem => (Message::OpenWeb(help::ISSUES_URL), true, false),
            Command::GitHub => (Message::OpenWeb(help::PROJECT_URL), true, false),
            Command::About => (Message::ShowAbout, true, false),
        };
        (Some(message), enabled, checked)
    }

    /// The menu line for `command`, greyed out when it can't be picked now.
    fn command_entry(&self, command: Command) -> menu::Entry<Message> {
        let (message, enabled, checked) = self.command(command);
        let mut entry = menu::item(command.label(), message.filter(|_| enabled)).checked(checked);
        if let Some(keys) = command.shortcut() {
            entry = entry.shortcut(keys);
        }
        entry
    }

    fn menu_entries(&self, open: Menu) -> Vec<menu::Entry<Message>> {
        use menu::Entry;

        enum Line {
            Do(Command),
            Gap,
        }
        use Command::*;
        use Line::{Do, Gap};

        let lines = |lines: &[Line]| -> Vec<Entry<Message>> {
            lines
                .iter()
                .map(|line| match line {
                    Do(command) => self.command_entry(*command),
                    Gap => Entry::Separator,
                })
                .collect()
        };

        match open {
            Menu::File => lines(&[
                Do(NewNote),
                Do(NewFolder),
                Do(OpenFile),
                Gap,
                Do(Save),
                Do(SaveAs),
                Do(SaveAll),
                Gap,
                Do(CloseTab),
                Do(CloseCampaign),
                Gap,
                Do(NewCampaign),
                Do(OpenCampaign),
                Gap,
                Do(Exit),
            ]),
            Menu::Edit => lines(&[
                Do(Cut),
                Do(Copy),
                Do(Paste),
                Do(SelectAll),
                Gap,
                Do(InsertLink),
                Do(InsertPicture),
                Do(AddField),
                Gap,
                Do(FindInNotes),
            ]),
            Menu::View => {
                let mut entries = lines(&[
                    Do(ReadingView),
                    Do(EditAsForm),
                    Do(EditAsMarkdown),
                    Gap,
                    Do(Quests),
                    Do(RollTables),
                    Do(DiceRoller),
                    Gap,
                    Do(FormattingToolbar),
                    Do(ShowExtensions),
                    Do(ShowTypeFiles),
                    Do(OpenInReading),
                    Gap,
                ]);
                let mut themes: Vec<Entry<Message>> = self
                    .themes
                    .iter()
                    .enumerate()
                    .map(|(index, theme)| {
                        menu::item(&theme.name, Some(Message::ThemePicked(theme.name.clone())))
                            .checked(index == self.theme)
                    })
                    .collect();
                themes.push(Entry::Separator);
                themes.push(self.command_entry(ReloadThemes));
                entries.push(menu::submenu("Theme", themes));
                entries.push(menu::submenu(
                    "File Tree",
                    lines(&[Do(RefreshTree), Do(ExpandAll), Do(CollapseAll)]),
                ));
                entries
            }
            Menu::Campaign => lines(&[
                Do(Walkthrough),
                Do(WalkthroughQuestions),
                Gap,
                Do(NoteTypes),
                Do(ExportStructure),
                Gap,
                Do(ShowCampaignFolder),
            ]),
            Menu::Tables => {
                let mut entries = lines(&[
                    Do(RollTables),
                    Do(DiceRoller),
                    Gap,
                    Do(NewRollTable),
                    Do(AddRollTableHere),
                    Gap,
                ]);
                // Picking a built-in table rolls it, as its die does in the Roll Tables list.
                entries.extend(tables::Group::ALL.into_iter().map(|group| {
                    let tables = tables::BUILT_IN
                        .into_iter()
                        .filter(|table| table.group == group)
                        .map(|table| {
                            menu::item(
                                table.name,
                                Some(Message::Tables(tables::Message::RollBuiltIn(
                                    table.name.to_string(),
                                ))),
                            )
                        })
                        .collect();
                    menu::submenu(group.label(), tables)
                }));
                entries
            }
            Menu::Maps => Vec::new(),
            Menu::Help => lines(&[
                Do(HowTo),
                Do(Shortcuts),
                Do(WhatsNew),
                Gap,
                Do(ReportProblem),
                Do(GitHub),
                Gap,
                Do(About),
            ]),
        }
    }

    /// Moves about the open menu with the arrow keys, or picks the line they are on.
    fn menu_arrow(&mut self, open: Menu, arrow: Arrow) -> Task<Message> {
        let mut entries = self.menu_entries(open);
        let cursor = self.menu_cursor;
        let sub = cursor
            .submenu
            .and_then(|line| menu::submenu_entries(&mut entries, line));

        match (arrow, sub) {
            (Arrow::Up | Arrow::Down, Some(sub)) => {
                self.menu_cursor.sub_line = menu::step(&sub, cursor.sub_line, arrow == Arrow::Down);
            }
            (Arrow::Up | Arrow::Down, None) => {
                self.menu_cursor.line = menu::step(&entries, cursor.line, arrow == Arrow::Down);
            }
            (Arrow::Left, Some(_)) => {
                self.menu_cursor.submenu = None;
                self.menu_cursor.sub_line = None;
            }
            (Arrow::Right | Arrow::Enter, None)
                if cursor
                    .line
                    .is_some_and(|line| menu::is_submenu(&entries, line)) =>
            {
                let line = cursor.line.unwrap_or_default();
                let sub = menu::submenu_entries(&mut entries, line).unwrap_or_default();
                self.menu_cursor.submenu = Some(line);
                self.menu_cursor.sub_line = menu::step(&sub, None, true);
            }
            (Arrow::Left | Arrow::Right, _) => {
                let step = if arrow == Arrow::Right { 1 } else { -1 };
                let next = open.beside(step, |menu| self.can_open(menu));
                return self.update(Message::MenuKey(next));
            }
            (Arrow::Enter, Some(sub)) => {
                if let Some(message) = cursor.sub_line.and_then(|line| menu::take_press(sub, line))
                {
                    return self.update(message);
                }
            }
            (Arrow::Enter, None) => {
                if let Some(message) = cursor.line.and_then(|line| menu::take_press(entries, line))
                {
                    return self.update(message);
                }
            }
        }
        Task::none()
    }
}

impl Workspace {
    fn open(root: PathBuf, open_in_reading: bool) -> Self {
        let tree = campaign::scan(&root);
        let (own_types, types) = load_types(&root, &tree);
        let scan = scan_notes(&tree);
        let index = links::Index::new(&root, markdown_files(&tree)).with_aliases(&scan.aliases);
        let backlinks = backlinks::Backlinks::new(scan.mentions, &index);
        let problems = check::campaign(&root, &tree, &types, &index, &check::Gone::new());
        let (fingerprints, titles) = (scan.hashes, scan.titles);
        let tags = known_tags(&root, &types);
        let status = match problems_summary(&problems) {
            Some(summary) => format!("Opened {}. {summary}", root.display()),
            None => format!("Opened {}", root.display()),
        };
        let table_settings = tables::Settings::load(&root);
        Workspace {
            types,
            own_types,
            problems,
            gone: check::Gone::new(),
            ignored: HashSet::new(),
            fingerprints,
            titles,
            tree,
            status,
            root,
            expanded: HashSet::new(),
            tabs: Vec::new(),
            active: None,
            next_id: 0,
            dialog: None,
            index,
            backlinks,
            ctrl: false,
            hovered: None,
            drag: None,
            dice: dice::Roller::default(),
            open_in_reading,
            web: web::Pictures::new(),
            context: None,
            hovered_block: None,
            menu_block: None,
            save_all: None,
            tags,
            tag_saves: HashMap::new(),
            search: Search::default(),
            quests: quests::Quests::default(),
            tables: tables::List::default(),
            table_settings,
            roll_folder: None,
            walkthrough: None,
            table_keys: BTreeSet::new(),
        }
        .with_table_keys()
    }

    /// Finds the roll tables a link can name, for [`Self::table_keys`].
    fn with_table_keys(mut self) -> Self {
        self.table_keys = self.step_table_keys();
        self
    }

    /// The [`tables::link_key`]s of the roll tables a step's link rolls: as
    /// [`Self::table_link_keys`], but a typed note, like a location with a table in its
    /// description, only by `Note#Heading`, since a link to it is a link to the place.
    fn step_table_keys(&self) -> BTreeSet<String> {
        let mut keys = self.table_link_keys();
        for entry in self.all_tables() {
            let typed = entry
                .path
                .parent()
                .is_some_and(|folder| self.types.contains_key(folder))
                || self
                    .note_text(&entry.path)
                    .and_then(|text| note::parse(&text).ok())
                    .is_some_and(|note| note.front.contains_key("type"));
            if typed {
                keys.remove(&tables::link_key(&entry.note));
            }
        }
        keys
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

    /// Brings the tag notes up to date with the note at `path`, just saved as `after`
    /// (and `before` before; empty for a new note), reading its tags from the front matter
    /// keys of `fields`: the note of each tag it has lists a link to it (and is made if
    /// there is none), and the notes of tags taken off it don't.
    fn sync_tags(&mut self, path: &Path, fields: &[(String, String)], before: &str, after: &str) {
        let now = tags::in_note(after, fields);
        let gone: Vec<(String, String)> = tags::in_note(before, fields)
            .into_iter()
            .filter(|tag| !now.contains(tag))
            .collect();
        self.update_tag_notes(path, &now, &gone);
    }

    /// Adds a link to the note at `path` to the notes of the tags in `add` (making the
    /// ones there aren't yet), and takes it out of the notes of the tags in `remove`.
    fn update_tag_notes(
        &mut self,
        path: &Path,
        add: &[(String, String)],
        remove: &[(String, String)],
    ) {
        let (now, gone) = (add, remove);
        if now.is_empty() && gone.is_empty() {
            return;
        }
        let Some(name) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
        else {
            return;
        };

        let mut changed = Vec::new();
        let mut problems = Vec::new();
        for ((folder, tag), add) in now
            .iter()
            .map(|tag| (tag, true))
            .chain(gone.iter().map(|tag| (tag, false)))
        {
            let file = tags::path(&self.root, folder, tag);
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            // A link in the tag's note is to this note if it finds it, or names it (as
            // a link in a note not made yet would).
            let links_to = |target: &str| {
                self.index.resolve(target, &file) == Some(path)
                    || links::normalize(target) == links::normalize(&name)
            };
            let new = if add {
                tags::add(&text, &name, &links_to)
            } else {
                tags::remove(&text, &links_to)
            };
            let Some(new) = new else {
                continue;
            };
            let written = file
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&file, new));
            match written {
                Ok(()) => changed.push(file),
                Err(_) => problems.push(self.relative(&file)),
            }
        }

        // Open tag notes show the change, unless they have changes of their own.
        for file in &changed {
            if let Some(index) = self
                .tabs
                .iter()
                .position(|tab| tab.path.as_ref() == Some(file))
            {
                if self.tabs[index].modified {
                    self.tabs[index].changed_outside = true;
                } else {
                    let _ = self.reload_tab(index);
                }
            }
        }
        if !changed.is_empty() {
            self.rescan();
        }
        if !problems.is_empty() {
            self.status.push_str(&format!(
                ". Could not update the tag notes {}",
                problems.join(", ")
            ));
        }
    }

    /// The notes of typed folders in `paths` whose tags differ from their tag notes, as
    /// they would after their tags were changed in another app, like Obsidian.
    fn tag_changes(&self, paths: &[PathBuf]) -> Vec<TagChange> {
        let mut changes = Vec::new();
        for path in paths {
            let Some(Ok(ty)) = path.parent().and_then(|folder| self.types.get(folder)) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            let Some(name) = path.file_stem().map(|stem| stem.to_string_lossy()) else {
                continue;
            };
            let links_to = |target: &str, file: &Path| {
                self.index.resolve(target, file) == Some(path.as_path())
                    || links::normalize(target) == links::normalize(&name)
            };
            let (add, remove) = tags::differences(&self.root, &text, &ty.tag_fields(), &links_to);
            if !add.is_empty() || !remove.is_empty() {
                changes.push(TagChange {
                    note: path.clone(),
                    add,
                    remove,
                });
            }
        }
        changes
    }

    fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| Some(tab.id) == self.active)
    }

    /// Reads the file tree and the folders' content types again, and checks the files.
    fn rescan(&mut self) {
        self.tree = campaign::scan(&self.root);
        (self.own_types, self.types) = load_types(&self.root, &self.tree);
        self.tags = known_tags(&self.root, &self.types);
        let scan = scan_notes(&self.tree);
        self.index =
            links::Index::new(&self.root, markdown_files(&self.tree)).with_aliases(&scan.aliases);
        self.backlinks = backlinks::Backlinks::new(scan.mentions, &self.index);
        (self.fingerprints, self.titles) = (scan.hashes, scan.titles);
        self.problems =
            check::campaign(&self.root, &self.tree, &self.types, &self.index, &self.gone);
        let ignored = &self.ignored;
        self.problems
            .retain(|problem| !is_ignored(ignored, problem));
        for index in 0..self.tabs.len() {
            if self.tabs[index].modified {
                self.recheck_tab(index);
            }
        }
        self.refresh_search();
        self.refresh_quests();
        self.refresh_tables();
        self.table_keys = self.step_table_keys();
    }

    /// Reads the quests again for the Quests list, if it is open. Open notes are read as
    /// their tabs have them, so ticking a step there finds its box where the tab has it.
    fn refresh_quests(&mut self) {
        if !self.quests.open {
            return;
        }
        let mut found = Vec::new();
        let mut options: Vec<String> = Vec::new();
        for path in markdown_files(&self.tree) {
            let Some(Ok(ty)) = path.parent().and_then(|folder| self.types.get(folder)) else {
                continue;
            };
            if !ty.is_a(quests::QUEST) {
                continue;
            }
            if let Some(status) = ty.fields.iter().find(|field| field.key == "status") {
                for option in &status.options {
                    if !options.contains(option) {
                        options.push(option.clone());
                    }
                }
            }
            let open = self
                .tabs
                .iter()
                .find(|tab| tab.path.as_ref() == Some(&path))
                .and_then(|tab| tab.text().ok());
            let Some(text) = open.or_else(|| std::fs::read_to_string(&path).ok()) else {
                continue;
            };
            let name = self.tree_name(&path, false);
            found.push(quests::read(path, name, &text, ty, &self.index));
        }
        self.quests.set(found, &options);
    }

    /// The text of the note at `path`: as its tab has it if it is open, or as saved.
    fn note_text(&self, path: &Path) -> Option<String> {
        self.tabs
            .iter()
            .find(|tab| tab.note_path() == path)
            .and_then(|tab| tab.text().ok())
            .or_else(|| std::fs::read_to_string(path).ok())
    }

    /// Whether the roll table under `heading` in the note at `path` rolls each column.
    fn rolls_each_column(&self, path: &Path, heading: Option<&str>) -> bool {
        self.table_settings
            .each_column
            .contains(&tables::key(&self.root, path, heading))
    }

    /// Where the roll tables in `text`, the note at `path`, that roll each column are.
    fn each_column_tables(&self, path: &Path, text: &str) -> Vec<usize> {
        tables::find(text)
            .into_iter()
            .filter(|table| self.rolls_each_column(path, table.heading.as_deref()))
            .map(|table| table.range.start)
            .collect()
    }

    /// Every roll table in the campaign.
    fn all_tables(&self) -> Vec<tables::Entry> {
        let mut found = Vec::new();
        for path in markdown_files(&self.tree) {
            let Some(text) = self.note_text(&path) else {
                continue;
            };
            let note = self.tree_name(&path, false);
            for table in tables::find(&text) {
                found.push(tables::Entry {
                    each_column: self.rolls_each_column(&path, table.heading.as_deref()),
                    path: path.clone(),
                    note: note.clone(),
                    table,
                });
            }
        }
        found
    }

    /// Reads the roll tables again for the Roll Tables list, if it is open.
    fn refresh_tables(&mut self) {
        if self.tables.open {
            let found = self.all_tables();
            let from = self.root.join("Untitled.md");
            // A note named like a built-in table is used in its place.
            let built_in = tables::BUILT_IN
                .into_iter()
                .filter(|table| self.index.resolve(table.name, &from).is_none())
                .collect();
            self.tables.set(found, built_in);
            self.tables.open_groups = self.table_settings.open_groups.clone();
        }
    }

    /// The table a link to `target` in the note at `from` names: `[[Note]]` is the
    /// note's first table, `[[Note#Heading]]` the one under that heading. A name no note
    /// has can be a built-in table's.
    /// The campaign's roll tables, by the name of the note each is in and its heading.
    fn table_refs(&self) -> Vec<(String, Option<String>)> {
        self.all_tables()
            .iter()
            .map(|entry| (entry.note.clone(), entry.table.heading.clone()))
            .collect()
    }

    fn linked_table(&self, target: &str, from: &Path) -> Option<(String, tables::Table, bool)> {
        let (note, heading) = match target.split_once('#') {
            Some((note, heading)) => (note.trim(), Some(heading.trim())),
            None => (target.trim(), None),
        };
        let path = match note {
            "" => Some(from.to_path_buf()),
            note => self.index.resolve(note, from).map(Path::to_path_buf),
        };
        let found = path.and_then(|path| {
            let text = self.note_text(&path)?;
            let table = tables::find(&text)
                .into_iter()
                .find(|table| match heading {
                    Some(heading) => table
                        .heading
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(heading)),
                    None => true,
                })?;
            let name = table.name(&self.tree_name(&path, false));
            let each_column = self.rolls_each_column(&path, table.heading.as_deref());
            Some((name, self.with_settings(&path, table), each_column))
        });
        found.or_else(|| tables::built_in_link(target))
    }

    /// `table`, from the note at `path`, with the links it doesn't roll from the settings.
    fn with_settings(&self, path: &Path, mut table: tables::Table) -> tables::Table {
        let key = tables::key(&self.root, path, table.heading.as_deref());
        table.not_rolled = self.not_rolled(&key);
        table
    }

    /// The tables that the results of table `key` link to without rolling them.
    fn not_rolled(&self, key: &str) -> BTreeSet<String> {
        self.table_settings
            .not_rolled
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    /// The table typed into the dice roller's box: by its note's name (or `Note#Heading`),
    /// or by its own name.
    fn table_named(&self, typed: &str) -> Option<(PathBuf, String, tables::Table, bool)> {
        let typed = typed.trim();
        if typed.is_empty() {
            return None;
        }
        let from = self.root.join("Untitled.md");
        let note = typed.split('#').next().unwrap_or(typed);
        if let Some((name, table, each_column)) = self.linked_table(typed, &from) {
            let path = self
                .index
                .resolve(note, &from)
                .map_or(from, Path::to_path_buf);
            return Some((path, name, table, each_column));
        }
        self.all_tables()
            .into_iter()
            .find(|entry| entry.name().eq_ignore_ascii_case(typed))
            .map(|entry| {
                let name = entry.name();
                (entry.path, name, entry.table, entry.each_column)
            })
    }

    /// Rolls the table that starts at `start` in the note at `path`.
    fn roll_table(&mut self, path: &Path, start: usize) {
        let Some(text) = self.note_text(path) else {
            return;
        };
        let Some(table) = tables::find(&text)
            .into_iter()
            .find(|table| table.range.start == start)
        else {
            return;
        };
        let name = table.name(&self.tree_name(path, false));
        let each_column = self.rolls_each_column(path, table.heading.as_deref());
        self.roll_found(path, &name, &table, each_column);
    }

    /// Rolls `table`, from the note at `path`, and shows the result in the dice roller.
    fn roll_found(
        &mut self,
        path: &Path,
        name: &str,
        table: &tables::Table,
        each_column: bool,
    ) -> String {
        let table = self.with_settings(path, table.clone());
        self.roll_found_as_is(path, name, &table, each_column)
    }

    /// Like [`Self::roll_found`], with the links `table` doesn't roll as they are in it.
    fn roll_found_as_is(
        &mut self,
        path: &Path,
        name: &str,
        table: &tables::Table,
        each_column: bool,
    ) -> String {
        let mut rng = self.dice.rng().clone();
        let rolled = tables::roll(name, table, each_column, &mut rng, &mut |target| {
            self.linked_table(target, path)
        });
        *self.dice.rng() = rng;
        let text = rolled.text();
        self.dice.open = true;
        self.dice.add_table(rolled);
        text
    }

    /// Rolls on the table field `index` of the active tab's form names, and puts the
    /// result in the field: straight away if it is empty, or after asking if it isn't.
    fn roll_into_field(&mut self, index: usize, table_name: &str) {
        let Some(tab) = self.active_tab().filter(|tab| !tab.source) else {
            return;
        };
        let Some(form) = &tab.form else {
            return;
        };
        let (Some((current, long)), Some(label)) = (form.text_value(index), form.label(index))
        else {
            return;
        };
        let path = tab.note_path();
        let Some((name, table, each_column)) = self.linked_table(table_name, &path) else {
            self.status = format!(
                "There is no roll table called {table_name}. Make one, or change the field's Rolls on in Edit > Types"
            );
            return;
        };
        let value = self.roll_found(&path, &name, &table, each_column);
        if current.trim().is_empty() {
            self.fill_field(index, &value, false);
        } else {
            self.dialog = Some(Dialog::ReplaceField {
                index,
                label,
                value,
                long,
            });
        }
    }

    /// Puts `value` in field `index` of the active tab's form, in place of what is there
    /// or on a new line after it.
    fn fill_field(&mut self, index: usize, value: &str, add_line: bool) {
        let root = self.root.clone();
        let links = self.index.clone();
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        let Some(form) = &mut tab.form else {
            return;
        };
        if form.set_text_value(index, value, add_line) {
            tab.modified = true;
            if tab.preview.is_some() {
                tab.preview = Some(tab.make_preview(&root, &links));
            }
        }
    }

    /// Sets whether the table that starts at `start` in the note at `path` rolls each
    /// column on its own, and remembers it in the campaign.
    fn set_each_column(&mut self, path: &Path, start: usize, on: bool) {
        let Some(text) = self.note_text(path) else {
            return;
        };
        let Some(table) = tables::find(&text)
            .into_iter()
            .find(|table| table.range.start == start)
        else {
            return;
        };
        let key = tables::key(&self.root, path, table.heading.as_deref());
        self.table_settings.set(&key, on);
        if let Err(error) = self.table_settings.save(&self.root) {
            self.status = format!("Could not remember how the table rolls: {error}");
        }
        self.refresh_tables();
    }

    /// `folder` as a choice in a folder picker.
    fn folder_choice(&self, folder: &Path) -> FolderChoice {
        FolderChoice {
            label: if folder == self.root {
                "The campaign folder".into()
            } else if folder.is_dir() {
                self.relative(folder)
            } else {
                format!("{} (new folder)", self.relative(folder))
            },
            path: folder.to_path_buf(),
        }
    }

    /// The folders a new note can go in: the campaign folder and every folder in it, and
    /// `extra` if it isn't one yet.
    fn folder_choices(&self, extra: &Path) -> Vec<FolderChoice> {
        let mut folders = vec![self.root.clone()];
        collect_folders(&self.tree, &mut folders);
        if !folders.iter().any(|folder| folder == extra) {
            folders.push(extra.to_path_buf());
        }
        folders
            .iter()
            .map(|folder| self.folder_choice(folder))
            .collect()
    }

    /// Copies the built-in table `name` into the campaign's tables folder and opens it.
    /// The copy is used in its place from then on.
    fn copy_built_in(&mut self, name: &str) {
        let Some(table) = tables::built_in(name) else {
            return;
        };
        let folder = self.root.join(TABLES_FOLDER);
        let mut path = folder.join(campaign::note_file_name(table.name));
        path.as_mut_os_string().push(".md");
        let made = std::fs::create_dir_all(&folder).and_then(|()| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            std::io::Write::write_all(&mut file, table.text.as_bytes())
        });
        match made {
            Ok(()) => {
                self.status = format!(
                    "Copied {} to {}: change it there, and it is used in place of the built-in one",
                    table.name,
                    self.relative(&path)
                );
                // Its tables roll each column like the built-in ones did.
                if table.each_column {
                    for found in tables::find(table.text) {
                        let key = tables::key(&self.root, &path, found.heading.as_deref());
                        self.table_settings.set(&key, true);
                    }
                    let _ = self.table_settings.save(&self.root);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                self.status = format!("{} is already in {}", table.name, self.relative(&folder));
            }
            Err(error) => {
                self.status = format!("Could not copy {}: {error}", table.name);
                return;
            }
        }
        self.rescan();
        self.expanded.insert(folder);
        self.open_file(path);
    }

    /// Makes the note New Roll Table asks for, and opens it to fill in.
    fn create_table(&mut self) -> Result<Task<Message>, String> {
        let Some(Dialog::NewTable {
            name,
            folder,
            die,
            columns,
            rows,
            pasted,
            ..
        }) = &self.dialog
        else {
            return Ok(Task::none());
        };
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err("Give the table a name".into());
        }
        campaign::validate_name(&name)?;
        let blank = tables::Blank {
            die: (*die).to_string(),
            columns: columns.clone(),
            rows: rows.parse().ok().filter(|rows: &i64| *rows > 0),
            pasted: pasted.text(),
        };
        let folder = folder.path.clone();
        std::fs::create_dir_all(&folder)
            .map_err(|error| format!("Could not make the folder: {error}"))?;
        let mut path = folder.join(campaign::note_file_name(&name));
        path.as_mut_os_string().push(".md");
        let mut text = tables::markdown(&blank);
        if campaign::note_file_name(&name) != name {
            // The file name can't hold the name, so the note keeps it.
            text = format!(
                "---\nname: {}\n---\n\n{text}",
                serde_yaml_ng::to_string(&name).unwrap_or_default().trim()
            );
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::AlreadyExists => {
                    format!("{name} is already a note in that folder")
                }
                _ => format!("Could not create {}: {error}", path.display()),
            })?;
        std::io::Write::write_all(&mut file, text.as_bytes())
            .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
        self.dialog = None;
        self.rescan();
        self.expanded.insert(folder);
        // It opens in the table editor, ready to fill in.
        let start = tables::find(&text).first().map(|table| table.range.start);
        let task = match start {
            Some(start) => self.edit_table_in(path.clone(), start),
            None => {
                self.open_file(path.clone());
                Task::none()
            }
        };
        self.status = format!(
            "Created the roll table {}: fill in its rows",
            self.relative(&path)
        );
        Ok(task)
    }

    /// Ticks or unticks the step of the quest at `path` whose task box is at `at`: in its
    /// tab if it is open, where it stays unsaved like any edit, or else in the file.
    fn tick_step(&mut self, path: &Path, at: usize) {
        let root = self.root.clone();
        let index = self.index.clone();
        if let Some(at_tab) = self
            .tabs
            .iter()
            .position(|tab| tab.path.as_deref() == Some(path))
        {
            let tab = &mut self.tabs[at_tab];
            let text = tab.text().unwrap_or_else(|_| tab.content.text());
            if let Some(new) = preview::toggle_task(&text, at) {
                tab.set_text(&new);
                tab.modified = true;
                if tab.preview.is_some() {
                    tab.preview = Some(tab.make_preview(&root, &index));
                }
                self.recheck_tab(at_tab);
            }
        } else {
            let ticked = std::fs::read_to_string(path)
                .map_err(|error| error.to_string())
                .and_then(|text| {
                    preview::toggle_task(&text, at).ok_or_else(|| "the step has moved".to_string())
                })
                .and_then(|new| std::fs::write(path, new).map_err(|error| error.to_string()));
            if let Err(error) = ticked {
                self.status = format!("Could not tick the step: {error}");
            }
        }
        self.refresh_quests();
    }

    /// Searches the notes again for what is in the search box. Open notes are searched
    /// as their tabs have them, unsaved changes and all.
    fn refresh_search(&mut self) {
        if !self.search.active() {
            self.search.hits.clear();
            return;
        }
        let query = search::Query::parse(&self.search.query);
        let mut hits = Vec::new();
        for path in markdown_files(&self.tree) {
            if self
                .search
                .folder
                .as_ref()
                .is_some_and(|folder| !path.starts_with(folder))
            {
                continue;
            }
            let open = self
                .tabs
                .iter()
                .find(|tab| tab.modified && tab.path.as_ref() == Some(&path))
                .and_then(|tab| tab.text().ok());
            let Some(text) = open.or_else(|| std::fs::read_to_string(&path).ok()) else {
                continue;
            };
            let mut names = vec![display_name(&path, false)];
            names.extend(self.titles.get(&path).cloned());
            let fields: Vec<(String, String)> =
                match path.parent().and_then(|folder| self.types.get(folder)) {
                    Some(Ok(ty)) => ty
                        .fields
                        .iter()
                        .map(|field| (field.key.clone(), field.label()))
                        .collect(),
                    _ => Vec::new(),
                };
            hits.extend(search::find(&query, path, &text, &names, &fields));
        }
        // Notes named for what was typed first, then by name.
        hits.sort_by_cached_key(|hit| (hit.rank, self.tree_name(&hit.path, false).to_lowercase()));
        self.search.hits = hits;
    }

    /// Checks the note in the tab at `index` as the tab has it, unsaved changes and all,
    /// so its problems go away as soon as they are fixed.
    fn recheck_tab(&mut self, index: usize) {
        let tab = &self.tabs[index];
        let (Some(path), true) = (tab.path.clone(), tab.is_note()) else {
            return;
        };
        let Ok(text) = tab.text() else {
            return;
        };
        self.backlinks
            .set(path.clone(), backlinks::mentions(&text), &self.index);
        let mut found = check::file(
            &path,
            &text,
            &self.root,
            &self.types,
            &self.index,
            &self.gone,
        );
        found.retain(|problem| !self.is_ignored(problem));

        self.problems.retain(|problem| problem.path != path);
        // Keep the list in the campaign's order, so the note doesn't jump to the end.
        let at = self
            .problems
            .iter()
            .position(|problem| problem.path > path)
            .unwrap_or(self.problems.len());
        self.problems.splice(at..at, found);
    }

    /// Checks the active tab again, after it may have been edited.
    fn recheck_active(&mut self) {
        if let Some(index) = self.tabs.iter().position(|tab| Some(tab.id) == self.active)
            && self.tabs[index].modified
        {
            self.recheck_tab(index);
        }
    }

    /// Checks the note at `path` again as it is on disk, after its tab was closed unsaved.
    fn recheck_file(&mut self, path: &Path) {
        self.problems.retain(|problem| problem.path != path);
        if let Ok(text) = std::fs::read_to_string(path) {
            self.backlinks
                .set(path.to_path_buf(), backlinks::mentions(&text), &self.index);
            let mut found = check::file(
                path,
                &text,
                &self.root,
                &self.types,
                &self.index,
                &self.gone,
            );
            found.retain(|problem| !self.is_ignored(problem));
            let at = self
                .problems
                .iter()
                .position(|problem| problem.path.as_path() > path)
                .unwrap_or(self.problems.len());
            self.problems.splice(at..at, found);
        }
    }

    /// Whether `problem` is a broken link the user chose to leave as it is.
    fn is_ignored(&self, problem: &Problem) -> bool {
        is_ignored(&self.ignored, problem)
    }

    /// Reads the campaign again after its files changed, maybe in another app: notes
    /// renamed there keep their tabs and the links to them, and open notes changed there
    /// are reloaded.
    fn pick_up_outside_changes(&mut self) {
        let status = std::mem::take(&mut self.status);
        let before = std::mem::take(&mut self.fingerprints);
        let old_notes: HashSet<PathBuf> = before.keys().cloned().collect();
        self.rescan();

        let removed: Vec<PathBuf> = old_notes
            .iter()
            .filter(|path| !self.fingerprints.contains_key(*path))
            .cloned()
            .collect();
        let added: Vec<PathBuf> = self
            .fingerprints
            .keys()
            .filter(|path| !old_notes.contains(*path))
            .cloned()
            .collect();
        let moved = renamed_notes(&removed, &added, &before, &self.fingerprints);

        // Links to notes that are simply gone now show as broken, rather than as notes
        // still to be written.
        let mut new_gone = false;
        for path in &removed {
            if moved.iter().any(|(old, _)| old == path) {
                continue;
            }
            if let Ok(relative) = path.strip_prefix(&self.root) {
                let relative = relative.with_extension("");
                let key = links::normalize(&relative.to_string_lossy());
                let mut name = key.as_str();
                loop {
                    new_gone |= self.gone.insert(name.to_string());
                    match name.split_once('/') {
                        Some((_, rest)) => name = rest,
                        None => break,
                    }
                }
            }
        }

        let mut messages = Vec::new();
        if !moved.is_empty() {
            // A note's name field follows its new filename, or saving it from the form
            // would rename it back.
            for (old, new) in &moved {
                let (Some(old_name), Some(new_name)) = (old.file_stem(), new.file_stem()) else {
                    continue;
                };
                let Ok(text) = std::fs::read_to_string(new) else {
                    continue;
                };
                let named_old = note::parse(&text).is_ok_and(|note| {
                    note.front.get("name").is_some_and(|name| {
                        campaign::note_file_name(&note::value_to_text(name))
                            == old_name.to_string_lossy()
                    })
                });
                if named_old
                    && let Ok(text) =
                        fix::set_value(&text, "name", Some(&new_name.to_string_lossy()))
                {
                    let text = with_alias(
                        &text,
                        &new_name.to_string_lossy(),
                        Some(&old_name.to_string_lossy()),
                    );
                    let _ = std::fs::write(new, text);
                }
            }

            for index in 0..self.tabs.len() {
                let tab = &mut self.tabs[index];
                if let Some(path) = &tab.path
                    && let Some((_, new)) = moved.iter().find(|(old, _)| old == path)
                {
                    tab.folder = new.parent().unwrap_or(&self.root).to_path_buf();
                    tab.path = Some(new.clone());
                    if tab.modified {
                        tab.disk = check::modified_time(new);
                    } else {
                        let _ = self.reload_tab(index);
                    }
                }
            }

            let done = match moved.as_slice() {
                [(old, new)] => format!(
                    "{} was renamed to {} outside Dungeon Elements",
                    self.relative(old),
                    self.relative(new)
                ),
                _ => format!(
                    "{} notes were renamed outside Dungeon Elements",
                    moved.len()
                ),
            };
            self.relink(&moved, &done);
            messages.push(std::mem::take(&mut self.status));
            self.rescan();
        } else if new_gone {
            self.rescan();
        }

        // Tags changed in another app, like Obsidian, can update the tag notes too.
        let changed: Vec<PathBuf> = self
            .fingerprints
            .iter()
            .filter(|(path, hash)| before.get(*path) != Some(*hash))
            .map(|(path, _)| path.clone())
            .collect();
        let tag_changes = self.tag_changes(&changed);
        if !tag_changes.is_empty() && self.dialog.is_none() {
            self.dialog = Some(Dialog::Tags(tag_changes));
        }

        self.status.clear();
        self.check_disk();
        if !self.status.is_empty() {
            messages.push(std::mem::take(&mut self.status));
        }
        self.status = if messages.is_empty() {
            status
        } else {
            messages.join(". ")
        };
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
            // Only the time changed (or the app's own save got here first): nothing to do.
            if now.is_some()
                && let (Ok(disk), Ok(text)) = (std::fs::read_to_string(&path), tab.text())
                && disk == text
            {
                self.tabs[index].disk = now;
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
        tab.edit = None;
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
        let read = self.open_in_reading && path.as_deref().is_some_and(campaign::is_markdown);
        self.tabs.push(Tab {
            id,
            content: text_editor::Content::with_text(text),
            form,
            source: false,
            preview: None,
            edit: None,
            disk: path.as_deref().and_then(check::modified_time),
            path,
            folder,
            modified: false,
            changed_outside: false,
        });
        self.active = Some(id);
        if read {
            self.read_active_tab();
        }
    }

    /// Shows the active tab in the reading view.
    fn read_active_tab(&mut self) {
        let root = self.root.clone();
        let index = self.index.clone();
        if let Some(tab) = self.active_tab_mut()
            && tab.preview.is_none()
        {
            tab.preview = Some(tab.make_preview(&root, &index));
        }
    }

    /// Puts the text typed into the reading view's block editor back into the note, and
    /// stops editing. Returns how many more blocks the note has than before, so a block
    /// index from before can be moved along.
    fn finish_editing(&mut self) -> isize {
        let root = self.root.clone();
        let index = self.index.clone();
        let Some(tab) = self.active_tab_mut() else {
            return 0;
        };
        let (range, edited, table) = match tab.edit.take() {
            Some(ReadEdit::Block { range, content, .. }) => (range, content.text(), None),
            Some(ReadEdit::Table { editor, .. }) => {
                (editor.range.clone(), editor.markdown(), Some(editor))
            }
            _ => return 0,
        };
        let Some(preview) = &tab.preview else {
            return 0;
        };
        let blocks = preview.block_count();
        let text = tab.text().unwrap_or_else(|_| tab.content.text());
        // The note can't have changed while the block was edited, but check anyway.
        if text != preview.source() || text.get(range.clone()).is_none() {
            return 0;
        }

        let path = tab.note_path();
        let mut new = match &table {
            // A table goes back exactly where it was, and only if it was changed.
            Some(editor) if !editor.changed() => text.clone(),
            Some(_) => format!("{}{edited}{}", &text[..range.start], &text[range.end..]),
            None => replace_block(&text, range.clone(), &edited),
        };
        // A renamed table renames its heading, and the links to it in this note.
        let renamed = table
            .as_ref()
            .and_then(|editor| editor.heading())
            .filter(|(old, new)| !new.is_empty() && old != new)
            .map(|(old, new)| (old.to_string(), new.to_string()));
        if let Some((old, heading)) = &renamed {
            if let Some(changed) = table_editor::rename_heading(&new, range.start, old, heading) {
                new = changed;
            }
            if let Some((changed, _)) =
                links::rename_heading(&new, &path, &index, &path, old, heading)
            {
                new = changed;
            }
        }
        let mut added = 0;
        if new != text {
            tab.set_text(&new);
            tab.modified = true;
            let preview = tab.make_preview(&root, &index);
            added = preview.block_count() as isize - blocks as isize;
            tab.preview = Some(preview);
        }
        if let Some(editor) = table {
            let old = editor.heading().map(|(old, _)| old.to_string());
            let heading = match &renamed {
                Some((_, heading)) => Some(heading.clone()),
                None => old.clone(),
            };
            self.save_table_settings(
                &path,
                old.as_deref(),
                heading.as_deref(),
                editor.each_column(),
                editor.not_rolled(),
            );
            if let Some((old, heading)) = &renamed {
                self.rename_heading_links(&path, old, heading);
            }
        }
        added
    }

    /// The heading of the note that the table in the active tab's form's table editor is
    /// under: its own heading in the field, or else the field's.
    fn form_table_heading(label: &str, editor: &table_editor::Editor) -> String {
        match editor.heading() {
            Some((old, _)) => old.to_string(),
            None => label.to_string(),
        }
    }

    /// Rolls on the roll table that step `step` of steps field `index` links to, in the
    /// active tab's form.
    fn roll_step(&mut self, index: usize, step: usize) {
        let Some(tab) = self.active_tab() else {
            return;
        };
        let path = tab.note_path();
        let Some(target) = tab.form.as_ref().and_then(|form| {
            form.step_targets(index, step)
                .into_iter()
                .find(|target| self.table_keys.contains(&tables::link_key(target)))
        }) else {
            return;
        };
        match self.linked_table(&target, &path) {
            Some((name, table, each_column)) => {
                self.roll_found_as_is(&path, &name, &table, each_column);
            }
            None => self.status = format!("There is no roll table called {target}"),
        }
    }

    /// Gives the editors of the roll table fields in the active tab's form what the
    /// settings keep for their tables, once.
    fn set_form_tables(&mut self) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        let path = tab.note_path();
        let Some(form) = &mut tab.form else {
            return;
        };
        let labels: Vec<String> = form
            .unset_tables()
            .into_iter()
            .map(|(label, _)| label)
            .collect();
        if labels.is_empty() {
            return;
        }
        let settings: Vec<(bool, BTreeSet<String>)> = labels
            .iter()
            .map(|label| {
                let key = tables::key(&self.root, &path, Some(label));
                (
                    self.rolls_each_column(&path, Some(label)),
                    self.not_rolled(&key),
                )
            })
            .collect();
        let links = self.table_link_keys();
        let Some(form) = self.active_tab_mut().and_then(|tab| tab.form.as_mut()) else {
            return;
        };
        for ((_, editor), (each_column, not_rolled)) in
            form.unset_tables().into_iter().zip(settings)
        {
            editor.set_settings(Some(each_column), not_rolled);
            editor.set_table_links(links.clone());
        }
    }

    /// Keeps what roll table field `index` of the active tab's form is set to: whether it
    /// rolls each column on its own, and the links it doesn't roll.
    fn save_field_table_settings(&mut self, index: usize) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        let path = tab.note_path();
        let Some((label, editor)) = tab
            .form
            .as_mut()
            .and_then(|form| form.field_table_mut(index))
        else {
            return;
        };
        if !editor.has_settings() {
            return;
        }
        let (each_column, not_rolled) = (editor.each_column(), editor.not_rolled());
        self.save_table_settings(&path, Some(&label), Some(&label), each_column, not_rolled);
    }

    /// Rolls roll table field `index` of the active tab's form, as it is in its editor.
    fn roll_field_table(&mut self, index: usize) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        let path = tab.note_path();
        let Some((_, editor)) = tab
            .form
            .as_mut()
            .and_then(|form| form.field_table_mut(index))
        else {
            return;
        };
        let (name, table, each_column) = (editor.name(), editor.table(), editor.each_column());
        if let Some(table) = table {
            self.roll_found_as_is(&path, &name, &table, each_column.unwrap_or(false));
        }
    }

    /// Rolls the table in the active tab's form's table editor, as it is in the editor.
    fn roll_form_table(&mut self) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        let path = tab.note_path();
        let Some((label, editor)) = tab.form.as_mut().and_then(form::Form::table_editor_mut) else {
            return;
        };
        let heading = Self::form_table_heading(&label, editor);
        let (name, table, each_column) = (editor.name(), editor.table(), editor.each_column());
        let Some(table) = table else {
            return;
        };
        let each_column =
            each_column.unwrap_or_else(|| self.rolls_each_column(&path, Some(&heading)));
        self.roll_found_as_is(&path, &name, &table, each_column);
    }

    /// Gives the table editor just opened in the active tab's form what the settings keep
    /// for its table, and keeps what the one `finished` with Done set.
    fn form_table_settings(
        &mut self,
        finished: Option<(String, Box<table_editor::Editor>)>,
        opened: bool,
    ) {
        let Some(path) = self.active_tab().map(Tab::note_path) else {
            return;
        };
        if let Some((label, editor)) = finished {
            let old = Self::form_table_heading(&label, &editor);
            let renamed = editor
                .heading()
                .filter(|(old, new)| !new.is_empty() && old != new)
                .map(|(_, new)| new.to_string());
            let heading = renamed.clone().unwrap_or_else(|| old.clone());
            self.save_table_settings(
                &path,
                Some(&old),
                Some(&heading),
                editor.each_column(),
                editor.not_rolled(),
            );
            if let Some(new) = renamed {
                self.rename_heading_links(&path, &old, &new);
            }
        }
        if !opened {
            return;
        }
        let links = self.table_link_keys();
        let Some((label, editor)) = self
            .active_tab()
            .and_then(|tab| tab.form.as_ref())
            .and_then(form::Form::table_heading)
        else {
            return;
        };
        let heading = match editor {
            Some(old) => old,
            None => label,
        };
        let each_column = self.rolls_each_column(&path, Some(&heading));
        let key = tables::key(&self.root, &path, Some(&heading));
        let not_rolled = self.not_rolled(&key);
        if let Some(tab) = self.active_tab_mut()
            && let Some((_, editor)) = tab.form.as_mut().and_then(form::Form::table_editor_mut)
        {
            editor.set_settings(Some(each_column), not_rolled);
            editor.set_table_links(links);
        }
    }

    /// Keeps what the table editor set for the table under `heading` in the note at
    /// `path`, which was under `old`: whether it rolls each column (if it could be set),
    /// and the tables it links to without rolling.
    fn save_table_settings(
        &mut self,
        path: &Path,
        old: Option<&str>,
        heading: Option<&str>,
        each_column: Option<bool>,
        not_rolled: BTreeSet<String>,
    ) {
        let old_key = tables::key(&self.root, path, old);
        let key = tables::key(&self.root, path, heading);
        let before = self.table_settings.clone();
        let each_column = each_column.unwrap_or_else(|| self.rolls_each_column(path, old));
        if old_key != key {
            self.table_settings.set(&old_key, false);
            self.table_settings
                .set_not_rolled(&old_key, BTreeSet::new());
        }
        self.table_settings.set(&key, each_column);
        self.table_settings.set_not_rolled(&key, not_rolled);
        if self.table_settings == before {
            return;
        }
        if let Err(error) = self.table_settings.save(&self.root) {
            self.status = format!("Could not remember how the table rolls: {error}");
        }
        self.refresh_tables();
    }

    /// Points the links to heading `old` of the note at `note`, in the other notes, to
    /// heading `new`, as when a roll table under it is renamed.
    fn rename_heading_links(&mut self, note: &Path, old: &str, new: &str) {
        let mut changed = 0;
        let mut problems = Vec::new();
        for path in markdown_files(&self.tree) {
            if path == note {
                continue;
            }
            let tab = self
                .tabs
                .iter()
                .position(|tab| tab.path.as_ref() == Some(&path));
            // An open note with unsaved changes gets them in its tab, to save with them.
            if let Some(index) = tab
                && self.tabs[index].modified
            {
                let Ok(text) = self.tabs[index].text() else {
                    continue;
                };
                if let Some((text, count)) =
                    links::rename_heading(&text, &path, &self.index, note, old, new)
                {
                    let tab = &mut self.tabs[index];
                    match &tab.form {
                        Some(form) if !tab.source => {
                            if let Ok(parsed) = note::parse(&text) {
                                tab.form = Some(Form::from_note(form.ty.clone(), parsed));
                            }
                        }
                        _ => tab.content = text_editor::Content::with_text(&text),
                    }
                    tab.preview = None;
                    changed += count;
                }
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some((text, count)) =
                links::rename_heading(&text, &path, &self.index, note, old, new)
            else {
                continue;
            };
            if std::fs::write(&path, text).is_err() {
                problems.push(self.relative(&path));
                continue;
            }
            changed += count;
            if let Some(index) = tab {
                let _ = self.reload_tab(index);
            }
        }
        self.status = match changed {
            0 => format!("Renamed the {new} table"),
            1 => format!("Renamed the {new} table and updated 1 link to it"),
            count => format!("Renamed the {new} table and updated {count} links to it"),
        };
        if !problems.is_empty() {
            self.status.push_str(&format!(
                ". Could not update the links in {}",
                problems.join(", ")
            ));
        }
    }

    /// The [`tables::link_key`]s of every roll table a link can name: each table's note,
    /// and `Note#Heading` for a table under a heading, the built-in tables included.
    fn table_link_keys(&self) -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        for entry in self.all_tables() {
            keys.insert(tables::link_key(&entry.note));
            if let Some(heading) = &entry.table.heading {
                keys.insert(tables::link_key(&format!("{}#{heading}", entry.note)));
            }
        }
        for built_in in tables::BUILT_IN {
            keys.insert(tables::link_key(built_in.name));
            for table in tables::find(built_in.text) {
                if let Some(heading) = &table.heading {
                    keys.insert(tables::link_key(&format!("{}#{heading}", built_in.name)));
                }
            }
        }
        keys
    }

    /// Opens the note at `path` in the reading view, with the table editor on the roll
    /// table that starts at `start` in it.
    fn edit_table_in(&mut self, path: PathBuf, start: usize) -> Task<Message> {
        self.open_file(path.clone());
        if self.active_tab().and_then(|tab| tab.path.as_ref()) != Some(&path) {
            return Task::none();
        }
        self.read_active_tab();
        let block = self
            .active_tab()
            .and_then(|tab| tab.preview.as_ref())
            .and_then(|preview| {
                (0..preview.block_count()).find(|&block| {
                    preview
                        .block_range(block)
                        .is_some_and(|range| range.start == start)
                })
            });
        match block {
            Some(block) => self.edit_table(block),
            None => Task::none(),
        }
    }

    /// Adds a d6 roll table after the block the right-click menu was opened on (or at the
    /// end), and opens the table editor on it.
    fn add_table_here(&mut self) -> Task<Message> {
        let after = self.menu_block;
        let editing = match self.active_tab().and_then(|tab| tab.edit.as_ref()) {
            Some(ReadEdit::Block { index, .. } | ReadEdit::Table { index, .. }) => Some(*index),
            _ => None,
        };
        let added = self.finish_editing();
        let after = match (after, editing) {
            (Some(after), Some(edited)) if after > edited => {
                Some(after.saturating_add_signed(added))
            }
            _ => after,
        };
        let (root, index) = (self.root.clone(), self.index.clone());
        let Some(tab) = self.active_tab_mut().filter(|tab| tab.is_note()) else {
            return Task::none();
        };
        let Some(preview) = &tab.preview else {
            return Task::none();
        };
        let text = preview.source().to_string();
        let at = after
            .and_then(|after| preview.block_range(after))
            .map_or(text.len(), |range| range.end);
        let table = tables::markdown(&tables::Blank {
            die: "d6".into(),
            columns: vec!["Result".into()],
            rows: None,
            pasted: String::new(),
        });
        // The table gets a blank line on each side, so it isn't read as part of a paragraph.
        let before = text[..at].trim_end_matches('\n');
        let rest = text[at..].trim_start_matches('\n');
        let lead = if before.is_empty() { "" } else { "\n\n" };
        let start = before.len() + lead.len();
        let tail = if rest.is_empty() { "" } else { "\n" };
        let table = table.trim_end_matches('\n');
        let new = format!("{before}{lead}{table}\n{tail}{rest}");
        let added = (text.clone(), tab.modified);
        tab.set_text(&new);
        tab.modified = true;
        let preview = tab.make_preview(&root, &index);
        let block = (0..preview.block_count()).find(|&block| {
            preview
                .block_range(block)
                .is_some_and(|range| range.start == start)
        });
        tab.preview = Some(preview);
        let Some(block) = block else {
            return Task::none();
        };
        let _ = self.edit_table(block);
        let Some(ReadEdit::Table {
            editor,
            added: slot,
            ..
        }) = self.active_tab_mut().and_then(|tab| tab.edit.as_mut())
        else {
            return Task::none();
        };
        *slot = Some(added);
        // Focusing its first box scrolls it into sight, as it may be further down.
        let first = editor.cell_id(0, 0);
        iced::advanced::widget::operate(form::tab(vec![first], false)).discard()
    }

    /// Opens the table editor on the roll table that is block `index` of the active tab's
    /// reading view, if it is one, finishing what was being edited first.
    fn edit_table(&mut self, index: usize) -> Task<Message> {
        let editing = match self.active_tab().and_then(|tab| tab.edit.as_ref()) {
            Some(ReadEdit::Block { index, .. } | ReadEdit::Table { index, .. }) => Some(*index),
            _ => None,
        };
        let added = self.finish_editing();
        let index = match editing {
            Some(edited) if index > edited => index.saturating_add_signed(added),
            _ => index,
        };
        let Some(tab) = self.active_tab() else {
            return Task::none();
        };
        let Some(preview) = &tab.preview else {
            return Task::none();
        };
        let Some(range) = preview.block_range(index) else {
            return Task::none();
        };
        let source = preview.source().to_string();
        let Some(table) = tables::find(&source)
            .into_iter()
            .find(|table| table.range.start == range.start)
        else {
            return Task::none();
        };
        let path = tab.note_path();
        let name = table.name(&self.tree_name(&path, false));
        let each_column = self.rolls_each_column(&path, table.heading.as_deref());
        // A heading with more than one table under it can't be renamed from one of them.
        let shared = tables::find(&source)
            .iter()
            .filter(|other| other.heading.is_some() && other.heading == table.heading)
            .count()
            > 1;
        let table = self.with_settings(&path, table);
        let mut editor = table_editor::Editor::new(&source, &table, &name, Some(each_column));
        editor.set_table_links(self.table_link_keys());
        if shared {
            editor.keep_heading();
        }
        let mut editor = Box::new(editor);
        let focus = editor.take_focus();
        if let Some(tab) = self.active_tab_mut() {
            tab.edit = Some(ReadEdit::Table {
                index,
                editor,
                added: None,
            });
        }
        match focus {
            Some(id) => iced::widget::operation::focus(id),
            None => Task::none(),
        }
    }

    /// Formats the active tab's note again if it changed while in the reading view, like
    /// when it was edited outside the app or a link in it was renamed.
    fn refresh_preview(&mut self) {
        let root = self.root.clone();
        let index = self.index.clone();
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        let Some(preview) = &tab.preview else {
            return;
        };
        let text = tab.text().unwrap_or_else(|_| tab.content.text());
        if preview.source() != text {
            tab.preview = Some(tab.make_preview(&root, &index));
        }
    }

    /// Starts downloading the web pictures in the active tab's reading view that haven't
    /// been yet.
    fn fetch_pictures(&mut self) -> Task<Message> {
        let Some(preview) = self.active_tab().and_then(|tab| tab.preview.as_ref()) else {
            return Task::none();
        };
        let urls: Vec<String> = preview
            .web_pictures()
            .filter(|url| !self.web.contains_key(*url))
            .map(str::to_string)
            .collect();

        Task::batch(urls.into_iter().map(|url| {
            self.web.insert(url.clone(), web::Picture::Loading);
            Task::perform(web::fetch(url), |(url, picture)| {
                Message::WebPicture(url, picture)
            })
        }))
    }

    /// Formats again the notes in the reading view that embed other notes, except the
    /// tab `saved`, so they show what was just saved.
    fn refresh_embeds(&mut self, saved: u64) {
        let root = self.root.clone();
        let index = self.index.clone();
        for tab in &mut self.tabs {
            if tab.id != saved
                && tab.preview.as_ref().is_some_and(Preview::has_embeds)
                && tab.edit.is_none()
            {
                tab.preview = Some(tab.make_preview(&root, &index));
            }
        }
    }

    /// Adds the picture `file` to the active tab's note: copies it into the campaign's
    /// `pictures` folder, unless it is in the campaign already, and embeds it where the
    /// cursor is, or at the end of the note in the reading view.
    fn add_picture(&mut self, file: &Path) {
        if !preview::is_picture(file) {
            self.status = format!(
                "{} isn't a picture. Pictures can be {}",
                file.file_name().unwrap_or_default().to_string_lossy(),
                preview::PICTURES.join(", ")
            );
            return;
        }
        let Some(tab) = self.active_tab() else {
            self.status = "Open a note first, then add the picture to it".into();
            return;
        };
        if !tab.is_note() {
            return;
        }
        let note = tab.note_path();

        let picture = match campaign::copy_picture(&self.root, file) {
            Ok(picture) => picture,
            Err(error) => {
                self.status = format!("Could not add the picture: {error}");
                return;
            }
        };
        let embed = format!(
            "![[{}]]",
            preview::picture_link(&picture, &note, &self.root)
        );
        let copied = !file.starts_with(&self.root);
        let name = self.relative(&picture);

        let root = self.root.clone();
        let index = self.index.clone();
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        let paste = |content: &mut text_editor::Content| {
            content.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                std::sync::Arc::new(embed.clone()),
            )));
        };
        let added = if let Some(ReadEdit::Block { content, .. }) = &mut tab.edit {
            paste(content);
            true
        } else {
            match &tab.edit {
                Some(_) => false,
                None if tab.reading() => {
                    let text = tab.text().unwrap_or_else(|_| tab.content.text());
                    let text = format!("{}\n\n{embed}\n", text.trim_end());
                    tab.set_text(&text);
                    tab.preview = Some(tab.make_preview(&root, &index));
                    true
                }
                None if tab.shows_form() => tab
                    .form
                    .as_mut()
                    .is_some_and(|form| form.insert_text(&embed)),
                None => {
                    paste(&mut tab.content);
                    true
                }
            }
        };
        if added {
            tab.modified = true;
        }

        self.status = match (added, copied) {
            (true, true) => format!("Added the picture (copied to {name})"),
            (true, false) => format!("Added the picture {name}"),
            (false, _) => format!(
                "Copied the picture to {name}. Click in a text box, then add it again to put it there"
            ),
        };
        if copied {
            self.rescan();
        }
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
            types::named(
                &self.types,
                folder_type.and_then(|ty| ty.as_ref().ok()),
                &id,
            )
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
        let tab = self.tabs.remove(index);
        // Its unsaved changes are gone, so what counts is what is on disk again.
        if tab.modified
            && let Some(path) = &tab.path
        {
            self.recheck_file(path);
        }

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
                self.not_saved(id, format!("Could not save: {error}"));
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
            self.not_saved(
                id,
                format!(
                    "Not saved: {name} was changed outside Dungeon Elements. Choose Reload or Keep Mine above the editor first"
                ),
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
            let old_name = old
                .as_deref()
                .and_then(Path::file_stem)
                .map(|stem| stem.to_string_lossy());
            let written = match tab.note_name() {
                Some(name) => with_alias(&text, &name, old_name.as_deref()),
                None => text.clone(),
            };
            let tag_fields = form.tag_fields();
            let before = old
                .as_deref()
                .and_then(|path| std::fs::read_to_string(path).ok())
                .unwrap_or_default();
            let result = save_named(tab, &written, &form.ty.label());
            let renamed = match (old, &result) {
                (Some(old), Ok(new)) if old != *new => Some((old, new.clone())),
                _ => None,
            };
            let saved = result.as_ref().ok().cloned();
            let task = self.update(Message::Saved(id, result.map(Some)));
            // The alias it was given shows in the form.
            if written != text
                && let Some(index) = self.tabs.iter().position(|tab| tab.id == id)
            {
                let _ = self.reload_tab(index);
            }
            if let Some((old, new)) = renamed {
                let done = format!(
                    "Renamed {} to {}",
                    display_name(&old, false),
                    display_name(&new, false)
                );
                self.relink(&[(old, new)], &done);
            }
            // After the links to its old name are renamed, so its tags' notes don't
            // list it twice.
            if let Some(path) = saved {
                self.sync_tags(&path, &tag_fields, &before, &written);
            }
            return task;
        }

        let path = if save_as { None } else { tab.path.clone() };
        if let Some(form) = &tab.form {
            let before = path
                .as_deref()
                .and_then(|path| std::fs::read_to_string(path).ok())
                .unwrap_or_default();
            self.tag_saves.insert(id, (form.tag_fields(), before));
        }
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

    /// Saves every tab with unsaved changes. Notes without a file yet ask where to go one
    /// at a time, and the status bar sums up once they are all done.
    fn save_all(&mut self) -> Task<Message> {
        let ids: Vec<u64> = self
            .tabs
            .iter()
            .filter(|tab| tab.modified)
            .map(|tab| tab.id)
            .collect();
        if ids.is_empty() {
            self.status = "Nothing to save".into();
            return Task::none();
        }

        // One more than the notes, so notes saved right away (and their renames' status)
        // can't sum it up before the loop is done.
        self.save_all = Some(SaveAll {
            pending: ids.len() + 1,
            ..SaveAll::default()
        });
        // One after another, so only one save dialog is open at a time.
        let tasks = ids
            .into_iter()
            .fold(Task::none(), |tasks, id| tasks.chain(self.save(id, false)));
        if let Some(batch) = &mut self.save_all {
            batch.pending -= 1;
        }
        self.finish_save_all();
        tasks
    }

    /// Reports that tab `id` was not saved, in the Save All summary if one is going.
    fn not_saved(&mut self, id: u64, problem: String) {
        match &mut self.save_all {
            Some(batch) => {
                let name = self
                    .tabs
                    .iter()
                    .find(|tab| tab.id == id)
                    .map(|tab| tab.name(false))
                    .unwrap_or_default();
                let problem = problem
                    .strip_prefix("Could not save: ")
                    .map(|why| format!("{name}: {why}"))
                    .unwrap_or(problem);
                batch.pending = batch.pending.saturating_sub(1);
                batch.problems.push(problem);
                self.finish_save_all();
            }
            None => self.status = problem,
        }
    }

    /// Counts a note as saved in a Save All, if one is going, or as not saved when its
    /// save dialog was cancelled.
    fn saved_one(&mut self, saved: bool) {
        if let Some(batch) = &mut self.save_all {
            batch.pending = batch.pending.saturating_sub(1);
            match saved {
                true => batch.saved += 1,
                false => batch.skipped += 1,
            }
            self.finish_save_all();
        }
    }

    /// Sums up a Save All in the status bar once every note in it is done.
    fn finish_save_all(&mut self) {
        let Some(batch) = self.save_all.take_if(|batch| batch.pending == 0) else {
            return;
        };
        let notes = |count: usize| match count {
            1 => "1 note".to_string(),
            count => format!("{count} notes"),
        };
        let mut status = format!("Saved {}", notes(batch.saved));
        if batch.skipped > 0 {
            status += &format!(". {} not saved (cancelled)", notes(batch.skipped));
        }
        match batch.problems.as_slice() {
            [] => {}
            [problem] => status += &format!(". Not saved: {problem}"),
            [problem, more @ ..] => {
                status += &format!(". Not saved: {problem} (and {} more)", more.len())
            }
        }
        self.status = status;
    }

    /// What making a note in `folder` makes: "New Weapon" in a typed folder.
    fn new_note_label(&self, folder: &Path) -> String {
        match self.types.get(folder) {
            Some(Ok(ty)) => format!("New {}", ty.label()),
            _ => "New Note Here".into(),
        }
    }

    /// The subfolders of `folder` that are a kind of its type with fields of their own,
    /// and the name of one of their notes, like `(items/weapons, "Weapon")`.
    fn subtypes(&self, folder: &Path) -> Vec<(PathBuf, String)> {
        let mut found: Vec<_> = self
            .own_types
            .keys()
            .filter(|path| path.parent() == Some(folder))
            .filter_map(|path| match self.types.get(path) {
                Some(Ok(ty)) if !ty.parents.is_empty() => Some((path.clone(), ty.label())),
                _ => None,
            })
            .collect();
        found.sort();
        found
    }

    /// The types a new folder in `parent` can start with. In a typed folder: a kind of its
    /// type, or its type as it is. Elsewhere: none, the default template's, and the
    /// campaign's own.
    fn folder_templates(&self, parent: &Path) -> Vec<Template> {
        match self.types.get(parent) {
            Some(Ok(ty)) if parent != self.root => {
                let label = ty.label();
                vec![
                    Template {
                        label: format!("A kind of {label}: {label} fields, and its own"),
                        yaml: None,
                        kind_of: true,
                    },
                    Template {
                        label: format!("{label} notes, like {}", self.shown_name(parent)),
                        yaml: None,
                        kind_of: false,
                    },
                ]
            }
            _ => self.templates(),
        }
    }

    /// The types a new folder can start with: none, the default template's, and the campaign's own.
    fn templates(&self) -> Vec<Template> {
        let mut templates = vec![Template::plain()];

        // A starter subfolder's type comes with everything it inherits, like the campaign's own.
        let starters = types::STARTER_TYPES.iter().filter_map(|(folder, _)| {
            let ty = types::starter(folder)?;
            Some((ty.id.clone(), ty.label(), templates::standalone(&ty).ok()?))
        });

        let mut folders: Vec<_> = self.own_types.keys().collect();
        folders.sort();
        // A subfolder's type starts the new folder with everything it inherits.
        let campaign = folders.into_iter().filter_map(|folder| {
            let ty = self.types.get(folder)?.as_ref().ok()?;
            let yaml = ContentType {
                removed_fields: Vec::new(),
                ..ty.clone()
            }
            .to_yaml()
            .ok()?;
            Some((ty.id.clone(), ty.label(), yaml))
        });

        // Types saved for every campaign come with their own label, so they can be told apart.
        let saved: Vec<Template> = templates::saved_types()
            .iter()
            .filter_map(|ty| {
                Some(Template {
                    label: format!("{} (saved type)", ty.label()),
                    yaml: Some(templates::standalone(ty).ok()?),
                    kind_of: false,
                })
            })
            .collect();

        // The campaign's own version of a type wins over the default one with the same id.
        let mut seen: Vec<(String, Template)> = Vec::new();
        for (id, label, yaml) in campaign.chain(starters) {
            if seen.iter().all(|(other, _)| *other != id) {
                seen.push((
                    id,
                    Template {
                        label,
                        yaml: Some(yaml),
                        kind_of: false,
                    },
                ));
            }
        }
        seen.sort_by(|a, b| a.1.label.cmp(&b.1.label));
        templates.extend(seen.into_iter().map(|(_, template)| template));
        templates.extend(saved);
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
        if let Dialog::Walkthrough(editor) = dialog {
            return editor.view(c).map(Message::WalkthroughEditor);
        }
        if let Dialog::Problems = dialog {
            return self.problems_dialog(c);
        }
        if let Dialog::Fix(path) = dialog {
            return self.fix_dialog(c, path);
        }
        if let Dialog::ReplaceField {
            label, value, long, ..
        } = dialog
        {
            let mut buttons = row![
                space::horizontal(),
                button("Cancel")
                    .style(style::secondary(c))
                    .on_press(Message::CloseDialog),
            ]
            .spacing(8);
            if *long {
                buttons = buttons.push(
                    button("Add on a New Line")
                        .style(style::secondary(c))
                        .on_press(Message::FillField(true)),
                );
            }
            buttons = buttons.push(
                button("Replace")
                    .style(style::primary(c))
                    .on_press(Message::FillField(false)),
            );
            return container(
                column![
                    text(format!("Replace {label}?")).size(22).color(c.text),
                    text(format!(
                        "{label} has something in it already. The roll gave:"
                    ))
                    .size(13)
                    .color(c.text_muted),
                    text(value.clone()).size(16).color(c.text),
                    buttons,
                ]
                .spacing(14),
            )
            .width(440)
            .padding(24)
            .style(style::dialog(c))
            .into();
        }

        let label = |label| text(label).size(13).color(c.text_muted);

        let (title, fields, error, confirm, on_confirm): (_, Element<'a, Message>, _, _, _) =
            match dialog {
                Dialog::AddField { label: name, kind, error } => (
                    "Add a field to this note".to_string(),
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
                    parent,
                    name,
                    template,
                    error,
                } => (
                    if *parent == self.root {
                        "New folder".to_string()
                    } else {
                        format!("New subfolder in {}", self.relative(parent))
                    },
                    column![
                        column![
                            label("Name"),
                            text_input(
                                if self.types.contains_key(parent) && *parent != self.root {
                                    "weapons"
                                } else {
                                    "factions"
                                },
                                name
                            )
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
                                self.folder_templates(parent),
                                Some(template.clone()),
                                Message::NewFolderTemplate
                            )
                            .padding(8)
                            .width(Fill)
                            .style(style::choice(c))
                            .menu_style(style::choice_menu(c)),
                            text(match (template.kind_of, self.types.get(parent)) {
                                (true, Some(Ok(ty))) => format!(
                                    "Like a child class: its notes have every {0} field, and fields of their own, which you add next in Edit > Types. Changes to {0}'s fields show up in it too.",
                                    ty.label()
                                ),
                                (false, Some(Ok(ty))) if *parent != self.root => format!(
                                    "Its notes have the same fields as {}'s. It can be made a kind of {} with fields of its own later, in Edit > Types.",
                                    self.shown_name(parent),
                                    ty.label()
                                ),
                                _ => "Picking a type gives every note in the folder its fields. You can change them later in Edit > Types.".into(),
                            })
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
                Dialog::Rename { path, name, error } => (
                    format!("Rename {}", self.shown_name(path)),
                    column![
                        label("New name"),
                        text_input("Name", name)
                            .id(DIALOG_INPUT)
                            .on_input(Message::RenameName)
                            .on_submit(Message::Rename)
                            .padding(8)
                            .style(style::input(c)),
                        text(if path.is_dir() {
                            "Links to the notes in it are updated to match."
                        } else {
                            "Its name field changes too, and links to it are updated to match."
                        })
                        .size(12)
                        .color(c.text_muted),
                    ]
                    .spacing(4)
                    .into(),
                    error,
                    "Rename",
                    Message::Rename,
                ),
                Dialog::Move {
                    path,
                    folder,
                    error,
                } => (
                    format!("Move {}", self.shown_name(path)),
                    column![
                        label("Into the folder"),
                        pick_list(self.move_targets(path), folder.clone(), Message::MoveFolder)
                            .placeholder("Choose a folder...")
                            .padding(8)
                            .width(Fill)
                            .style(style::choice(c))
                            .menu_style(style::choice_menu(c)),
                        text(if path.is_dir() {
                            "Everything in it moves too, and links to its notes are updated to match."
                        } else {
                            "It takes the new folder's type, keeping every field that has something in it, and links to it are updated to match."
                        })
                        .size(12)
                        .color(c.text_muted),
                    ]
                    .spacing(4)
                    .into(),
                    error,
                    "Move",
                    Message::Move,
                ),
                Dialog::Tags(changes) => {
                    let tag_list = |tags: &[(String, String)]| {
                        tags.iter()
                            .map(|(_, tag)| tag.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    let lines = changes.iter().map(|change| {
                        let mut parts = Vec::new();
                        if !change.add.is_empty() {
                            parts.push(format!("tagged {}", tag_list(&change.add)));
                        }
                        if !change.remove.is_empty() {
                            parts.push(format!("no longer tagged {}", tag_list(&change.remove)));
                        }
                        text(format!(
                            "{}: {}",
                            self.tree_name(&change.note, false),
                            parts.join("; ")
                        ))
                        .size(14)
                        .color(c.text)
                        .into()
                    });
                    (
                        "Tags changed outside Dungeon Elements".to_string(),
                        column![
                            text("These notes' tags were changed in another app. Update the tag notes to match? Notes with new tags are added to their lists (new tags get a note), and taken off the lists of tags they no longer have.")
                                .size(13)
                                .color(c.text_muted),
                            column(lines).spacing(6),
                        ]
                        .spacing(12)
                        .into(),
                        &None,
                        "Update Tag Notes",
                        Message::UpdateTagNotes,
                    )
                }
                Dialog::NewTable {
                    name,
                    folder,
                    die,
                    columns,
                    rows,
                    pasted,
                    error,
                } => {
                    let faces = tables::markdown(&tables::Blank {
                        die: (*die).to_string(),
                        columns: Vec::new(),
                        rows: None,
                        pasted: String::new(),
                    })
                    .lines()
                    .count()
                        - 2;
                    let mut column_inputs = column![].spacing(4);
                    for (index, value) in columns.iter().enumerate() {
                        let mut line = row![
                            text_input(if index == 0 { "Result" } else { "Another column" }, value)
                                .on_input(move |value| Message::NewTableColumn(index, value))
                                .padding(8)
                                .style(style::input(c)),
                        ]
                        .spacing(4)
                        .align_y(Center);
                        if columns.len() > 1 {
                            line = line.push(
                                button(text("✕").size(11))
                                    .padding([4, 8])
                                    .style(style::flat(c, false))
                                    .on_press(Message::NewTableRemoveColumn(index)),
                            );
                        }
                        column_inputs = column_inputs.push(line);
                    }
                    column_inputs = column_inputs.push(
                        button(text("+ Add column").size(12))
                            .padding([3, 10])
                            .style(style::secondary(c))
                            .on_press(Message::NewTableAddColumn),
                    );
                    (
                        "New roll table".to_string(),
                        column![
                            column![
                                label("Name"),
                                text_input("Weather", name)
                                    .id(DIALOG_INPUT)
                                    .on_input(Message::NewTableName)
                                    .on_submit(Message::CreateTable)
                                    .padding(8)
                                    .style(style::input(c)),
                            ]
                            .spacing(4),
                            column![
                                label("Folder"),
                                pick_list(
                                    self.folder_choices(&folder.path),
                                    Some(folder.clone()),
                                    Message::NewTableFolder
                                )
                                .padding(8)
                                .width(Fill)
                                .style(style::choice(c))
                                .menu_style(style::choice_menu(c)),
                            ]
                            .spacing(4),
                            row![
                                column![
                                    label("Die"),
                                    pick_list(&tables::DICE[..], Some(*die), Message::NewTableDie)
                                        .padding(8)
                                        .width(Fill)
                                        .style(style::choice(c))
                                        .menu_style(style::choice_menu(c)),
                                ]
                                .spacing(4)
                                .width(Fill),
                                column![
                                    label("Rows"),
                                    text_input(&faces.to_string(), rows)
                                        .on_input(Message::NewTableRows)
                                        .on_submit(Message::CreateTable)
                                        .padding(8)
                                        .style(style::input(c)),
                                ]
                                .spacing(4)
                                .width(Fill),
                            ]
                            .spacing(12),
                            column![label("Result columns"), column_inputs].spacing(4),
                            column![
                                label("Paste a list (optional)"),
                                text_editor(pasted)
                                    .on_action(Message::NewTablePasted)
                                    .placeholder("One result a line. Numbers like 1. or 3-4 at the start are kept.")
                                    .height(110)
                                    .padding(8)
                                    .size(13)
                                    .style(style::editor(c)),
                            ]
                            .spacing(4),
                            text("Fewer rows than the die has numbers split them evenly, so a d100 with 20 rows gets 01-05, 06-10 and so on. A pasted list picks the die that fits it.")
                                .size(12)
                                .color(c.text_muted),
                        ]
                        .spacing(14)
                        .into(),
                        error,
                        "Create",
                        Message::CreateTable,
                    )
                }
                Dialog::NoteFromRoll {
                    name,
                    folder,
                    error,
                } => (
                    "New note from the roll".to_string(),
                    column![
                        column![
                            label("Name"),
                            text_input("Name", name)
                                .id(DIALOG_INPUT)
                                .on_input(Message::NoteFromRollName)
                                .on_submit(Message::CreateNoteFromRoll)
                                .padding(8)
                                .style(style::input(c)),
                        ]
                        .spacing(4),
                        column![
                            label("Folder"),
                            pick_list(
                                self.folder_choices(&folder.path),
                                Some(folder.clone()),
                                Message::NoteFromRollFolder
                            )
                            .padding(8)
                            .width(Fill)
                            .style(style::choice(c))
                            .menu_style(style::choice_menu(c)),
                            text("The note gets the folder's fields, like any new note there.")
                                .size(12)
                                .color(c.text_muted),
                        ]
                        .spacing(4),
                    ]
                    .spacing(16)
                    .into(),
                    error,
                    "Create",
                    Message::CreateNoteFromRoll,
                ),
                Dialog::Types(_)
                | Dialog::Walkthrough(_)
                | Dialog::Problems
                | Dialog::Fix(_)
                | Dialog::ReplaceField { .. } => {
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

    /// A note's name, or a folder's path in the campaign, for dialog titles.
    fn shown_name(&self, path: &Path) -> String {
        if path.is_dir() {
            self.relative(path)
        } else {
            self.tree_name(path, false)
        }
    }

    /// The folders the note or folder at `path` can move to: every folder but the one it
    /// is in (and, for a folder, the ones inside it).
    fn move_targets(&self, path: &Path) -> Vec<FolderChoice> {
        let mut folders = vec![self.root.clone()];
        collect_folders(&self.tree, &mut folders);
        folders
            .into_iter()
            .filter(|folder| Some(folder.as_path()) != path.parent() && !folder.starts_with(path))
            .map(|folder| FolderChoice {
                label: if folder == self.root {
                    "The campaign folder".into()
                } else {
                    self.relative(&folder)
                },
                path: folder,
            })
            .collect()
    }

    /// Offers fixes for each problem of the note at `path`, the active tab.
    fn fix_dialog<'a>(&'a self, c: Colors, path: &'a Path) -> Element<'a, Message> {
        let tab = self
            .tabs
            .iter()
            .find(|tab| tab.path.as_deref() == Some(path));
        let ty = tab.and_then(|tab| tab.form.as_ref()).map(|form| &form.ty);
        let fix_button = |label: String, fix: Fix, primary: bool| {
            button(text(label).size(13))
                .padding([3, 10])
                .style(move |theme, status| {
                    if primary {
                        style::primary(c)(theme, status)
                    } else {
                        style::secondary(c)(theme, status)
                    }
                })
                .on_press(Message::Fix(fix))
        };

        let rows = self.problems_in(path).map(|problem| {
            let mut buttons: Vec<Element<'a, Message>> = Vec::new();
            match &problem.issue {
                check::Issue::Link { target, .. } => {
                    for (link, label) in self.link_fixes(target, path) {
                        buttons.push(
                            fix_button(
                                format!("Link to {label}"),
                                Fix::Relink {
                                    target: target.clone(),
                                    to: link,
                                },
                                buttons.is_empty(),
                            )
                            .into(),
                        );
                    }
                    buttons.push(
                        fix_button(
                            format!("Make a note called {target}"),
                            Fix::MakeNote(target.clone()),
                            false,
                        )
                        .into(),
                    );
                    buttons.push(
                        fix_button("Remove the link".into(), Fix::Unlink(target.clone()), false)
                            .into(),
                    );
                    buttons.push(
                        fix_button("Ignore".into(), Fix::Ignore(target.clone()), false).into(),
                    );
                }
                check::Issue::InFrontMatter { key } => {
                    let label = ty
                        .and_then(|ty| ty.fields.iter().find(|field| field.key == *key))
                        .map_or_else(|| key.clone(), types::Field::label);
                    buttons.push(
                        fix_button(
                            format!("Move it to a ## {label} section"),
                            Fix::MoveToSection(key.clone()),
                            true,
                        )
                        .into(),
                    );
                }
                check::Issue::UnknownType => {
                    let folder_type = path
                        .parent()
                        .and_then(|folder| self.types.get(folder)?.as_ref().ok());
                    if let Some(folder_type) = folder_type {
                        buttons.push(
                            fix_button(
                                format!("Use this folder's type ({})", folder_type.label()),
                                Fix::SetType(Some(folder_type.id.clone())),
                                true,
                            )
                            .into(),
                        );
                    }
                    buttons.push(
                        fix_button("Remove its type line".into(), Fix::SetType(None), false).into(),
                    );
                }
                check::Issue::NotAnOption { key, value } => {
                    let options = ty
                        .and_then(|ty| ty.fields.iter().find(|field| field.key == *key))
                        .map(|field| field.options.as_slice())
                        .unwrap_or_default();
                    if let Some(option) = options
                        .iter()
                        .find(|option| option.trim().eq_ignore_ascii_case(value.trim()))
                    {
                        buttons.push(
                            fix_button(
                                format!("Change it to {option}"),
                                Fix::SetOption {
                                    key: key.clone(),
                                    value: option.clone(),
                                },
                                true,
                            )
                            .into(),
                        );
                    }
                    buttons.push(
                        button(text("Pick one in the form").size(13))
                            .padding([3, 10])
                            .style(style::secondary(c))
                            .on_press(Message::CloseDialog)
                            .into(),
                    );
                }
                check::Issue::Other => {}
            }

            column![
                text(&problem.message).size(13).color(c.text),
                row(buttons).spacing(6).wrap(),
            ]
            .spacing(6)
            .into()
        });

        let content = column![
            text(format!("Fix {}", display_name(path, false)))
                .size(22)
                .color(c.text),
            text(
                "A fix changes the note right away. If it had no unsaved changes, it is saved too."
            )
            .size(12)
            .color(c.text_muted),
            scrollable(column(rows).spacing(16)).height(iced::Shrink),
            row![
                button("Show Markdown Source")
                    .style(style::secondary(c))
                    .on_press(Message::Fix(Fix::ShowSource)),
                space::horizontal(),
                button("Close")
                    .style(style::secondary(c))
                    .on_press(Message::CloseDialog),
            ],
        ]
        .spacing(16);

        container(content)
            .width(600)
            .max_height(600)
            .padding(24)
            .style(style::dialog(c))
            .into()
    }

    /// The notes a broken link to `target` in the note at `path` may have meant, as
    /// `(link text, label)`: notes spelled almost the same, then notes whose name has it in.
    fn link_fixes(&self, target: &str, path: &Path) -> Vec<(String, String)> {
        let mut fixes = self.index.similar(target, path, 3);
        let name = target.rsplit('/').next().unwrap_or(target);
        for suggestion in self.index.suggest(name, path, None, 3) {
            if fixes.len() < 3 && !fixes.iter().any(|(link, _)| *link == suggestion.0) {
                fixes.push(suggestion);
            }
        }
        fixes
    }

    /// Makes `fix` to the note in the open fix dialog.
    fn fix(&mut self, fix: Fix) -> Task<Message> {
        let Some(Dialog::Fix(path)) = &self.dialog else {
            return Task::none();
        };
        let path = path.clone();
        let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.path.as_ref() == Some(&path))
        else {
            self.dialog = None;
            return Task::none();
        };
        let name = display_name(&path, false);

        let text = match &fix {
            Fix::MakeNote(target) => {
                self.dialog = None;
                let folder = self.link_field_folder(index, target);
                return self.follow_link_into(target, folder);
            }
            Fix::Ignore(target) => {
                self.ignored
                    .insert((path.clone(), links::normalize(target)));
                self.problems.retain(|problem| {
                    !(problem.path == path
                        && matches!(&problem.issue, check::Issue::Link { target: t, .. }
                            if links::normalize(t) == links::normalize(target)))
                });
                self.status = format!(
                    "Leaving the links to [[{target}]] in {name} as they are until the campaign is closed"
                );
                self.close_fix_dialog_if_done(&path);
                return Task::none();
            }
            Fix::ShowSource => {
                self.dialog = None;
                let tab = &mut self.tabs[index];
                tab.preview = None;
                if let Some(form) = &tab.form
                    && !tab.source
                    && let Ok(text) = form.to_markdown()
                {
                    tab.content = text_editor::Content::with_text(&text);
                    tab.source = true;
                }
                return Task::none();
            }
            _ => match self.tabs[index].text() {
                Ok(text) => text,
                Err(error) => {
                    self.status = format!("Could not fix {name}: {error}");
                    return Task::none();
                }
            },
        };

        let fixed = match &fix {
            Fix::Relink { target, to } => links::retarget_note(&text, target, to)
                .map(|(text, _)| text)
                .ok_or_else(|| format!("no link to [[{target}]] was found")),
            Fix::Unlink(target) => links::unlink_all(&text, target)
                .ok_or_else(|| format!("no link to [[{target}]] was found")),
            Fix::MoveToSection(key) => {
                let label = self.tabs[index]
                    .form
                    .as_ref()
                    .and_then(|form| form.ty.fields.iter().find(|field| field.key == *key))
                    .map_or_else(|| key.clone(), types::Field::label);
                fix::move_to_section(&text, key, &label)
            }
            Fix::SetType(id) => fix::set_value(&text, "type", id.as_deref()),
            Fix::SetOption { key, value } => fix::set_value(&text, key, Some(value)),
            Fix::MakeNote(_) | Fix::Ignore(_) | Fix::ShowSource => unreachable!(),
        };
        let fixed = match fixed {
            Ok(fixed) => fixed,
            Err(error) => {
                self.status = format!("Could not fix {name}: {error}");
                return Task::none();
            }
        };

        let was_modified = self.tabs[index].modified;
        let form = self.form_for(&path, &fixed);
        let tab = &mut self.tabs[index];
        tab.content = text_editor::Content::with_text(&fixed);
        tab.edit = None;
        // A note whose front matter can't be read any more shows its source.
        tab.form = form.ok().flatten();
        if tab.form.is_none() {
            tab.source = false;
        }
        tab.modified = true;
        self.recheck_tab(index);
        self.status = format!("Fixed {name}");
        self.close_fix_dialog_if_done(&path);

        if was_modified {
            self.status.push_str(" (not saved yet)");
            Task::none()
        } else {
            let id = self.tabs[index].id;
            let task = self.save(id, false);
            if self.status.starts_with("Saved") {
                self.status = format!("Fixed {name} and saved it");
            }
            task
        }
    }

    /// Closes the fix dialog once the note at `path` has no problems left.
    fn close_fix_dialog_if_done(&mut self, path: &Path) {
        if self.problems_in(path).next().is_none() {
            self.dialog = None;
        }
    }

    /// The folder a note made for a broken link to `target` goes in: the one its link
    /// field links to, if `target` is in a link field of the note in tab `index`.
    fn link_field_folder(&self, index: usize, target: &str) -> Option<PathBuf> {
        let tab = &self.tabs[index];
        let ty = &tab.form.as_ref()?.ty;
        let note = note::parse(&tab.text().ok()?).ok()?;
        let field = ty.fields.iter().find(|field| {
            let Some(value) = note.front.get(field.key.as_str()) else {
                return false;
            };
            match field.kind {
                types::Kind::Link => {
                    links::normalize(note::unlink(&note::value_to_text(value)))
                        == links::normalize(target)
                }
                // Any of a links field's links.
                types::Kind::Links => links::targets(&note::value_to_text(value))
                    .iter()
                    .any(|linked| links::normalize(linked) == links::normalize(target)),
                _ => false,
            }
        })?;
        self.link_folder(field)
    }

    /// The name the file tree shows for the file at `path`: the note's name as written,
    /// or (with File Extensions on, or if it has none) its file name.
    fn tree_name(&self, path: &Path, show_extensions: bool) -> String {
        match self.titles.get(path) {
            Some(title) if !show_extensions => title.clone(),
            _ => display_name(path, show_extensions),
        }
    }

    /// The notes that link to the note at `path`, for the reading view's "Linked from":
    /// grouped by their folder's type, then by name.
    fn linked_from(&self, path: &Path) -> Vec<preview::Backlink> {
        let mut found: Vec<preview::Backlink> = self
            .backlinks
            .to(path)
            .into_iter()
            .filter_map(|(from, mentions)| {
                let link = self.index.link_text(from, path, false)?;
                let folder = from.parent()?;
                let ty = self.types.get(folder).and_then(|ty| ty.as_ref().ok());
                let group = match ty {
                    Some(ty) => ty.label(),
                    None if folder == self.root => "Other notes".into(),
                    None => types::title_case(&display_name(folder, false)),
                };
                let lines = mentions
                    .into_iter()
                    .map(|mention| match mention {
                        backlinks::Mention::Field { key, value } => {
                            let label = ty
                                .and_then(|ty| ty.fields.iter().find(|field| field.key == *key))
                                .map_or_else(|| types::title_case(key), types::Field::label);
                            format!("{label}: {value}")
                        }
                        backlinks::Mention::Line(line) => line.clone(),
                    })
                    .collect();
                Some(preview::Backlink {
                    group,
                    name: self.tree_name(from, false),
                    link,
                    lines,
                })
            })
            .collect();
        found.sort_by_cached_key(|backlink| {
            (backlink.group.to_lowercase(), backlink.name.to_lowercase())
        });
        found
    }

    /// The name on a tab, like [`Self::tree_name`].
    fn tab_name(&self, tab: &Tab, show_extensions: bool) -> String {
        match &tab.path {
            Some(path) => self.tree_name(path, show_extensions),
            None => tab.name(show_extensions),
        }
    }

    /// The ⚠ after a file tree row whose file has problems (or, for a collapsed folder,
    /// something inside it). Hovering shows what is wrong; clicking offers fixes.
    fn warning_button<'a>(&'a self, c: Colors, entry: &'a Entry) -> Element<'a, Message> {
        let (tip, message) = match entry.children {
            None => {
                let mut lines: Vec<String> = self
                    .problems_in(&entry.path)
                    .map(|problem| format!("• {}", problem.message))
                    .collect();
                let fixable = campaign::is_markdown(&entry.path);
                lines.push(if fixable {
                    "Click the ⚠ to fix it".into()
                } else {
                    "Click the ⚠ to see the problems".into()
                });
                let message = if fixable {
                    Message::FixProblems(entry.path.clone())
                } else {
                    Message::ShowProblems
                };
                (lines.join("\n"), message)
            }
            Some(_) => {
                let files = problem_files_in(&self.problems, &entry.path);
                (
                    format!(
                        "{files} {} in this folder {} problems. Click the ⚠ to see them",
                        if files == 1 { "file" } else { "files" },
                        if files == 1 { "has" } else { "have" }
                    ),
                    Message::ShowProblems,
                )
            }
        };

        tooltip(
            button(text("⚠").size(12).color(c.warning))
                .padding([0, 4])
                .style(style::flat(c, false))
                .on_press(message),
            container(text(tip).size(12).color(c.text))
                .padding([4, 8])
                .max_width(420)
                .style(style::dropdown(c)),
            tooltip::Position::Bottom,
        )
        .into()
    }

    /// Lists the files with problems; clicking one opens it.
    fn problems_dialog(&self, c: Colors) -> Element<'_, Message> {
        let rows = self.problems.iter().map(|problem| {
            let fixable = campaign::is_markdown(&problem.path);
            column![
                row![
                    button(text(self.relative(&problem.path)).size(13).color(c.accent))
                        .padding(0)
                        .style(style::flat(c, false))
                        .on_press(Message::OpenProblem(problem.path.clone())),
                    space::horizontal(),
                ]
                .push(fixable.then(|| {
                    button(text("Fix...").size(12))
                        .padding([1, 8])
                        .style(style::secondary(c))
                        .on_press(Message::FixProblems(problem.path.clone()))
                })),
                text(&problem.message).size(13).color(c.text),
            ]
            .spacing(2)
            .into()
        });

        let content = column![
            text("Problems").size(22).color(c.text),
            text("These files were probably edited outside Dungeon Elements, and parts of them can't be shown as intended, or their links point nowhere. Nothing has been changed. Choose Fix... to see ways to fix a note.")
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
        // Leaving what is being edited in the reading view puts it back into the note first.
        if matches!(
            message,
            Message::Save
                | Message::SaveAs
                | Message::SaveTab(_)
                | Message::SaveAll
                | Message::SelectTab(_)
                | Message::CloseTab(_)
                | Message::CloseActiveTab
                | Message::CloseCampaign
                | Message::ToggleSource
                | Message::SetSource(_)
                | Message::SetReading(_)
                | Message::PreviewLink(_)
                | Message::FollowLink(_)
                | Message::NewNote
                | Message::OpenEntry(_)
                | Message::EditField(_)
                | Message::AddText
                | Message::FinishEditing
        ) {
            self.finish_editing();
        }

        // While the walkthrough shows, typing goes into its answer box.
        if self.walkthrough_shown()
            && matches!(
                message,
                Message::Form(_)
                    | Message::CompleteLink(_)
                    | Message::TabField(_)
                    | Message::FieldTabbed(_)
                    | Message::FillField(_)
            )
        {
            return self.walkthrough_input(message);
        }

        match message {
            Message::ShowWalkthrough => {
                self.show_walkthrough();
                self.focus_walkthrough()
            }
            Message::Walkthrough(message) => self.walkthrough_message(message),
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
            Message::ToggleDice => {
                self.dice.open = !self.dice.open;
                Task::none()
            }
            Message::Dice(dice::Message::Copy(result)) => {
                self.status = format!("Copied {result}");
                iced::clipboard::write(result)
            }
            Message::Dice(dice::Message::NewNote(name)) => {
                // The folder picked last time, since rolls of one kind tend to go together.
                let folder = self
                    .roll_folder
                    .clone()
                    .filter(|folder| folder.is_dir())
                    .unwrap_or_else(|| self.root.clone());
                self.dialog = Some(Dialog::NoteFromRoll {
                    name,
                    folder: self.folder_choice(&folder),
                    error: None,
                });
                iced::widget::operation::focus(DIALOG_INPUT)
            }
            Message::Dice(dice::Message::RollExpression)
                if dice::Expression::parse(self.dice.expression()).is_err() =>
            {
                // A table's name rolls the table.
                match self.table_named(self.dice.expression()) {
                    Some((path, name, table, each_column)) => {
                        self.roll_found(&path, &name, &table, each_column);
                    }
                    None => self.dice.update(dice::Message::RollExpression),
                }
                Task::none()
            }
            Message::Dice(message) => {
                self.dice.update(message);
                Task::none()
            }
            Message::RollField(label, expression) => {
                self.dice.open = true;
                self.dice.roll_field(&label, &expression);
                Task::none()
            }
            Message::SearchChanged(query) => {
                self.search.query = query;
                self.refresh_search();
                Task::none()
            }
            Message::SearchIn(folder) => {
                self.search.folder = Some(folder);
                self.refresh_search();
                iced::widget::operation::focus(SEARCH)
            }
            Message::SearchEverywhere => {
                self.search.folder = None;
                self.refresh_search();
                Task::none()
            }
            Message::ClearSearch => {
                self.search = Search::default();
                Task::none()
            }
            Message::FocusSearch => iced::widget::operation::focus(SEARCH),
            Message::ShowQuests => {
                self.quests.open = true;
                self.tables.open = false;
                self.search = Search::default();
                self.refresh_quests();
                Task::none()
            }
            Message::Quests(message) => match message {
                quests::Message::Open(path) => self.update(Message::OpenEntry(path)),
                quests::Message::Tick(path, at) => {
                    self.tick_step(&path, at);
                    Task::none()
                }
                quests::Message::Close => {
                    self.quests.open = false;
                    Task::none()
                }
                message => {
                    self.quests.update(message);
                    Task::none()
                }
            },
            Message::ShowTables => {
                self.tables.open = true;
                self.quests.open = false;
                self.search = Search::default();
                self.refresh_tables();
                Task::none()
            }
            Message::Tables(message) => match message {
                tables::Message::Open(path) => self.update(Message::OpenEntry(path)),
                tables::Message::Roll(path, start) => {
                    self.roll_table(&path, start);
                    Task::none()
                }
                tables::Message::EachColumn(path, start, on) => {
                    self.set_each_column(&path, start, on);
                    Task::none()
                }
                tables::Message::RollBuiltIn(name) => {
                    let from = self.root.join("Untitled.md");
                    if let Some((name, table, each_column)) = self.linked_table(&name, &from) {
                        self.roll_found(&from, &name, &table, each_column);
                    }
                    Task::none()
                }
                tables::Message::CopyBuiltIn(name) => {
                    self.copy_built_in(&name);
                    Task::none()
                }
                tables::Message::ToggleGroup(group) => {
                    let open = &mut self.table_settings.open_groups;
                    if !open.remove(group.key()) {
                        open.insert(group.key().to_string());
                    }
                    self.tables.open_groups = self.table_settings.open_groups.clone();
                    if let Err(error) = self.table_settings.save(&self.root) {
                        self.status = format!("Could not remember which groups are open: {error}");
                    }
                    Task::none()
                }
                tables::Message::Edit(path, start) => self.edit_table_in(path, start),
                tables::Message::EditBuiltIn(name) => {
                    self.copy_built_in(&name);
                    let Some(path) = self.active_tab().and_then(|tab| tab.path.clone()) else {
                        return Task::none();
                    };
                    let start = self.note_text(&path).and_then(|text| {
                        tables::find(&text).first().map(|table| table.range.start)
                    });
                    match start {
                        Some(start) => self.edit_table_in(path, start),
                        None => Task::none(),
                    }
                }
                tables::Message::Menu(pick, position) => {
                    self.context = Some((Target::Table(pick), position));
                    Task::none()
                }
                tables::Message::New => self.update(Message::ShowNewTable(None)),
                tables::Message::Close => {
                    self.tables.open = false;
                    Task::none()
                }
            },
            Message::EditTable(index) => self.edit_table(index),
            Message::TableEditor(message) => {
                let (root, index) = (self.root.clone(), self.index.clone());
                let Some(tab) = self.active_tab_mut() else {
                    return Task::none();
                };
                let path = tab.note_path();
                let Some(ReadEdit::Table { editor, .. }) = &mut tab.edit else {
                    return Task::none();
                };
                // Roll to try it rolls the table as it is in the editor.
                if matches!(message, table_editor::Message::Roll) {
                    let (name, table) = (editor.name(), editor.table());
                    let each_column = editor.each_column();
                    if let Some(table) = table {
                        let each_column = each_column.unwrap_or_else(|| {
                            self.rolls_each_column(&path, table.heading.as_deref())
                        });
                        self.roll_found_as_is(&path, &name, &table, each_column);
                    }
                    return Task::none();
                }
                match editor.update(message) {
                    table_editor::Outcome::Editing => match editor.take_focus() {
                        Some(id) => {
                            iced::advanced::widget::operate(form::tab(vec![id], false)).discard()
                        }
                        None => Task::none(),
                    },
                    table_editor::Outcome::Done => {
                        self.finish_editing();
                        Task::none()
                    }
                    table_editor::Outcome::Cancel => {
                        // A table just added goes again.
                        if let Some(ReadEdit::Table {
                            added: Some((text, modified)),
                            ..
                        }) = tab.edit.take()
                        {
                            tab.set_text(&text);
                            tab.modified = modified;
                            tab.preview = Some(tab.make_preview(&root, &index));
                        }
                        Task::none()
                    }
                }
            }
            Message::RollTable(start) => {
                if let Some(path) = self.active_tab().map(Tab::note_path) {
                    self.roll_table(&path, start);
                }
                Task::none()
            }
            Message::TableEachColumn(start, on) => {
                if let Some(path) = self.active_tab().map(Tab::note_path) {
                    self.set_each_column(&path, start, on);
                }
                Task::none()
            }
            Message::ShowNewTable(folder) => {
                let folder = folder.unwrap_or_else(|| self.root.join(TABLES_FOLDER));
                self.dialog = Some(Dialog::NewTable {
                    name: String::new(),
                    folder: self.folder_choice(&folder),
                    die: "d6",
                    columns: vec!["Result".into()],
                    rows: String::new(),
                    pasted: text_editor::Content::new(),
                    error: None,
                });
                iced::widget::operation::focus(DIALOG_INPUT)
            }
            Message::NewTableName(value) => {
                if let Some(Dialog::NewTable { name, error, .. }) = &mut self.dialog {
                    *name = value;
                    *error = None;
                }
                Task::none()
            }
            Message::NewTableFolder(choice) => {
                if let Some(Dialog::NewTable { folder, .. }) = &mut self.dialog {
                    *folder = choice;
                }
                Task::none()
            }
            Message::NewTableDie(value) => {
                if let Some(Dialog::NewTable { die, .. }) = &mut self.dialog {
                    *die = value;
                }
                Task::none()
            }
            Message::NewTableColumn(index, value) => {
                if let Some(Dialog::NewTable { columns, .. }) = &mut self.dialog
                    && let Some(column) = columns.get_mut(index)
                {
                    *column = value;
                }
                Task::none()
            }
            Message::NewTableAddColumn => {
                if let Some(Dialog::NewTable { columns, .. }) = &mut self.dialog {
                    columns.push(String::new());
                }
                Task::none()
            }
            Message::NewTableRemoveColumn(index) => {
                if let Some(Dialog::NewTable { columns, .. }) = &mut self.dialog
                    && columns.len() > 1
                    && index < columns.len()
                {
                    columns.remove(index);
                }
                Task::none()
            }
            Message::NewTableRows(value) => {
                if let Some(Dialog::NewTable { rows, .. }) = &mut self.dialog {
                    *rows = value.chars().filter(char::is_ascii_digit).take(3).collect();
                }
                Task::none()
            }
            Message::NewTablePasted(action) => {
                if let Some(Dialog::NewTable { pasted, die, .. }) = &mut self.dialog {
                    let edits = action.is_edit();
                    pasted.perform(action);
                    // The die follows what is pasted, so the list fits it.
                    if edits && let Some(fits) = tables::die_for_pasted(&pasted.text()) {
                        *die = fits;
                    }
                }
                Task::none()
            }
            Message::CreateTable => match self.create_table() {
                Ok(task) => task,
                Err(message) => {
                    self.dialog_result(Err(message));
                    Task::none()
                }
            },
            Message::NoteFromRollName(value) => {
                if let Some(Dialog::NoteFromRoll { name, error, .. }) = &mut self.dialog {
                    *name = value;
                    *error = None;
                }
                Task::none()
            }
            Message::NoteFromRollFolder(choice) => {
                if let Some(Dialog::NoteFromRoll { folder, .. }) = &mut self.dialog {
                    *folder = choice;
                }
                Task::none()
            }
            Message::CreateNoteFromRoll => {
                let Some(Dialog::NoteFromRoll { name, folder, .. }) = &self.dialog else {
                    return Task::none();
                };
                let (name, folder) = (name.trim().to_string(), folder.path.clone());
                if let Err(error) = campaign::validate_name(&name) {
                    self.dialog_result(Err(error));
                    return Task::none();
                }
                let ty = self
                    .types
                    .get(&folder)
                    .and_then(|ty| ty.as_ref().ok())
                    .cloned();
                match create_linked_note(&self.root, &name, Some(&folder), ty.as_ref()) {
                    Ok(path) => {
                        self.dialog = None;
                        self.roll_folder = Some(folder.clone());
                        self.rescan();
                        self.expanded.insert(folder);
                        self.open_file(path.clone());
                        // A new note is empty, so it opens ready to write in.
                        if let Some(tab) = self.active_tab_mut() {
                            tab.preview = None;
                        }
                        self.status = format!("Created {}", self.relative(&path));
                    }
                    Err(_) if path_taken(&folder, &name) => {
                        self.dialog_result(Err(format!("{name} is already a note in that folder")));
                    }
                    Err(error) => self.dialog_result(Err(error)),
                }
                Task::none()
            }
            Message::OpenFirstHit => match self.search.hits.first() {
                Some(hit) => {
                    let path = hit.path.clone();
                    self.update(Message::OpenEntry(path))
                }
                None => Task::none(),
            },
            Message::ExpandAll => {
                self.expanded = folders_of(&self.tree).into_iter().collect();
                Task::none()
            }
            Message::CollapseAll => {
                self.expanded.clear();
                Task::none()
            }
            Message::RefreshTree => {
                self.status.clear();
                self.pick_up_outside_changes();
                if self.status.is_empty() {
                    self.status = problems_summary(&self.problems)
                        .unwrap_or_else(|| "Refreshed; no problems found".into());
                }
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
            Message::ShowNewFolder => self.update(Message::ShowNewFolderIn(self.root.clone())),
            Message::ShowNewFolderIn(parent) => {
                let template = self
                    .folder_templates(&parent)
                    .into_iter()
                    .next()
                    .unwrap_or_else(Template::plain);
                self.dialog = Some(Dialog::NewFolder {
                    parent,
                    name: String::new(),
                    template,
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
                    parent,
                    name,
                    template,
                    error,
                }) = &mut self.dialog
                else {
                    return Task::none();
                };

                // A kind of the parent's type is named for one of its notes ("Weapon").
                let yaml = match template.kind_of {
                    true => {
                        let label = types::singular(name.trim());
                        ContentType {
                            id: types::key_from(&label),
                            label: Some(label),
                            icon: None,
                            fields: Vec::new(),
                            removed_fields: Vec::new(),
                            parents: Vec::new(),
                        }
                        .to_yaml()
                        .ok()
                    }
                    false => template.yaml.clone(),
                };
                let kind_of = template.kind_of;
                match campaign::create_folder(parent, name, yaml.as_deref()) {
                    Ok(path) => {
                        let parent = parent.clone();
                        self.status = format!("Created the folder {}", self.relative(&path));
                        self.expanded.insert(parent);
                        self.dialog = None;
                        self.rescan();
                        // Its own fields are added next.
                        if kind_of {
                            return self.update(Message::EditType(path));
                        }
                    }
                    Err(message) => *error = Some(message),
                }
                Task::none()
            }
            Message::CloseDialog => {
                self.dialog = None;
                Task::none()
            }
            Message::UpdateTagNotes => {
                let Some(Dialog::Tags(changes)) = self.dialog.take() else {
                    return Task::none();
                };
                self.status = match changes.len() {
                    1 => "Updated the tag notes of 1 note".into(),
                    count => format!("Updated the tag notes of {count} notes"),
                };
                for change in changes {
                    self.update_tag_notes(&change.note, &change.add, &change.remove);
                }
                Task::none()
            }
            Message::ShowTypes => match self.active_tab() {
                Some(tab) => self.update(Message::EditType(tab.folder.clone())),
                None => self.update(Message::EditType(self.root.clone())),
            },
            Message::EditType(folder) => {
                let mut folders = Vec::new();
                collect_folders(&self.tree, &mut folders);
                let mut editor = types_editor::Editor::new(&self.root, &folders, &self.own_types);
                editor.set_saved(templates::saved_types());
                editor.set_tables(table_picker::choices(self.table_refs()));
                editor.select(&folder);
                self.dialog = Some(Dialog::Types(editor));
                Task::none()
            }
            Message::ShowWalkthroughEditor => {
                let mut folders = Vec::new();
                collect_folders(&self.tree, &mut folders);
                let folders = folders
                    .iter()
                    .map(|folder| {
                        let name = folder
                            .strip_prefix(&self.root)
                            .unwrap_or(folder)
                            .to_string_lossy()
                            .replace('\\', "/");
                        let ty = self.types.get(folder).and_then(|ty| ty.as_ref().ok());
                        (name, ty.cloned())
                    })
                    .collect();
                let tables = self.table_refs();
                self.dialog = Some(Dialog::Walkthrough(walkthrough_editor::Editor::new(
                    &self.root, folders, tables,
                )));
                Task::none()
            }
            Message::WalkthroughEditor(walkthrough_editor::Message::Cancel) => {
                self.dialog = None;
                Task::none()
            }
            Message::WalkthroughEditor(
                message @ (walkthrough_editor::Message::Save | walkthrough_editor::Message::TryIt),
            ) => {
                let Some(Dialog::Walkthrough(editor)) = &mut self.dialog else {
                    return Task::none();
                };
                if editor.has_changes() {
                    if let Err(error) = editor.save() {
                        editor.error = Some(format!("Could not save them: {error}"));
                        return Task::none();
                    }
                    self.status = "Saved the walkthrough questions".into();
                }
                self.dialog = None;
                self.rescan();
                // An open walkthrough asks the new questions.
                let shown = self.walkthrough_shown();
                self.close_walkthrough();
                if shown || matches!(message, walkthrough_editor::Message::TryIt) {
                    self.show_walkthrough();
                    return self.focus_walkthrough();
                }
                Task::none()
            }
            Message::WalkthroughEditor(message) => {
                let scroll = matches!(
                    message,
                    walkthrough_editor::Message::AddQuestion | walkthrough_editor::Message::AddNote
                );
                if let Some(Dialog::Walkthrough(editor)) = &mut self.dialog {
                    editor.update(message);
                }
                if scroll {
                    return iced::widget::operation::snap_to(
                        walkthrough_editor::DETAIL,
                        iced::widget::scrollable::RelativeOffset::END,
                    );
                }
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
            Message::Types(types_editor::Message::ExportType) => {
                let Some(Dialog::Types(editor)) = &mut self.dialog else {
                    return Task::none();
                };
                match editor.export_type() {
                    Ok((name, yaml)) => Task::perform(
                        save_export(templates::types_folder(), name, yaml.into_bytes()),
                        Message::Exported,
                    ),
                    Err(message) => {
                        editor.error = Some(message);
                        Task::none()
                    }
                }
            }
            Message::Types(types_editor::Message::ExportStructure) | Message::ExportStructure => {
                match templates::Structure::read(&self.root).to_zip() {
                    Ok(bytes) => Task::perform(
                        save_export(
                            templates::templates_folder(),
                            format!("{}.zip", self.name()),
                            bytes,
                        ),
                        Message::Exported,
                    ),
                    Err(message) => {
                        let message = format!("Could not export it: {message}");
                        match &mut self.dialog {
                            Some(Dialog::Types(editor)) => editor.error = Some(message),
                            _ => self.status = message,
                        }
                        Task::none()
                    }
                }
            }
            Message::Types(types_editor::Message::Import) => {
                Task::perform(pick_import(), Message::TypesImportPicked)
            }
            Message::Exported(result) => {
                // From Campaign > Export Campaign Structure, with no dialog open.
                let Some(Dialog::Types(editor)) = &mut self.dialog else {
                    match result {
                        Ok(None) => {}
                        Ok(Some(path)) => self.status = exported_notice(&path),
                        Err(message) => self.status = format!("Could not export it: {message}"),
                    }
                    return Task::none();
                };
                match result {
                    Ok(None) => {}
                    Ok(Some(path)) => {
                        editor.set_saved(templates::saved_types());
                        editor.notice = Some(exported_notice(&path));
                    }
                    Err(message) => editor.error = Some(format!("Could not export it: {message}")),
                }
                Task::none()
            }
            Message::TypesImportPicked(file) => {
                let (Some(Dialog::Types(editor)), Some(file)) = (&mut self.dialog, file) else {
                    return Task::none();
                };
                match templates::import(&file) {
                    Ok(templates::Imported::Type(label)) => {
                        editor.set_saved(templates::saved_types());
                        editor.notice = Some(format!(
                            "Saved the type {label}. Folders with no type can start from it, here or in New Folder, in any campaign."
                        ));
                    }
                    Ok(templates::Imported::Template(name, skipped)) => {
                        let skipped = match skipped {
                            0 => String::new(),
                            1 => {
                                " One file in it that isn't a folder or a type was left out.".into()
                            }
                            n => format!(
                                " {n} files in it that aren't folders or types were left out."
                            ),
                        };
                        editor.notice = Some(format!(
                            "Added the template {name}. New campaigns can start from it.{skipped}"
                        ));
                    }
                    Err(message) => editor.error = Some(format!("Could not import it: {message}")),
                }
                Task::none()
            }
            Message::Types(message) => {
                let added = matches!(message, types_editor::Message::AddNew);
                if let Some(Dialog::Types(editor)) = &mut self.dialog {
                    editor.update(message);
                }
                // A new type is at the end of the list, and is named first.
                if added {
                    return Task::batch([
                        iced::widget::operation::snap_to(
                            types_editor::LIST,
                            iced::widget::scrollable::RelativeOffset::END,
                        ),
                        iced::widget::operation::focus(types_editor::NAME_INPUT),
                    ]);
                }
                Task::none()
            }
            Message::Form(form::Message::FollowLink(index)) => self.follow_link_field(index),
            Message::Form(form::Message::FollowItem(index, item)) => {
                self.follow_list_item(index, item)
            }
            Message::Form(form::Message::FollowAll(index)) => self.follow_all(index),
            Message::Form(form::Message::RollStep(index, step)) => {
                self.roll_step(index, step);
                Task::none()
            }
            Message::Form(form::Message::FollowStep(index, step)) => {
                let Some((field, target)) = self
                    .active_tab()
                    .and_then(|tab| tab.form.as_ref())
                    .and_then(|form| form.step_link(index, step))
                else {
                    return Task::none();
                };
                let folder = self.link_folder(field);
                self.follow_link_into(&target, folder)
            }
            Message::TabField(back) => {
                if self.dialog.is_some() || self.context.is_some() {
                    return Task::none();
                }
                // In a link field or step, Tab finishes the link being typed, as in text.
                if let Some((link, _)) = self.link_suggestions().notes.first()
                    && self
                        .active_tab()
                        .and_then(|tab| tab.form.as_ref())
                        .and_then(form::Form::focused_linking_field)
                        .is_some()
                {
                    let link = link.clone();
                    return self.update(Message::CompleteLink(link));
                }
                // A link being typed in a roll table's result is finished by Tab too.
                let in_table = self.active_tab().is_some_and(|tab| match &tab.edit {
                    Some(ReadEdit::Table { editor, .. }) => editor.link_query().is_some(),
                    _ => tab
                        .form
                        .as_ref()
                        .is_some_and(|form| form.typing_in_table() && form.link_query().is_some()),
                });
                if in_table
                    && !back
                    && let Some((link, _)) = self.link_suggestions().notes.first()
                {
                    let link = link.clone();
                    return self.update(Message::CompleteLink(link));
                }
                // In the table editor, Tab goes through its boxes.
                if let Some(ReadEdit::Table { editor, .. }) =
                    self.active_tab().and_then(|tab| tab.edit.as_ref())
                {
                    return iced::advanced::widget::operate(form::tab(editor.tab_ids(), back))
                        .discard();
                }
                let Some(form) = self
                    .active_tab()
                    .filter(|tab| tab.shows_form() && tab.edit.is_none() && !tab.reading())
                    .and_then(|tab| tab.form.as_ref())
                else {
                    return Task::none();
                };
                iced::advanced::widget::operate(form::tab(form.tab_ids(), back))
                    .map(Message::FieldTabbed)
            }
            Message::FieldTabbed(position) => {
                if let Some(form) = self.active_tab_mut().and_then(|tab| tab.form.as_mut()) {
                    form.tabbed_to(position);
                }
                Task::none()
            }
            Message::Form(form::Message::RollTable(index, table)) => {
                self.roll_into_field(index, &table);
                Task::none()
            }
            Message::Form(form::Message::PickTable(index, table)) => {
                match self
                    .active_tab()
                    .and_then(|tab| tab.form.as_ref())
                    .and_then(|form| form.pick_key_of(index))
                {
                    Some(key) => self.update(Message::PickTable(key, table)),
                    None => Task::none(),
                }
            }
            Message::PickTable(key, table) => {
                self.table_settings.picked.insert(key, table);
                if let Err(error) = self.table_settings.save(&self.root) {
                    self.status = format!("Couldn't remember the table picked: {error}");
                }
                Task::none()
            }
            Message::RollIntoField(key, table) => {
                let index = self
                    .active_tab()
                    .and_then(|tab| tab.form.as_ref())
                    .and_then(|form| form.short_field(&key))
                    .map(|(index, _, _)| index);
                if let Some(index) = index {
                    self.roll_into_field(index, &table);
                }
                Task::none()
            }
            Message::FillField(add_line) => {
                if let Some(Dialog::ReplaceField { index, value, .. }) = self.dialog.take() {
                    self.fill_field(index, &value, add_line);
                }
                Task::none()
            }
            Message::Form(form::Message::Roll(index, expression)) => {
                let label = self
                    .active_tab()
                    .and_then(|tab| tab.form.as_ref())
                    .and_then(|form| form.label(index))
                    .unwrap_or_default();
                self.dice.open = true;
                self.dice.roll_field(&label, &expression);
                Task::none()
            }
            Message::Form(form::Message::Table(table_editor::Message::Roll)) => {
                self.roll_form_table();
                Task::none()
            }
            Message::Form(form::Message::FieldTable(index, table_editor::Message::Roll)) => {
                self.roll_field_table(index);
                Task::none()
            }
            Message::Form(message) => {
                // Picking a choice in the reading view finishes editing that field.
                let picked = matches!(message, form::Message::Choice(..));
                if picked
                    && let Some(tab) = self.active_tab_mut()
                    && matches!(tab.edit, Some(ReadEdit::Field(_)))
                {
                    tab.edit = None;
                }
                let click = matches!(
                    message,
                    form::Message::Long(_, text_editor::Action::Click(_))
                );
                let moves_focus = message.moves_focus();
                let opens_table = matches!(message, form::Message::EditTable(..));
                let field_table = match &message {
                    form::Message::FieldTable(index, _) => Some(*index),
                    _ => None,
                };
                let ctrl = self.ctrl;
                if let Some(tab) = self.active_tab_mut()
                    && let Some(form) = &mut tab.form
                {
                    tab.modified |= form.update(message);
                    let finished = form.take_finished_table();
                    if finished.is_some() || opens_table {
                        self.form_table_settings(finished, opens_table);
                    }
                    if let Some(index) = field_table {
                        self.save_field_table_settings(index);
                    }
                    let Some(tab) = self.active_tab_mut() else {
                        return Task::none();
                    };
                    let Some(form) = &mut tab.form else {
                        return Task::none();
                    };
                    if click
                        && ctrl
                        && let Some(target) = form.link_under_cursor()
                    {
                        return self.follow_link(&target);
                    }
                    if let Some(id) = form.take_table_focus() {
                        return iced::advanced::widget::operate(form::tab(vec![id], false))
                            .discard();
                    }
                    if moves_focus && let Some(id) = form.focused_id() {
                        // Focus the step, scrolling to it if it moved out of sight.
                        return iced::advanced::widget::operate(form::tab(vec![id], false))
                            .discard();
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
                    && let Err(message) = self.move_note(&path, &folder)
                {
                    self.status = message;
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
                let reading = tab.reading();
                match &mut tab.edit {
                    Some(ReadEdit::Block { content, .. }) => {
                        links::complete(content, &link);
                        tab.modified = true;
                        return iced::widget::operation::focus(BLOCK_EDITOR);
                    }
                    Some(ReadEdit::Field(_)) => {
                        if let Some(form) = &mut tab.form {
                            tab.modified |= form.complete_link(&link);
                        }
                        return iced::widget::operation::focus(FIELD_EDITOR);
                    }
                    Some(ReadEdit::Table { editor, .. }) => {
                        return match editor.complete_link(&link) {
                            Some(id) => iced::widget::operation::focus(id),
                            None => Task::none(),
                        };
                    }
                    None if reading => return Task::none(),
                    None => {}
                }
                if tab.shows_form() {
                    let Some(form) = &mut tab.form else {
                        return Task::none();
                    };
                    let focus = form.focused_id();
                    tab.modified |= form.complete_link(&link);
                    match focus {
                        Some(id) => iced::widget::operation::focus(id),
                        None => Task::none(),
                    }
                } else {
                    links::complete(&mut tab.content, &link);
                    tab.modified = true;
                    iced::widget::operation::focus(EDITOR)
                }
            }
            Message::FollowLink(target) => self.follow_link(&target),
            Message::InsertPicture => {
                if !self.active_tab().is_some_and(Tab::is_note) {
                    return Task::none();
                }
                Task::perform(pick_picture(), Message::AddPicture)
            }
            Message::AddPicture(Some(file)) => {
                // A Markdown file dropped on a walkthrough question that takes files is
                // added there, like a player's character.
                if campaign::is_markdown(&file) && self.walkthrough_shown() {
                    let campaign = walkthrough::Campaign {
                        root: &self.root,
                        types: &self.types,
                        index: &self.index,
                    };
                    if self
                        .walkthrough
                        .as_ref()
                        .and_then(|walkthrough| walkthrough.import_into(&campaign))
                        .is_some()
                    {
                        return self
                            .walkthrough_message(walkthrough_tab::Message::Imported(vec![file]));
                    }
                }
                self.add_picture(&file);
                Task::none()
            }
            Message::WebPicture(url, picture) => {
                self.web.insert(url, picture);
                Task::none()
            }
            Message::InsertLink => self.change_focused_text(
                links::insert,
                "Click in a text box first, then press Ctrl+K to add a link there",
            ),
            Message::Format(command) => self.change_focused_text(
                |content| format::apply(content, command),
                "Click in a text box first, then pick how to format it",
            ),
            Message::ToggleReading => {
                let reading = self.active_tab().is_some_and(Tab::reading);
                self.update(Message::SetReading(!reading))
            }
            Message::SetReading(true) => {
                if self.active_tab().is_some_and(Tab::is_note) {
                    self.read_active_tab();
                }
                Task::none()
            }
            Message::SetReading(false) => {
                if let Some(tab) = self.active_tab_mut() {
                    tab.preview = None;
                }
                Task::none()
            }
            Message::EditBlock(index) => {
                // A roll table is edited in the table editor, not as Markdown.
                if self
                    .active_tab()
                    .is_some_and(|tab| tab.is_table_block(index))
                {
                    return self.edit_table(index);
                }
                let editing = match self.active_tab().and_then(|tab| tab.edit.as_ref()) {
                    Some(ReadEdit::Block { index, .. } | ReadEdit::Table { index, .. }) => {
                        Some(*index)
                    }
                    _ => None,
                };
                let added = self.finish_editing();
                // Blocks after the one just edited moved if it became more or fewer blocks.
                let index = match editing {
                    Some(edited) if index > edited => index.saturating_add_signed(added),
                    _ => index,
                };
                if let Some(tab) = self.active_tab_mut()
                    && let Some(preview) = &tab.preview
                    && let Some(range) = preview.block_range(index)
                {
                    let content = text_editor::Content::with_text(&preview.source()[range.clone()]);
                    tab.edit = Some(ReadEdit::Block {
                        index,
                        range,
                        content,
                    });
                    return iced::widget::operation::focus(BLOCK_EDITOR);
                }
                Task::none()
            }
            Message::HoverBlock(index) => {
                self.hovered_block = Some(index);
                Task::none()
            }
            Message::AddTableHere => self.add_table_here(),
            Message::AddTableAtEnd => {
                self.menu_block = None;
                self.add_table_here()
            }
            Message::AddText => {
                if let Some(tab) = self.active_tab_mut()
                    && let Some(preview) = &tab.preview
                {
                    let end = preview.source().len();
                    tab.edit = Some(ReadEdit::Block {
                        index: preview.block_count(),
                        range: end..end,
                        content: text_editor::Content::new(),
                    });
                    return iced::widget::operation::focus(BLOCK_EDITOR);
                }
                Task::none()
            }
            Message::BlockEdit(action) => {
                if let Some(tab) = self.active_tab_mut()
                    && let Some(ReadEdit::Block { content, .. }) = &mut tab.edit
                {
                    tab.modified |= action.is_edit();
                    links::perform(content, action);
                }
                Task::none()
            }
            Message::EditField(key) => {
                let Some(tab) = self.active_tab_mut() else {
                    return Task::none();
                };
                let Some(form) = &mut tab.form else {
                    return Task::none();
                };
                match form.short_field(&key) {
                    // A checkbox just flips.
                    Some((index, _, form::FieldValue::Check(checked))) => {
                        tab.modified |= form.update(form::Message::Check(index, !checked));
                        Task::none()
                    }
                    // A dice field's first box is how many dice; a list's and a date's
                    // is the one to type in.
                    Some((
                        index,
                        _,
                        form::FieldValue::Dice | form::FieldValue::List | form::FieldValue::Date,
                    )) => {
                        tab.edit = Some(ReadEdit::Field(key));
                        iced::widget::operation::focus(form::field_id(index))
                    }
                    Some(_) => {
                        tab.edit = Some(ReadEdit::Field(key));
                        iced::widget::operation::focus(FIELD_EDITOR)
                    }
                    None => Task::none(),
                }
            }
            Message::FinishEditing => Task::none(),
            Message::PreviewLink(url) => {
                let Some(tab) = self.active_tab() else {
                    return Task::none();
                };
                match preview::click(&url, &tab.note_path()) {
                    // A note followed from the reading view is read too, unless it's new.
                    Some(preview::Click::Note(target)) => {
                        let exists = self.index.resolve(&target, &tab.note_path()).is_some();
                        let task = self.follow_link(&target);
                        if exists {
                            self.read_active_tab();
                        }
                        task
                    }
                    Some(preview::Click::File(path)) => {
                        self.open_file(path);
                        if self.active_tab().is_some_and(Tab::is_note) {
                            self.read_active_tab();
                        }
                        Task::none()
                    }
                    Some(preview::Click::Web(url)) => {
                        self.status = match open_in_browser(&url) {
                            Ok(()) => format!("Opened {url} in your browser"),
                            Err(error) => format!("Could not open {url}: {error}"),
                        };
                        Task::none()
                    }
                    None => Task::none(),
                }
            }
            Message::ToggleTask(at) => {
                let root = self.root.clone();
                let index = self.index.clone();
                if let Some(tab) = self.active_tab_mut()
                    && let Some(preview) = &tab.preview
                {
                    // Ticking a box doesn't move anything in the note, so a block being
                    // edited is still where it was, and goes back in after.
                    let text = tab.text().unwrap_or_else(|_| tab.content.text());
                    if text == preview.source()
                        && let Some(new) = preview::toggle_task(&text, at)
                    {
                        tab.set_text(&new);
                        tab.modified = true;
                        tab.preview = Some(tab.make_preview(&root, &index));
                    }
                }
                self.finish_editing();
                Task::none()
            }
            Message::ToggleSource => {
                // From the reading view, go to the Markdown source.
                let source = self
                    .active_tab()
                    .is_some_and(|tab| tab.reading() || !tab.source);
                self.update(Message::SetSource(source))
            }
            Message::SetSource(source) => {
                let Some(tab) = self.tabs.iter_mut().find(|tab| Some(tab.id) == self.active) else {
                    return Task::none();
                };
                let Some(form) = &mut tab.form else {
                    return Task::none();
                };

                tab.preview = None;
                if tab.source == source {
                    return Task::none();
                }
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
                if self.walkthrough_shown() {
                    // Its notes may have changed in their tabs.
                    self.refresh_walkthrough();
                }
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
                self.pick_up_outside_changes();
                Task::none()
            }
            Message::FilesChanged => {
                self.pick_up_outside_changes();
                Task::none()
            }
            Message::FixProblems(path) => {
                self.open_file(path.clone());
                if self.active_tab().and_then(|tab| tab.path.as_ref()) == Some(&path) {
                    self.dialog = Some(Dialog::Fix(path));
                }
                Task::none()
            }
            Message::Fix(fix) => self.fix(fix),
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
            Message::CloseTab(id) if self.walkthrough.as_ref().is_some_and(|w| w.id == id) => {
                self.close_walkthrough();
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
            Message::SaveAll => self.save_all(),
            Message::Saved(id, Ok(Some(path))) => {
                self.status = format!("Saved {}", self.relative(&path));
                if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                    tab.folder = path.parent().unwrap_or(&self.root).to_path_buf();
                    tab.disk = check::modified_time(&path);
                    tab.path = Some(path.clone());
                    tab.modified = false;
                    tab.changed_outside = false;
                }
                self.rescan();
                if let Some((fields, before)) = self.tag_saves.remove(&id)
                    && let Ok(after) = std::fs::read_to_string(&path)
                {
                    self.sync_tags(&path, &fields, &before, &after);
                }
                self.refresh_embeds(id);
                self.saved_one(true);
                Task::none()
            }
            // The save dialog was cancelled.
            Message::Saved(id, Ok(None)) => {
                self.tag_saves.remove(&id);
                self.saved_one(false);
                Task::none()
            }
            Message::Saved(id, Err(error)) => {
                self.tag_saves.remove(&id);
                self.not_saved(id, format!("Could not save: {error}"));
                Task::none()
            }
            Message::OpenContextMenu(target, position) => {
                self.menu_block = self.hovered_block;
                self.context = Some((target, position));
                Task::none()
            }
            Message::Copy | Message::Cut => {
                let cut = matches!(message, Message::Cut);
                let Some((content, id)) = self.focused_text_mut() else {
                    return Task::none();
                };
                let Some(selected) = content.selection() else {
                    return Task::none();
                };
                if cut {
                    content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                    if let Some(tab) = self.active_tab_mut() {
                        tab.modified = true;
                    }
                }
                Task::batch([
                    iced::clipboard::write(selected),
                    iced::widget::operation::focus(id),
                ])
            }
            Message::Paste => iced::clipboard::read().map(Message::Pasted),
            Message::Pasted(Some(pasted)) => {
                let Some((content, id)) = self.focused_text_mut() else {
                    return Task::none();
                };
                content.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                    std::sync::Arc::new(pasted),
                )));
                if let Some(tab) = self.active_tab_mut() {
                    tab.modified = true;
                }
                iced::widget::operation::focus(id)
            }
            Message::SelectAll => {
                let Some((content, id)) = self.focused_text_mut() else {
                    return Task::none();
                };
                content.perform(text_editor::Action::SelectAll);
                iced::widget::operation::focus(id)
            }
            Message::OpenRead(path) => {
                self.open_file(path);
                if self.active_tab().is_some_and(Tab::is_note) {
                    self.read_active_tab();
                }
                Task::none()
            }
            Message::ShowRename(path) => {
                let name = if path.is_dir() {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                } else {
                    self.tree_name(&path, false)
                };
                self.dialog = Some(Dialog::Rename {
                    path,
                    name,
                    error: None,
                });
                iced::widget::operation::focus(DIALOG_INPUT)
            }
            Message::RenameName(value) => {
                if let Some(Dialog::Rename { name, error, .. }) = &mut self.dialog {
                    *name = value;
                    *error = None;
                }
                Task::none()
            }
            Message::Rename => {
                let Some(Dialog::Rename { path, name, .. }) = &self.dialog else {
                    return Task::none();
                };
                let (path, name) = (path.clone(), name.trim().to_string());
                let result = if path.is_dir() {
                    campaign::validate_name(&name).and_then(|()| {
                        let parent = path.parent().unwrap_or(&self.root).to_path_buf();
                        self.move_folder(&path, &parent.join(&name))
                    })
                } else {
                    self.rename_note(&path, &name)
                };
                self.dialog_result(result);
                Task::none()
            }
            Message::ShowMove(path) => {
                self.dialog = Some(Dialog::Move {
                    path,
                    folder: None,
                    error: None,
                });
                Task::none()
            }
            Message::MoveFolder(choice) => {
                if let Some(Dialog::Move { folder, error, .. }) = &mut self.dialog {
                    *folder = Some(choice);
                    *error = None;
                }
                Task::none()
            }
            Message::Move => {
                let Some(Dialog::Move { path, folder, .. }) = &self.dialog else {
                    return Task::none();
                };
                let (path, folder) = (path.clone(), folder.clone());
                let result = match folder {
                    None => Err("Pick the folder to move it to".to_string()),
                    Some(folder) if path.is_dir() => match path.file_name() {
                        Some(name) => self.move_folder(&path, &folder.path.join(name)),
                        None => Ok(()),
                    },
                    Some(folder) => self.move_note(&path, &folder.path),
                };
                self.dialog_result(result);
                Task::none()
            }
            Message::Delete(path) => {
                let name = self.shown_name(&path);
                let unsaved = self.tabs.iter().any(|tab| {
                    tab.modified
                        && tab
                            .path
                            .as_ref()
                            .is_some_and(|open| open.starts_with(&path))
                });
                let mut question = if path.is_dir() {
                    let notes = markdown_files(&self.tree)
                        .iter()
                        .filter(|note| note.starts_with(&path))
                        .count();
                    match notes {
                        0 => format!("Delete the folder {name}?"),
                        1 => format!("Delete the folder {name} and the note in it?"),
                        notes => format!("Delete the folder {name} and the {notes} notes in it?"),
                    }
                } else {
                    format!("Delete {name}?")
                };
                question.push_str(&format!(
                    " It goes to the {TRASH}, so you can get it back from there."
                ));
                if unsaved {
                    question.push_str(" Unsaved changes to it are lost.");
                }
                if campaign::is_markdown(&path) || path.is_dir() {
                    question.push_str(" Links to it will show as missing.");
                }
                Task::perform(confirm_delete(question), move |yes| {
                    Message::DeleteConfirmed(path.clone(), yes)
                })
            }
            Message::DeleteConfirmed(path, true) => {
                let name = self.relative(&path);
                match trash::delete(&path) {
                    Ok(()) => {
                        let closed: Vec<u64> = self
                            .tabs
                            .iter()
                            .filter(|tab| {
                                tab.path
                                    .as_ref()
                                    .is_some_and(|open| open.starts_with(&path))
                            })
                            .map(|tab| tab.id)
                            .collect();
                        for id in closed {
                            self.remove_tab(id);
                        }
                        self.expanded.retain(|folder| !folder.starts_with(&path));
                        self.rescan();
                        self.status = format!("Moved {name} to the {TRASH}");
                    }
                    Err(error) => self.status = format!("Could not delete {name}: {error}"),
                }
                Task::none()
            }
            Message::CopyLink(path) => {
                let from = self
                    .active_tab()
                    .map(Tab::note_path)
                    .unwrap_or_else(|| self.root.join("Untitled.md"));
                let link = self
                    .index
                    .link_text(&path, &from, false)
                    .unwrap_or_else(|| display_name(&path, false));
                let link = format!("[[{link}]]");
                self.status = format!("Copied {link}: paste it into a note to link there");
                iced::clipboard::write(link)
            }
            Message::ShowInFileManager(path) => {
                if let Err(error) = show_in_file_manager(&path) {
                    self.status = format!("Could not show {}: {error}", self.relative(&path));
                }
                Task::none()
            }
            Message::RevealInTree(path) => {
                for folder in path.ancestors().skip(1) {
                    if folder == self.root || !folder.starts_with(&self.root) {
                        break;
                    }
                    self.expanded.insert(folder.to_path_buf());
                }
                Task::none()
            }
            Message::SaveTab(id) => self.save(id, false),
            Message::CloseTabs(ids) => {
                if self
                    .tabs
                    .iter()
                    .any(|tab| tab.modified && ids.contains(&tab.id))
                {
                    Task::perform(
                        confirm_discard(
                            "Some of these files have unsaved changes. Close them anyway?",
                        ),
                        move |yes| Message::CloseTabsConfirmed(ids.clone(), yes),
                    )
                } else {
                    self.update(Message::CloseTabsConfirmed(ids, true))
                }
            }
            Message::CloseTabsConfirmed(ids, true) => {
                for id in ids {
                    self.remove_tab(id);
                }
                Task::none()
            }
            // Dialogs the user cancelled, and splash screen messages.
            _ => Task::none(),
        }
    }

    /// The items of the right-click menu for `target`.
    fn context_entries(&self, target: &Target) -> Vec<menu::Entry<Message>> {
        use menu::{Entry, item};

        let when = |enabled: bool, message: Message| enabled.then_some(message);
        let active = self.active_tab();
        // Copying a link and finding the file in the tree, for the note in the active tab.
        let note_items = |entries: &mut Vec<Entry<Message>>| {
            let path = active.and_then(|tab| tab.path.clone());
            entries.extend([
                Entry::Separator,
                item(
                    "Copy Link to This Note",
                    path.clone()
                        .filter(|path| campaign::is_markdown(path))
                        .map(Message::CopyLink),
                ),
                item("Show in File Tree", path.map(Message::RevealInTree)),
            ]);
        };

        match target {
            Target::Tree(path) if *path == self.root => vec![
                item("New Note", Some(Message::NewNoteIn(path.clone()))),
                item(
                    "New Folder...",
                    Some(Message::ShowNewFolderIn(path.clone())),
                ),
                Entry::Separator,
                item("Refresh File Tree", Some(Message::RefreshTree)),
                item("Expand All", Some(Message::ExpandAll)),
                item("Collapse All", Some(Message::CollapseAll)),
                item(
                    SHOW_IN_FILE_MANAGER,
                    Some(Message::ShowInFileManager(path.clone())),
                ),
            ],
            Target::Tree(path) if path.is_dir() => {
                let expanded = self.expanded.contains(path);
                let typed = self.types.contains_key(path);
                let mut entries = vec![item(
                    self.new_note_label(path),
                    Some(Message::NewNoteIn(path.clone())),
                )];
                // A note of each kind of this folder's type that has a subfolder of its own.
                entries.extend(self.subtypes(path).into_iter().map(|(folder, label)| {
                    item(format!("New {label}"), Some(Message::NewNoteIn(folder)))
                }));
                entries.extend([
                    item(
                        "New Subfolder...",
                        Some(Message::ShowNewFolderIn(path.clone())),
                    ),
                    item(
                        "New Roll Table...",
                        Some(Message::ShowNewTable(Some(path.clone()))),
                    ),
                    Entry::Separator,
                    item("Search in Folder", Some(Message::SearchIn(path.clone()))),
                ]);
                if matches!(self.types.get(path), Some(Ok(ty)) if ty.is_a(quests::QUEST)) {
                    entries.push(item("Show Quests", Some(Message::ShowQuests)));
                }
                entries.extend([
                    item(
                        if expanded { "Collapse" } else { "Expand" },
                        Some(Message::ToggleFolder(path.clone())),
                    ),
                    item(
                        if typed {
                            "Edit Note Fields..."
                        } else {
                            "Give Notes Fields..."
                        },
                        Some(Message::EditType(path.clone())),
                    ),
                    Entry::Separator,
                    item("Rename...", Some(Message::ShowRename(path.clone()))),
                    item("Move To...", Some(Message::ShowMove(path.clone()))),
                    item(
                        SHOW_IN_FILE_MANAGER,
                        Some(Message::ShowInFileManager(path.clone())),
                    ),
                    Entry::Separator,
                    item("Delete...", Some(Message::Delete(path.clone()))),
                ]);
                entries
            }
            Target::Tree(path) => {
                let note = campaign::is_markdown(path);
                vec![
                    item("Open", Some(Message::OpenEntry(path.clone()))),
                    item(
                        "Open in Reading View",
                        when(note, Message::OpenRead(path.clone())),
                    ),
                    Entry::Separator,
                    item("Copy Link", when(note, Message::CopyLink(path.clone()))),
                    item("Rename...", when(note, Message::ShowRename(path.clone()))),
                    item("Move To...", when(note, Message::ShowMove(path.clone()))),
                    item(
                        SHOW_IN_FILE_MANAGER,
                        Some(Message::ShowInFileManager(path.clone())),
                    ),
                    Entry::Separator,
                    item("Delete...", Some(Message::Delete(path.clone()))),
                ]
            }
            Target::Tab(id) => {
                let Some(tab) = self.tabs.iter().find(|tab| tab.id == *id) else {
                    return Vec::new();
                };
                let is_active = self.active == Some(*id);
                let shortcut = |entry: Entry<Message>, keys| {
                    if is_active {
                        entry.shortcut(keys)
                    } else {
                        entry
                    }
                };
                let others: Vec<u64> = self
                    .tabs
                    .iter()
                    .map(|tab| tab.id)
                    .filter(|other| other != id)
                    .collect();
                let right: Vec<u64> = self
                    .tabs
                    .iter()
                    .skip_while(|tab| tab.id != *id)
                    .skip(1)
                    .map(|tab| tab.id)
                    .collect();
                let path = tab.path.clone();
                let note = path.as_deref().filter(|path| campaign::is_markdown(path));

                vec![
                    shortcut(item("Save", Some(Message::SaveTab(*id))), "Ctrl+S"),
                    Entry::Separator,
                    shortcut(item("Close", Some(Message::CloseTab(*id))), "Ctrl+W"),
                    item(
                        "Close Others",
                        when(!others.is_empty(), Message::CloseTabs(others)),
                    ),
                    item(
                        "Close Tabs to the Right",
                        when(!right.is_empty(), Message::CloseTabs(right)),
                    ),
                    item(
                        "Close All",
                        Some(Message::CloseTabs(
                            self.tabs.iter().map(|tab| tab.id).collect(),
                        )),
                    ),
                    Entry::Separator,
                    item(
                        "Copy Link",
                        note.map(|path| Message::CopyLink(path.to_path_buf())),
                    ),
                    item(
                        "Rename...",
                        note.map(|path| Message::ShowRename(path.to_path_buf())),
                    ),
                    item("Show in File Tree", path.clone().map(Message::RevealInTree)),
                    item(SHOW_IN_FILE_MANAGER, path.map(Message::ShowInFileManager)),
                ]
            }
            Target::Editor => {
                let content = self.focused_text();
                let selected = content.is_some_and(|content| content.selection().is_some());
                let is_note = active.is_some_and(Tab::is_note);
                let mut entries = vec![
                    item("Cut", when(selected, Message::Cut)).shortcut("Ctrl+X"),
                    item("Copy", when(selected, Message::Copy)).shortcut("Ctrl+C"),
                    item("Paste", when(content.is_some(), Message::Paste)).shortcut("Ctrl+V"),
                    item("Select All", when(content.is_some(), Message::SelectAll))
                        .shortcut("Ctrl+A"),
                    Entry::Separator,
                ];
                // The link that was right-clicked, if any.
                if is_note && let Some(target) = content.and_then(links::under_cursor) {
                    let from = active.map(Tab::note_path).unwrap_or_default();
                    let label = if self.index.resolve(&target, &from).is_some() {
                        format!("Open {target}")
                    } else {
                        format!("Create {target}")
                    };
                    entries.push(item(label, Some(Message::FollowLink(target))));
                }
                entries.extend([
                    item(
                        "Insert Link",
                        when(is_note && content.is_some(), Message::InsertLink),
                    )
                    .shortcut("Ctrl+K"),
                    item("Insert Picture...", when(is_note, Message::InsertPicture))
                        .shortcut("Ctrl+Shift+K"),
                ]);
                // In a typed note's Markdown source, the way back to its form.
                if active.is_some_and(|tab| tab.form.is_some() && tab.source && !tab.reading()) {
                    entries.extend([
                        Entry::Separator,
                        item("Edit as Form", Some(Message::SetSource(false))).shortcut("Ctrl+E"),
                    ]);
                }
                entries
            }
            Target::Form => {
                let mut entries = vec![
                    item("Add Field to This Note...", Some(Message::ShowAddField)),
                    Entry::Separator,
                    item("Edit as Markdown", Some(Message::SetSource(true))).shortcut("Ctrl+E"),
                    item("Reading View", Some(Message::SetReading(true))).shortcut("Ctrl+R"),
                ];
                note_items(&mut entries);
                entries
            }
            Target::Table(tables::Pick::Note(path, start)) => {
                let table = |message| Some(Message::Tables(message));
                vec![
                    item("Roll", table(tables::Message::Roll(path.clone(), *start))),
                    item(
                        "Edit Table",
                        table(tables::Message::Edit(path.clone(), *start)),
                    ),
                    item("Open Note", table(tables::Message::Open(path.clone()))),
                    Entry::Separator,
                    item(
                        "Show in File Tree",
                        Some(Message::RevealInTree(path.clone())),
                    ),
                ]
            }
            Target::Table(tables::Pick::BuiltIn(name)) => {
                let table = |message| Some(Message::Tables(message));
                vec![
                    item("Roll", table(tables::Message::RollBuiltIn(name.clone()))),
                    item(
                        "Copy into Campaign and Edit",
                        table(tables::Message::EditBuiltIn(name.clone())),
                    ),
                    item(
                        "Copy into Campaign",
                        table(tables::Message::CopyBuiltIn(name.clone())),
                    ),
                ]
            }
            Target::Reading => {
                let mut entries = vec![
                    item("Edit This Note", Some(Message::SetReading(false))).shortcut("Ctrl+R"),
                    item("Add Text at the End", Some(Message::AddText)),
                    item("Add Roll Table Here", Some(Message::AddTableHere)),
                    item("Insert Picture...", Some(Message::InsertPicture))
                        .shortcut("Ctrl+Shift+K"),
                ];
                note_items(&mut entries);
                entries
            }
        }
    }

    fn view<'a>(
        &'a self,
        c: Colors,
        settings: &Settings,
        dice: &'a dice::Icons,
    ) -> Element<'a, Message> {
        let pane = match &self.walkthrough {
            Some(walkthrough) if self.walkthrough_shown() => {
                self.walkthrough_view(c, walkthrough, dice)
            }
            _ => self.editor(c, settings.show_toolbar, dice),
        };
        let mut main = column![self.tab_bar(c, settings, dice), pane];
        if self.dice.open {
            main = main
                .push(rule::horizontal(c.border_width.max(1.0)).style(style::separator(c)))
                .push(self.dice.view(c, dice).map(Message::Dice));
        }

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
                    .color(c.status_text)
                    .wrapping(text::Wrapping::None),
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
            tree_button(c, "▾", "Expand all folders", Message::ExpandAll),
            tree_button(c, "▸", "Collapse all folders", Message::CollapseAll),
            tree_button(c, "⟳", "Refresh the file tree", Message::RefreshTree),
        ]
        .align_y(Center)
        .padding([6, 12]);

        let tree: Element<'_, Message> = if self.search.active() {
            self.search_results(c)
        } else if self.quests.open {
            let selected = self.active_tab().and_then(|tab| tab.path.as_deref());
            self.quests.view(c, selected).map(Message::Quests)
        } else if self.tables.open {
            let selected = self.active_tab().and_then(|tab| tab.path.as_deref());
            self.tables.view(c, selected).map(Message::Tables)
        } else if self.tree.is_empty() {
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

        let sidebar = container(column![
            root_area(header),
            self.search_box(c),
            stack![
                scrollable(column![tree, root_area(space().height(Fill).into())].padding([0, 4]))
                    .height(Fill),
                corner_button(c, Message::ShowNewFolder),
            ]
            .height(Fill)
        ])
        .width(260)
        .height(Fill)
        .style(style::panel(c.sidebar));

        let root = self.root.clone();
        context_menu::area(sidebar, move |position| {
            Message::OpenContextMenu(Target::Tree(root.clone()), position)
        })
        .into()
    }

    /// The search box at the top of the sidebar, with the folder being searched in.
    fn search_box(&self, c: Colors) -> Element<'_, Message> {
        let input = text_input("Search notes  (status:alive)", &self.search.query)
            .id(SEARCH)
            .on_input(Message::SearchChanged)
            .on_submit(Message::OpenFirstHit)
            .size(13)
            .padding([4, 8])
            .style(style::input(c));
        let clear = |message| {
            button(text("✕").size(11))
                .padding([2, 6])
                .style(style::flat(c, false))
                .on_press(message)
        };
        let mut content = column![
            row![input]
                .push(self.search.active().then(|| clear(Message::ClearSearch)))
                .spacing(2)
                .align_y(Center)
        ]
        .spacing(4);
        if let Some(folder) = &self.search.folder {
            let name = folder
                .strip_prefix(&self.root)
                .unwrap_or(folder)
                .display()
                .to_string();
            content = content.push(
                row![
                    text(format!("In {name}"))
                        .size(12)
                        .color(c.text_muted)
                        .width(Fill),
                    button(text("Search everywhere").size(11).color(c.accent))
                        .padding([0, 4])
                        .style(style::flat(c, false))
                        .on_press(Message::SearchEverywhere),
                ]
                .align_y(Center),
            );
        }
        container(content).padding([4, 8]).into()
    }

    /// The notes the search found, in place of the file tree: each with its folder and
    /// the line that matched.
    fn search_results(&self, c: Colors) -> Element<'_, Message> {
        let hits = &self.search.hits;
        let count = match hits.len() {
            0 => "No notes found".to_string(),
            1 => "1 note".to_string(),
            n => format!("{n} notes"),
        };
        let mut rows = vec![
            container(text(count).size(11).color(c.text_muted))
                .padding([2, 8])
                .into(),
        ];
        let active_path = self.active_tab().and_then(|tab| tab.path.as_ref());
        for hit in hits.iter().take(MAX_HITS) {
            let folder = hit
                .path
                .parent()
                .and_then(|folder| folder.strip_prefix(&self.root).ok())
                .map(|folder| folder.display().to_string())
                .unwrap_or_default();
            let mut lines = column![
                row![
                    text(self.tree_name(&hit.path, false))
                        .size(13)
                        .color(c.text)
                        .wrapping(text::Wrapping::None),
                    text(folder)
                        .size(11)
                        .color(c.text_muted)
                        .wrapping(text::Wrapping::None),
                ]
                .spacing(6)
                .align_y(iced::Alignment::End)
            ]
            .spacing(1);
            if let Some(line) = &hit.line {
                lines = lines.push(text(line).size(11).color(c.text_muted));
            }
            let selected = active_path == Some(&hit.path);
            rows.push(
                button(container(lines).clip(true))
                    .width(Fill)
                    .padding([3, 8])
                    .style(style::flat(c, selected))
                    .on_press(Message::OpenEntry(hit.path.clone()))
                    .into(),
            );
        }
        if hits.len() > MAX_HITS {
            rows.push(
                container(
                    text(format!(
                        "and {} more. Type more to narrow the search.",
                        hits.len() - MAX_HITS
                    ))
                    .size(11)
                    .color(c.text_muted),
                )
                .padding([2, 8])
                .into(),
            );
        }
        column(rows).spacing(1).into()
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
                None => self.tree_name(&entry.path, settings.show_extensions),
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
            // The row a right-click menu is open for shows which one it is.
            let selected = selected
                || matches!(&self.context, Some((Target::Tree(path), _)) if *path == entry.path);

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
            let warning = has_problem.then(|| self.warning_button(c, entry));

            let label = button(
                row![indent, text(arrow).size(13).width(14), text(name).size(13)]
                    .push(warning)
                    .align_y(Center),
            )
            .width(Fill)
            .padding([2, 0])
            .style(style::flat(c, selected))
            .on_press(message);

            let row: Element<'a, Message> = match entry.children {
                Some(_) => row![
                    label,
                    tooltip(
                        button(text("+").size(14))
                            .padding([0, 8])
                            .style(style::flat(c, false))
                            .on_press(Message::NewNoteIn(entry.path.clone())),
                        container(text(self.new_note_label(&entry.path)).size(12))
                            .padding([2, 6])
                            .style(style::dropdown(c)),
                        tooltip::Position::Right,
                    ),
                ]
                .align_y(Center)
                .into(),
                None => label.into(),
            };

            let path = entry.path.clone();
            rows.push(
                context_menu::area(
                    mouse_area(row)
                        .on_enter(Message::TreeEnter(entry.path.clone()))
                        .on_exit(Message::TreeExit(entry.path.clone())),
                    move |position| Message::OpenContextMenu(Target::Tree(path.clone()), position),
                )
                .into(),
            );

            if let Some(children) = &entry.children
                && self.expanded.contains(&entry.path)
            {
                self.tree_rows(c, settings, children, depth + 1, rows);
            }
        }
    }

    fn tab_bar(&self, c: Colors, settings: &Settings, dice: &dice::Icons) -> Element<'_, Message> {
        let tabs = self.tabs.iter().map(|tab| {
            let active = Some(tab.id) == self.active;
            let close_label = if tab.modified { "●" } else { "×" };
            let id = tab.id;

            let tab = container(
                row![
                    button(text(self.tab_name(tab, settings.show_extensions)).size(13))
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
            .style(style::tab(c, active));

            context_menu::area(tab, move |position| {
                Message::OpenContextMenu(Target::Tab(id), position)
            })
            .into()
        });
        // The walkthrough's tab comes first.
        let walkthrough = self
            .walkthrough
            .as_ref()
            .map(|walkthrough| -> Element<'_, Message> {
                let active = self.walkthrough_shown();
                container(
                    row![
                        button(text("Walkthrough").size(13))
                            .padding([8, 4])
                            .style(style::tab_button(c, active))
                            .on_press(Message::SelectTab(walkthrough.id)),
                        button(text("×").size(13))
                            .padding([8, 4])
                            .style(style::tab_button(c, active))
                            .on_press(Message::CloseTab(walkthrough.id)),
                    ]
                    .align_y(Center),
                )
                .padding([0, 6])
                .style(style::tab(c, active))
                .into()
            });
        let tabs = walkthrough.into_iter().chain(tabs);

        // The d20 at the right opens and closes the dice roller.
        let roll = button(Svg::new(dice.roll.clone()).width(28).height(28))
            .padding(3)
            .style(style::flat(c, self.dice.open))
            .on_press(Message::ToggleDice);
        let roll = tooltip(
            roll,
            container(text("Dice roller").size(12))
                .padding([2, 6])
                .style(style::dropdown(c)),
            tooltip::Position::Left,
        );

        let mut bar = row![
            scrollable(row(tabs).spacing(1))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new().width(3).scroller_width(3),
                ))
                .width(Fill),
        ]
        .align_y(Center);

        // These switch a note between its editor and the reading view. A note with a type
        // can be edited as a form or as Markdown, so it has a button for each.
        if let Some(tab) = self.active_tab().filter(|tab| tab.is_note()) {
            let switch = |label, selected: bool, message, tip| {
                tooltip(
                    button(text(label).size(13))
                        .padding([4, 10])
                        .style(style::flat(c, selected))
                        .on_press(message),
                    container(text(tip).size(12))
                        .padding([2, 6])
                        .style(style::dropdown(c)),
                    tooltip::Position::Bottom,
                )
            };
            let reading = tab.reading();
            let mut switches = row![].spacing(2);
            if tab.form.is_some() {
                switches = switches
                    .push(switch(
                        "Form",
                        tab.shows_form(),
                        Message::SetSource(false),
                        "Edit the note's fields in a form (Ctrl+E switches)",
                    ))
                    .push(switch(
                        "Markdown",
                        tab.source && !reading,
                        Message::SetSource(true),
                        "Edit the note as Markdown text (Ctrl+E switches)",
                    ));
            } else {
                switches = switches.push(switch(
                    "Edit",
                    !reading,
                    Message::SetReading(false),
                    "Edit the note (Ctrl+R switches)",
                ));
            }
            switches = switches.push(switch(
                "Read",
                reading,
                Message::SetReading(true),
                "Read the note formatted (Ctrl+R switches)",
            ));
            bar = bar.push(container(switches).padding([0, 6]));
        }

        container(bar.push(container(roll).padding([0, 6])))
            .width(Fill)
            .style(style::panel(c.sidebar))
            .into()
    }

    fn editor<'a>(
        &'a self,
        c: Colors,
        show_toolbar: bool,
        dice: &'a dice::Icons,
    ) -> Element<'a, Message> {
        let suggestions = self.link_suggestions();
        let editor = self.editor_body(c, &suggestions, dice);
        // Like the banner, its place is kept when it is hidden.
        let toolbar = show_toolbar
            .then(|| self.format_bar(c))
            .flatten()
            .unwrap_or_else(|| space().into());

        // The banner's place is kept when there is none, so the editor stays where iced
        // looks for it and keeps the focus when the note changes outside while typing.
        // (Not with `space().height(0)`: columns leave out children with no size.)
        let banner: Element<'_, Message> =
            if self.active_tab().is_some_and(|tab| tab.changed_outside) {
                changed_outside_banner(c)
            } else {
                space().into()
            };
        let mut view = column![toolbar, banner, editor];
        // Below the note, so the text doesn't jump while a problem comes and goes as it is typed.
        if let Some(path) = self.active_tab().and_then(|tab| tab.path.as_ref())
            && self.dialog.is_none()
        {
            let problems: Vec<&Problem> = self.problems_in(path).collect();
            if !problems.is_empty() {
                view = view.push(problems_banner(c, path, &problems));
            }
        }
        if !self.active_tab().is_some_and(Tab::reading) {
            view = view.push(self.link_bar(c, &suggestions));
        }
        view.into()
    }

    /// The notes the link being typed in the active tab could be, as `(link text, label)`.
    ///
    /// The flag is true when the link is typed in a link field rather than a text editor.
    fn link_suggestions(&self) -> Suggestions {
        let none = Suggestions::default();
        let (form, query, from) = match &self.walkthrough {
            // The walkthrough's answer box is a form of its own.
            Some(walkthrough) if self.walkthrough_shown() => {
                let Some(form) = &walkthrough.form else {
                    return none;
                };
                (Some(form), form.link_query(), self.walkthrough_from())
            }
            _ => {
                let Some(tab) = self.active_tab().filter(|tab| tab.is_note()) else {
                    return none;
                };
                // In the reading view, the link is typed in the block or field being edited.
                let form = match &tab.edit {
                    Some(ReadEdit::Field(key)) => tab.form.as_ref().filter(|form| {
                        form.short_field(key).map(|(index, ..)| index) == form.focus()
                    }),
                    _ => tab.form.as_ref().filter(|_| tab.shows_form()),
                };
                let query = match (&tab.edit, form) {
                    (Some(ReadEdit::Block { content, .. }), _) => {
                        links::query(content).map(|query| (query, false))
                    }
                    (Some(ReadEdit::Table { editor, .. }), _) => {
                        editor.link_query().map(|query| (query, false))
                    }
                    (_, Some(form)) => form.link_query(),
                    (Some(ReadEdit::Field(_)), None) => None,
                    (None, None) if tab.reading() => None,
                    (None, None) => links::query(&tab.content).map(|query| (query, false)),
                };
                (form, query, tab.note_path())
            }
        };
        let Some((query, in_field)) = query else {
            return none;
        };

        // A link or steps field lists the notes of the folder it links to first.
        let field = form.and_then(|form| {
            let index = form.focused_link_field()?;
            Some((index, form.link_field(index)?.0))
        });
        let folder = form
            .and_then(form::Form::focused_linking_field)
            .and_then(|field| self.link_folder(field));

        let mut notes = self
            .index
            .suggest(&query, &from, folder.as_deref(), SUGGESTIONS);
        // A result in a roll table can roll the built-in tables too.
        let in_table = !self.walkthrough_shown()
            && self.active_tab().is_some_and(|tab| {
                matches!(tab.edit, Some(ReadEdit::Table { .. }))
                    || form.is_some_and(form::Form::typing_in_table)
            });
        if in_table {
            let typed = query.trim().to_lowercase();
            for built_in in tables::BUILT_IN {
                if notes.len() >= SUGGESTIONS {
                    break;
                }
                let name = built_in.name;
                let taken = notes
                    .iter()
                    .any(|(link, _)| link.eq_ignore_ascii_case(name))
                    || self.index.resolve(name, &from).is_some();
                if !taken && name.to_lowercase().contains(&typed) {
                    notes.push((name.to_string(), format!("{name} (built in)")));
                }
            }
        }
        // A link field that already names a note needs no suggestions.
        if in_field && self.index.resolve(&query, &from).is_some() {
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

        // The walkthrough makes the note without leaving the question, in any folder its
        // answer can link to.
        if self.walkthrough_shown() {
            let make = self
                .walkthrough_pick()
                .into_iter()
                .map(|folder| {
                    let kind = match self.types.get(&folder) {
                        Some(Ok(ty)) => ty.label(),
                        _ => "note".into(),
                    };
                    let name = query.trim().to_string();
                    let label = format!("+ New {kind}: {name}");
                    (folder, name, label)
                })
                .collect();
            return Suggestions {
                notes,
                in_field,
                create: None,
                make,
            };
        }

        Suggestions {
            notes,
            in_field,
            create,
            make: Vec::new(),
        }
    }

    /// The folder link field `field` links to: the one its type names, or else a folder
    /// whose type has the field's name (so a `location` field links to Locations).
    fn link_folder(&self, field: &types::Field) -> Option<PathBuf> {
        types::link_folder(&self.root, &self.types, field)
    }

    /// The colors `[[links]]` are shown in, in text editors.
    fn link_colors(&self, c: Colors) -> links::Colors {
        links::Colors {
            link: c.accent,
            missing: c.text_muted,
            mark: c.text_muted,
            bold: c.accent == c.text,
            names: self.index.names(),
        }
    }

    /// The bar under the editor: notes to link to while a `[[link]]` is being typed, or a
    /// button to open the link the cursor is in.
    fn link_bar<'a>(&self, c: Colors, suggestions: &Suggestions) -> Element<'a, Message> {
        let bar = |content| {
            container(content)
                .padding([6, 16])
                .width(Fill)
                .style(style::panel(c.sidebar))
                .into()
        };

        if let Some(chips) = suggestion_chips(c, suggestions, false) {
            return bar(chips);
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
        .align_y(Center)
        .into())
    }

    /// Opens the note item `item` of list field `index` of the active tab is about: the
    /// note a links field links to (made in the folder it links to if there is none), or
    /// a tag's note.
    fn follow_list_item(&mut self, index: usize, item: usize) -> Task<Message> {
        let Some((field, value)) = self
            .active_tab()
            .and_then(|tab| tab.form.as_ref())
            .and_then(|form| {
                let (field, items) = form.list_items(index)?;
                Some((field.clone(), items.get(item)?.clone()))
            })
        else {
            return Task::none();
        };
        if field.kind == types::Kind::Links {
            let target = value.split(['|', '#']).next().unwrap_or(&value).trim();
            let folder = self.link_folder(&field);
            return self.follow_link_into(target, folder);
        }

        let path = tags::path(&self.root, &field.tag_folder(), &value);
        if path.is_file() {
            self.open_file(path);
        } else {
            self.status = format!(
                "The note for the tag {value} is made in {}/ when this note is saved",
                field.tag_folder()
            );
        }
        Task::none()
    }

    /// Opens every note links field `index` of the active tab links to, each in its tab.
    /// Notes that aren't there yet aren't made.
    fn follow_all(&mut self, index: usize) -> Task<Message> {
        let Some(tab) = self.active_tab() else {
            return Task::none();
        };
        let (id, from) = (tab.id, tab.note_path());
        let Some((_, items)) = tab.form.as_ref().and_then(|form| form.list_items(index)) else {
            return Task::none();
        };
        let mut found = Vec::new();
        let mut missing = Vec::new();
        for item in items {
            let target = item.split(['|', '#']).next().unwrap_or(item).trim();
            match self.index.resolve(target, &from) {
                Some(path) => found.push(path.to_path_buf()),
                None => missing.push(target.to_string()),
            }
        }
        let opened = found.len();
        for path in found {
            self.open_file(path);
        }
        // The note they were opened from stays in front.
        self.active = Some(id);
        self.status = match (opened, missing.is_empty()) {
            (1, true) => "Opened 1 note".into(),
            (count, true) => format!("Opened {count} notes"),
            (count, false) => format!(
                "Opened {count} of the notes; there is no note called {} yet",
                missing.join(", ")
            ),
        };
        Task::none()
    }

    /// Opens the note link field `index` of the active tab points to. If there is none,
    /// makes it in the folder the field links to, with that folder's type.
    fn follow_link_field(&mut self, index: usize) -> Task<Message> {
        // A links field's note goes on its list, then opens.
        let typed = self
            .active_tab()
            .and_then(|tab| tab.form.as_ref())
            .and_then(|form| {
                let (field, target) = form.link_field(index)?;
                (field.kind == types::Kind::Links && !target.trim().is_empty())
                    .then(|| target.trim().to_string())
            });
        if let Some(target) = typed {
            let Some(tab) = self.active_tab_mut() else {
                return Task::none();
            };
            let Some(form) = &mut tab.form else {
                return Task::none();
            };
            tab.modified |= form.update(form::Message::ListAdd(index, target.clone()));
            let item = form.list_items(index).and_then(|(_, items)| {
                items
                    .iter()
                    .position(|item| item.eq_ignore_ascii_case(&target))
            });
            return match item {
                Some(item) => self.follow_list_item(index, item),
                None => Task::none(),
            };
        }
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
                // A new note is empty, so it opens ready to write in.
                if let Some(tab) = self.active_tab_mut() {
                    tab.preview = None;
                }
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

    /// Rewrites `[[links]]` across the campaign after the notes in `moved` were renamed
    /// or moved (from the first path to the second), so they still point to them. Open tabs
    /// are updated too, unsaved changes and all.
    ///
    /// The status bar says `done`, then how many links changed.
    fn relink(&mut self, moved: &[(PathBuf, PathBuf)], done: &str) {
        let notes = markdown_files(&self.tree);
        // Already read again after the rename, with the notes' aliases.
        let after = self.index.clone();
        let before = links::Index::new(
            &self.root,
            notes.iter().map(|path| {
                moved
                    .iter()
                    .find(|(_, new)| new == path)
                    .map_or_else(|| path.clone(), |(old, _)| old.clone())
            }),
        );
        let rename = links::Rename {
            moved,
            before: &before,
            after: &after,
        };

        let mut changed_links = 0;
        let mut changed_notes = 0;
        let mut problems = Vec::new();

        // The walkthrough keeps track of the notes it made.
        let saved = match &mut self.walkthrough {
            Some(walkthrough) => {
                !walkthrough.progress.renamed(&self.root, moved)
                    || walkthrough.progress.save(&self.root).is_ok()
            }
            None => {
                let mut progress = walkthrough::Progress::load(&self.root);
                !progress.renamed(&self.root, moved) || progress.save(&self.root).is_ok()
            }
        };
        if !saved {
            problems.push(".dungeon_elements/walkthrough.yaml".to_string());
        }

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
    fn move_note(&mut self, path: &Path, folder: &Path) -> Result<(), String> {
        let Some(name) = path.file_name() else {
            return Ok(());
        };
        if path.parent() == Some(folder) || !folder.starts_with(&self.root) {
            return Ok(());
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
            return Err(format!("Save {label} before moving it"));
        }
        if target.exists() {
            return Err(format!(
                "Can't move {label}: {where_to} already has a note with that name"
            ));
        }

        let ty = match self.types.get(folder) {
            Some(Ok(ty)) => Some(ty.clone()),
            _ => None,
        };
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("Could not read {label}: {error}"))?;
        let retyped = ty.as_ref().and_then(|ty| retype(&text, ty, &label));

        std::fs::rename(path, &target)
            .map_err(|error| format!("Could not move {label}: {error}"))?;
        let mut done = String::new();
        if let Some(retyped) = &retyped
            && let Err(error) = std::fs::write(&target, retyped)
        {
            done = format!("Moved {label}, but could not give it its new type ({error})");
        }

        self.expanded.insert(folder.to_path_buf());
        self.rescan();
        self.retarget_tabs(path, &target);

        let done = match (&ty, retyped.is_some()) {
            _ if !done.is_empty() => done,
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
        self.relink(&[(path.to_path_buf(), target)], &done);
        Ok(())
    }

    /// Whether the walkthrough's tab is the one showing.
    fn walkthrough_shown(&self) -> bool {
        self.walkthrough
            .as_ref()
            .is_some_and(|walkthrough| Some(walkthrough.id) == self.active)
    }

    /// Opens the walkthrough in a tab where the GM left off, or shows its tab.
    fn show_walkthrough(&mut self) {
        if let Some(walkthrough) = &self.walkthrough {
            self.active = Some(walkthrough.id);
            self.refresh_walkthrough();
            return;
        }
        let id = self.next_id;
        let campaign = walkthrough::Campaign {
            root: &self.root,
            types: &self.types,
            index: &self.index,
        };
        let tabs = &self.tabs;
        match walkthrough_tab::Walkthrough::open(id, &campaign, &|path| open_text(tabs, path)) {
            Ok(walkthrough) => {
                self.next_id += 1;
                self.walkthrough = Some(walkthrough);
                self.active = Some(id);
            }
            Err(error) => self.status = error,
        }
    }

    /// Reads the walkthrough's questions and answers again from the notes.
    fn refresh_walkthrough(&mut self) {
        let Some(mut walkthrough) = self.walkthrough.take() else {
            return;
        };
        let campaign = walkthrough::Campaign {
            root: &self.root,
            types: &self.types,
            index: &self.index,
        };
        let tabs = &self.tabs;
        walkthrough.refresh(&campaign, &|path| open_text(tabs, path));
        self.walkthrough = Some(walkthrough);
    }

    /// Closes the walkthrough's tab. Where the GM was is kept for next time.
    fn close_walkthrough(&mut self) {
        let Some(walkthrough) = self.walkthrough.take() else {
            return;
        };
        if let Err(error) = walkthrough.progress.save(&self.root) {
            self.status = format!("Could not remember where the walkthrough was: {error}");
        }
        if self.active == Some(walkthrough.id) {
            self.active = self.tabs.first().map(|tab| tab.id);
        }
    }

    /// Puts the cursor in the walkthrough's answer box.
    fn focus_walkthrough(&self) -> Task<Message> {
        let id = self
            .walkthrough
            .as_ref()
            .and_then(walkthrough_tab::Walkthrough::focus_id);
        match id {
            Some(id) if self.walkthrough_shown() => iced::widget::operation::focus(id),
            _ => Task::none(),
        }
    }

    fn walkthrough_message(&mut self, message: walkthrough_tab::Message) -> Task<Message> {
        use walkthrough_tab::Message as Step;
        // Ctrl+Enter sends Next wherever the cursor is.
        if !self.walkthrough_shown() {
            return Task::none();
        }
        match &message {
            Step::Open(path) => {
                self.open_file(path.clone());
                return Task::none();
            }
            Step::Name(..)
            | Step::PasteNames(..)
            | Step::PastedNames(..)
            | Step::NameEntered(_)
            | Step::AddName
            | Step::RemoveName(_) => {
                let done = self
                    .walkthrough
                    .as_mut()
                    .and_then(|walkthrough| walkthrough.update_names(message));
                return self.names_done(done);
            }
            Step::Import => {
                return Task::perform(pick_imports(), |files| {
                    Message::Walkthrough(walkthrough_tab::Message::Imported(files))
                });
            }
            Step::Imported(files) => {
                let files = files.clone();
                return self.import_into_walkthrough(&files);
            }
            Step::PickTable(table) => {
                let key = self
                    .walkthrough
                    .as_ref()
                    .map(|walkthrough| walkthrough.pick_key.clone())
                    .unwrap_or_default();
                if key.is_empty() {
                    return Task::none();
                }
                // The answer keeps the keyboard.
                return self
                    .update(Message::PickTable(key, table.clone()))
                    .chain(self.focus_walkthrough());
            }
            Step::Pick(link) => {
                let link = link.clone();
                return self.walkthrough_pick_link(&link);
            }
            Step::Make(folder, name) => {
                let ty = match self.types.get(folder) {
                    Some(Ok(ty)) => Some(ty.clone()),
                    _ => None,
                };
                let path = match create_linked_note(&self.root, name, Some(folder), ty.as_ref()) {
                    Ok(path) => path,
                    Err(error) => {
                        self.status = error;
                        return Task::none();
                    }
                };
                self.rescan();
                self.reveal(&path);
                self.status = format!("Made {}", display_name(&path, false));
                let link = self
                    .index
                    .link_text(&path, &self.walkthrough_from(), false)
                    .unwrap_or_else(|| name.clone());
                return self.walkthrough_input(Message::CompleteLink(link));
            }
            Step::Inspire(table) => {
                // A rolled name goes on the list of names.
                if self
                    .walkthrough
                    .as_ref()
                    .is_some_and(walkthrough_tab::Walkthrough::asks_for_names)
                {
                    let from = self.walkthrough_from();
                    let Some((name, found, each_column)) = self.linked_table(table, &from) else {
                        self.status = format!("There is no roll table called {table}");
                        return Task::none();
                    };
                    let value = self.roll_found(&from, &name, &found, each_column);
                    let done = self
                        .walkthrough
                        .as_mut()
                        .and_then(|walkthrough| walkthrough.add_name(&value));
                    return self.names_done(done);
                }
                // The roll goes into the answer, as a field's own table button's does.
                if let Some(index) = self.walkthrough.as_ref().and_then(|w| w.field) {
                    // The answer keeps the keyboard, so Enter goes on to the next question.
                    let roll = self.walkthrough_input(Message::Form(form::Message::RollTable(
                        index,
                        table.clone(),
                    )));
                    return roll.chain(self.focus_walkthrough());
                }
                let from = self.walkthrough_from();
                match self.linked_table(table, &from) {
                    Some((name, table, each_column)) => {
                        self.roll_found(&from, &name, &table, each_column);
                    }
                    None => self.status = format!("There is no roll table called {table}"),
                }
                return Task::none();
            }
            Step::Next => {
                if let Err(error) = self.answer_walkthrough() {
                    if let Some(walkthrough) = &mut self.walkthrough {
                        walkthrough.error = Some(error);
                    }
                    return Task::none();
                }
            }
            _ => {}
        }

        let Some(mut walkthrough) = self.walkthrough.take() else {
            return Task::none();
        };
        let campaign = walkthrough::Campaign {
            root: &self.root,
            types: &self.types,
            index: &self.index,
        };
        let tabs = &self.tabs;
        let open = |path: &Path| open_text(tabs, path);
        match message {
            Step::Chapter(chapter) => walkthrough.open_chapter(chapter, &campaign, &open),
            Step::Back => walkthrough.back(&campaign, &open),
            Step::Next => walkthrough.next(&campaign, &open),
            Step::Skip => walkthrough.skip(&campaign, &open),
            Step::SkipChapter => walkthrough.skip_chapter(&campaign, &open),
            _ => {}
        }
        if let Err(error) = walkthrough.progress.save(&self.root) {
            self.status = format!("Could not remember where the walkthrough was: {error}");
        }
        self.walkthrough = Some(walkthrough);
        self.focus_walkthrough()
    }

    /// The folders the walkthrough's answer links to notes in: the question's own, or the
    /// link folder of a link or links field. Their notes are offered to pick from, and a
    /// new name can be made in any of them.
    fn walkthrough_pick(&self) -> Vec<PathBuf> {
        let Some(walkthrough) = self.walkthrough.as_ref() else {
            return Vec::new();
        };
        let Some((question, _)) = walkthrough.question() else {
            return Vec::new();
        };
        let Some(field) = question
            .field
            .as_ref()
            .zip(walkthrough.form.as_ref())
            .and_then(|(key, form)| form.field(key))
        else {
            return Vec::new();
        };
        if !question.pick.is_empty() {
            return question
                .pick
                .iter()
                .map(|folder| self.root.join(folder.trim_matches('/')))
                .filter(|folder| folder.is_dir())
                .collect();
        }
        match field.kind {
            types::Kind::Link | types::Kind::Links => self.link_folder(field).into_iter().collect(),
            _ => Vec::new(),
        }
    }

    /// Adds the Markdown files `files` to the walkthrough's list of names, each copied
    /// into the list's folder as a note.
    fn import_into_walkthrough(&mut self, files: &[PathBuf]) -> Task<Message> {
        let campaign = walkthrough::Campaign {
            root: &self.root,
            types: &self.types,
            index: &self.index,
        };
        let Some((spec, ty)) = self
            .walkthrough
            .as_ref()
            .and_then(|walkthrough| walkthrough.import_into(&campaign))
        else {
            return Task::none();
        };
        let mut added = Vec::new();
        let mut problems = Vec::new();
        for file in files {
            match walkthrough::import_note(&campaign, &spec, &ty, file) {
                Ok(name) => added.push(name),
                Err(problem) => problems.push(problem),
            }
        }
        if added.is_empty() && problems.is_empty() {
            return Task::none();
        }
        self.rescan();
        let mut done = None;
        for name in &added {
            let path = self
                .root
                .join(spec.folder.trim_matches('/'))
                .join(format!("{}.md", campaign::note_file_name(name)));
            self.reveal(&path);
            if let Some(walkthrough) = self.walkthrough.as_mut() {
                done = walkthrough.add_name(name).or(done);
            }
        }
        self.status = match (added.as_slice(), problems.as_slice()) {
            ([], [problem, ..]) => problem.clone(),
            (added, []) => format!("Added {} to {}", added.join(", "), spec.folder),
            (added, [problem, ..]) => format!(
                "Added {} to {}, but {problem}",
                added.join(", "),
                spec.folder
            ),
        };
        self.names_done(done)
    }

    /// Links the walkthrough's answer to the note `link` names: it is the answer to a
    /// link field, and is added to a list or steps.
    fn walkthrough_pick_link(&mut self, link: &str) -> Task<Message> {
        let Some(walkthrough) = self.walkthrough.as_mut() else {
            return Task::none();
        };
        let Some((key, index)) = walkthrough
            .question()
            .and_then(|(question, _)| question.field.clone())
            .zip(walkthrough.field)
        else {
            return Task::none();
        };
        let Some(form) = walkthrough.form.as_mut() else {
            return Task::none();
        };
        let kind = form.field(&key).map(|field| field.kind);
        match kind {
            Some(types::Kind::Link) => {
                form.set_value(
                    &key,
                    Some(&serde_yaml_ng::Value::String(format!("[[{link}]]"))),
                );
            }
            Some(types::Kind::Links) => {
                form.add_item(index, link);
            }
            Some(_) => {
                form.add_item(index, &format!("[[{link}]]"));
            }
            None => {}
        }
        self.focus_walkthrough()
    }

    /// Expands the folders the file or folder at `path` is in, so the tree shows it.
    fn reveal(&mut self, path: &Path) {
        for folder in path.ancestors().skip(1) {
            if !folder.starts_with(&self.root) || folder == self.root {
                break;
            }
            self.expanded.insert(folder.to_path_buf());
        }
    }

    /// Puts the keyboard in the name box a list of names says, or goes on.
    fn names_done(&mut self, done: Option<walkthrough_tab::NamesDone>) -> Task<Message> {
        match done {
            Some(walkthrough_tab::NamesDone::Focus(index)) => {
                iced::widget::operation::focus(walkthrough_tab::name_id(index))
            }
            Some(walkthrough_tab::NamesDone::ReadPaste(index, before)) => iced::clipboard::read()
                .map(move |pasted| {
                    Message::Walkthrough(walkthrough_tab::Message::PastedNames(
                        index,
                        before.clone(),
                        pasted.unwrap_or_default(),
                    ))
                }),
            Some(walkthrough_tab::NamesDone::Next) => {
                self.walkthrough_message(walkthrough_tab::Message::Next)
            }
            None => Task::none(),
        }
    }

    /// The note links in the walkthrough's answer box are read from: the note the answer
    /// goes into, or else the campaign folder.
    fn walkthrough_from(&self) -> PathBuf {
        self.walkthrough
            .as_ref()
            .and_then(|walkthrough| walkthrough.subject.clone())
            .unwrap_or_else(|| self.root.join("Untitled.md"))
    }

    /// Writes the walkthrough's answer into its note. A note open with unsaved changes
    /// gets it in its tab instead, so they aren't written over.
    fn answer_walkthrough(&mut self) -> Result<(), String> {
        let Some(walkthrough) = &self.walkthrough else {
            return Ok(());
        };
        let (Some((question, _)), Some(answer)) = (walkthrough.question(), walkthrough.answer())
        else {
            return Ok(());
        };
        let key = question.field.clone();
        let subject = walkthrough.subject.clone();
        // Nothing typed and nothing there before: nothing to write.
        if walkthrough.unchanged(&answer) {
            return Ok(());
        }

        if let (Some(path), Some(key), walkthrough::Answer::Value(value)) =
            (&subject, &key, &answer)
            && let Some(index) = self
                .tabs
                .iter()
                .position(|tab| tab.modified && tab.path.as_ref() == Some(path))
        {
            let name = display_name(path, false);
            let tab = &mut self.tabs[index];
            let Some(form) = tab.form.as_mut().filter(|_| !tab.source) else {
                return Err(format!(
                    "{name} is open as Markdown with unsaved changes. Save it first"
                ));
            };
            form.set_value(key, value.as_ref());
            let field = form.field(key).cloned();
            if let Some(field) = field {
                let campaign = walkthrough::Campaign {
                    root: &self.root,
                    types: &self.types,
                    index: &self.index,
                };
                let mut written = walkthrough::Written::default();
                walkthrough::make_linked(&campaign, &field, value.as_ref(), path, &mut written)?;
                if !written.made.is_empty() {
                    self.rescan();
                }
            }
            self.status = format!(
                "Put the answer in {name}'s tab, which has unsaved changes. Save it to keep it"
            );
            if let Some(walkthrough) = &mut self.walkthrough {
                walkthrough.answered();
            }
            return Ok(());
        }

        let before = subject
            .as_deref()
            .and_then(|path| std::fs::read_to_string(path).ok());
        let Some(mut walkthrough) = self.walkthrough.take() else {
            return Ok(());
        };
        let campaign = walkthrough::Campaign {
            root: &self.root,
            types: &self.types,
            index: &self.index,
        };
        let result = walkthrough.write(&campaign, answer);
        let saved = walkthrough.progress.save(&self.root);
        self.walkthrough = Some(walkthrough);
        let written = result?;
        saved.map_err(|error| format!("Could not save the walkthrough's progress: {error}"))?;

        // Notes made show in the file tree, and open ones what was written into them.
        for path in &written.made {
            self.reveal(path);
        }
        for path in written.made.iter().chain(&written.changed) {
            if let Some(index) = self
                .tabs
                .iter()
                .position(|tab| tab.path.as_ref() == Some(path) && !tab.modified)
            {
                let _ = self.reload_tab(index);
            }
        }
        self.rescan();
        if let (Some(path), Some(before)) = (&subject, &before)
            && let Ok(after) = std::fs::read_to_string(path)
            && let Some(Ok(ty)) = path.parent().and_then(|folder| self.types.get(folder))
        {
            let fields = ty.tag_fields();
            self.sync_tags(path, &fields, before, &after);
        }
        self.status = match written.made.as_slice() {
            [] => match &subject {
                Some(path) => format!("Saved the answer in {}", display_name(path, false)),
                None if !written.changed.is_empty() => format!(
                    "Saved the answer in {}",
                    written
                        .changed
                        .iter()
                        .map(|path| display_name(path, false))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                None => "Saved".into(),
            },
            made => format!(
                "Made {}",
                made.iter()
                    .map(|path| display_name(path, false))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        if let Some((path, name)) = written.rename {
            self.rename_note(&path, &name)?;
        }
        Ok(())
    }

    /// Typing and buttons in the walkthrough's answer box, which is a form of its own.
    fn walkthrough_input(&mut self, message: Message) -> Task<Message> {
        let from = self.walkthrough_from();
        let ctrl = self.ctrl;
        match message {
            Message::Form(form::Message::RollTable(index, table)) => {
                let Some((name, found, each_column)) = self.linked_table(&table, &from) else {
                    self.status = format!("There is no roll table called {table}");
                    return Task::none();
                };
                let value = self.roll_found(&from, &name, &found, each_column);
                let Some(form) = self
                    .walkthrough
                    .as_mut()
                    .and_then(|walkthrough| walkthrough.form.as_mut())
                else {
                    return Task::none();
                };
                match (form.text_value(index), form.label(index)) {
                    // A roll is a new step, or a new link, of a list.
                    (None, _) => {
                        form.add_item(index, &value);
                    }
                    (Some((current, _)), _) if current.trim().is_empty() => {
                        form.set_text_value(index, &value, false);
                    }
                    (Some((_, long)), Some(label)) => {
                        self.dialog = Some(Dialog::ReplaceField {
                            index,
                            label,
                            value,
                            long,
                        });
                    }
                    (Some(_), None) => {}
                }
                Task::none()
            }
            Message::FillField(add_line) => {
                if let Some(Dialog::ReplaceField { index, value, .. }) = self.dialog.take()
                    && let Some(form) = self
                        .walkthrough
                        .as_mut()
                        .and_then(|walkthrough| walkthrough.form.as_mut())
                {
                    form.set_text_value(index, &value, add_line);
                }
                Task::none()
            }
            Message::Form(form::Message::Roll(index, expression)) => {
                let label = self
                    .walkthrough
                    .as_ref()
                    .and_then(|walkthrough| walkthrough.form.as_ref())
                    .and_then(|form| form.label(index))
                    .unwrap_or_default();
                self.dice.open = true;
                self.dice.roll_field(&label, &expression);
                Task::none()
            }
            Message::Form(
                form::Message::FollowLink(index)
                | form::Message::FollowItem(index, _)
                | form::Message::FollowStep(index, _),
            ) => {
                // The link in the field, or the item or step clicked.
                let Some(form) = self
                    .walkthrough
                    .as_ref()
                    .and_then(|walkthrough| walkthrough.form.as_ref())
                else {
                    return Task::none();
                };
                let found = match &message {
                    Message::Form(form::Message::FollowItem(_, item)) => {
                        form.list_items(index).and_then(|(field, items)| {
                            Some((field.clone(), items.get(*item)?.clone()))
                        })
                    }
                    Message::Form(form::Message::FollowStep(_, step)) => form
                        .step_link(index, *step)
                        .map(|(field, target)| (field.clone(), target)),
                    _ => form
                        .link_field(index)
                        .map(|(field, target)| (field.clone(), target.to_string())),
                };
                let Some((field, target)) = found else {
                    return Task::none();
                };
                let target = target.split(['|', '#']).next().unwrap_or(&target).trim();
                if target.is_empty() || field.kind == types::Kind::Tags {
                    return Task::none();
                }
                let folder = self.link_folder(&field);
                self.follow_link_into(target, folder)
            }
            Message::Form(form::Message::FollowAll(_)) => Task::none(),
            Message::Form(message) => {
                let click = matches!(
                    message,
                    form::Message::Long(_, text_editor::Action::Click(_))
                );
                let moves_focus = message.moves_focus();
                let Some(form) = self
                    .walkthrough
                    .as_mut()
                    .and_then(|walkthrough| walkthrough.form.as_mut())
                else {
                    return Task::none();
                };
                form.update(message);
                if click
                    && ctrl
                    && let Some(target) = form.link_under_cursor()
                {
                    return self.follow_link(&target);
                }
                if moves_focus && let Some(id) = form.focused_id() {
                    return iced::advanced::widget::operate(form::tab(vec![id], false)).discard();
                }
                Task::none()
            }
            Message::CompleteLink(link) => {
                let Some(form) = self
                    .walkthrough
                    .as_mut()
                    .and_then(|walkthrough| walkthrough.form.as_mut())
                else {
                    return Task::none();
                };
                let focus = form.focused_id();
                form.complete_link(&link);
                match focus {
                    Some(id) => iced::widget::operation::focus(id),
                    None => Task::none(),
                }
            }
            Message::TabField(back) => {
                if self.dialog.is_some() || self.context.is_some() {
                    return Task::none();
                }
                // In a link field or step, Tab finishes the link being typed, as in a form.
                if let Some((link, _)) = self.link_suggestions().notes.first() {
                    let link = link.clone();
                    return self.walkthrough_input(Message::CompleteLink(link));
                }
                let Some(form) = self
                    .walkthrough
                    .as_ref()
                    .and_then(|walkthrough| walkthrough.form.as_ref())
                else {
                    return Task::none();
                };
                iced::advanced::widget::operate(form::tab(form.tab_ids(), back))
                    .map(Message::FieldTabbed)
            }
            Message::FieldTabbed(position) => {
                if let Some(form) = self
                    .walkthrough
                    .as_mut()
                    .and_then(|walkthrough| walkthrough.form.as_mut())
                {
                    form.tabbed_to(position);
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// The walkthrough's tab: its answer box is drawn by its form, with the notes to link
    /// to under it while a link is typed.
    fn walkthrough_view<'a>(
        &'a self,
        c: Colors,
        walkthrough: &'a walkthrough_tab::Walkthrough,
        dice: &'a dice::Icons,
    ) -> Element<'a, Message> {
        let suggestions = self.link_suggestions();
        let accept = suggestions
            .notes
            .first()
            .map(|(link, _)| Message::CompleteLink(link.clone()));
        let answer = walkthrough
            .form
            .as_ref()
            .zip(walkthrough.field)
            .map(|(form, index)| {
                form.field_view(
                    index,
                    c,
                    Message::Form,
                    shortcut,
                    self.link_colors(c),
                    accept,
                    editor_menu,
                    &dice.field,
                    &self.tags,
                    &self.table_keys,
                    &self.table_settings,
                    Some(Message::Walkthrough(walkthrough_tab::Message::Next)),
                )
            });
        // While no link is being typed, the notes the answer can link to are offered.
        let chips = suggestion_chips(c, &suggestions, true).or_else(|| {
            let from = self.walkthrough_from();
            // Notes the answer links to already aren't offered again.
            let linked: Vec<String> = walkthrough
                .answer()
                .and_then(|answer| match answer {
                    walkthrough::Answer::Value(Some(value)) => {
                        serde_yaml_ng::to_string(&value).ok()
                    }
                    _ => None,
                })
                .map(|text| {
                    links::targets(&text)
                        .iter()
                        .map(|target| links::normalize(target))
                        .collect()
                })
                .unwrap_or_default();
            let mut notes: Vec<(String, String)> = Vec::new();
            for folder in self.walkthrough_pick() {
                for note in self.index.in_folder(&folder, &from) {
                    let target = note.0.split('|').next().unwrap_or_default();
                    if !notes.contains(&note) && !linked.contains(&links::normalize(target)) {
                        notes.push(note);
                    }
                }
            }
            if notes.is_empty() {
                return None;
            }
            let chips = notes.into_iter().take(PICKS).map(|(link, label)| {
                button(text(label).size(13))
                    .padding([3, 10])
                    .style(style::secondary(c))
                    .on_press(Message::Walkthrough(walkthrough_tab::Message::Pick(link)))
                    .into()
            });
            Some(
                column![
                    text("Pick one you've made, or type a new name")
                        .size(12)
                        .color(c.text_muted),
                    row(chips).spacing(6).wrap(),
                ]
                .spacing(6)
                .into(),
            )
        });
        walkthrough.view(
            c,
            answer,
            chips,
            &dice.field,
            &self.table_settings,
            Message::Walkthrough,
        )
    }

    /// Renames the note at `path` to `name`, in the same folder. Its name field changes
    /// too, and links to it are rewritten.
    fn rename_note(&mut self, path: &Path, name: &str) -> Result<(), String> {
        let name = name.trim();
        let name = name
            .strip_suffix(".md")
            .or_else(|| name.strip_suffix(".MD"))
            .unwrap_or(name)
            .trim();
        if name.is_empty() {
            return Err("The name can't be empty".into());
        }
        // The name field gets the name as typed, and the file a name any system can hold.
        let file_name = campaign::note_file_name(name);
        let label = display_name(path, false);
        if self
            .tabs
            .iter()
            .any(|tab| tab.modified && tab.path.as_deref() == Some(path))
        {
            return Err(format!("Save {label} before renaming it"));
        }

        let folder = path.parent().unwrap_or(&self.root);
        let target = folder.join(format!("{file_name}.md"));
        if target == path && path.file_stem().is_some_and(|stem| stem == name) {
            return Ok(());
        }
        // Changing only the case of the name renames the same file on Windows.
        let same_file =
            target.to_string_lossy().to_lowercase() == path.to_string_lossy().to_lowercase();
        if target.exists() && !same_file && target != path {
            return Err(format!("{file_name}.md already exists in this folder"));
        }

        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("Could not read {label}: {error}"))?;
        let wants_name = matches!(
            self.form_for(path, &text),
            Ok(Some(form)) if form.ty.name_field().is_some()
        );
        let old_name = path.file_stem().map(|stem| stem.to_string_lossy());
        let renamed = with_name(&text, name, wants_name)
            .map(|text| with_alias(&text, name, old_name.as_deref()));

        if target != path {
            std::fs::rename(path, &target)
                .map_err(|error| format!("Could not rename {label}: {error}"))?;
        }
        let mut done = format!("Renamed {label} to {name}");
        if let Some(renamed) = &renamed
            && let Err(error) = std::fs::write(&target, renamed)
        {
            done =
                format!("Renamed {label} to {name}, but could not change its name field ({error})");
        }

        self.rescan();
        self.retarget_tabs(path, &target);
        self.relink(&[(path.to_path_buf(), target)], &done);
        Ok(())
    }

    /// Moves (or renames) the folder `old` to `new`, with everything in it. Links to its
    /// notes, and link fields that link to it, are rewritten.
    fn move_folder(&mut self, old: &Path, new: &Path) -> Result<(), String> {
        let label = self.relative(old);
        if new == old {
            return Ok(());
        }
        if new.starts_with(old) {
            return Err(format!("Can't move {label} into itself"));
        }
        if self
            .tabs
            .iter()
            .any(|tab| tab.modified && tab.path.as_ref().is_some_and(|path| path.starts_with(old)))
        {
            return Err(format!("Save the notes open from {label} first"));
        }
        let same_folder =
            new.to_string_lossy().to_lowercase() == old.to_string_lossy().to_lowercase();
        if new.exists() && !same_folder {
            return Err(format!("{} already exists", self.relative(new)));
        }

        let moved: Vec<(PathBuf, PathBuf)> = markdown_files(&self.tree)
            .into_iter()
            .filter_map(|note| {
                let inside = note.strip_prefix(old).ok()?.to_path_buf();
                Some((note, new.join(inside)))
            })
            .collect();
        std::fs::rename(old, new).map_err(|error| format!("Could not move {label}: {error}"))?;

        self.expanded = self
            .expanded
            .drain()
            .map(|folder| match folder.strip_prefix(old) {
                Ok(inside) => new.join(inside),
                Err(_) => folder,
            })
            .collect();
        if let Some(parent) = new.parent() {
            self.expanded.insert(parent.to_path_buf());
        }
        self.rescan();
        let mut done = format!("Moved {label} to {}", self.relative(new));
        if new.parent() == old.parent() {
            done = format!("Renamed {label} to {}", self.relative(new));
        }
        if let Err(problem) = self.retarget_link_fields(old, new) {
            done.push_str(&format!(" ({problem})"));
        }
        self.retarget_tabs(old, new);
        self.relink(&moved, &done);
        Ok(())
    }

    /// Points link fields that link to folder `old`, or a folder in it, to where it moved.
    fn retarget_link_fields(&mut self, old: &Path, new: &Path) -> Result<(), String> {
        let mut changed = Vec::new();
        for (folder, ty) in &self.own_types {
            let Ok(ty) = ty else {
                continue;
            };
            let mut ty = ty.clone();
            let mut any = false;
            for field in &mut ty.fields {
                let Some(linked) = &field.folder else {
                    continue;
                };
                let Ok(inside) = self
                    .root
                    .join(linked.trim_matches('/'))
                    .strip_prefix(old)
                    .map(Path::to_path_buf)
                else {
                    continue;
                };
                let moved = new.join(inside);
                let relative = moved.strip_prefix(&self.root).unwrap_or(&moved);
                field.folder = Some(relative.to_string_lossy().replace('\\', "/"));
                any = true;
            }
            if any {
                changed.push((folder.clone(), ty));
            }
        }
        if changed.is_empty() {
            return Ok(());
        }

        let mut problems = Vec::new();
        for (folder, ty) in changed {
            let written = ty.to_yaml().and_then(|yaml| {
                std::fs::write(folder.join(types::TYPE_FILE), yaml)
                    .map_err(|error| error.to_string())
            });
            if written.is_err() {
                problems.push(self.relative(&folder.join(types::TYPE_FILE)));
            }
        }
        self.rescan();
        self.reload_forms();
        match problems.is_empty() {
            true => Ok(()),
            false => Err(format!(
                "could not update the link fields in {}",
                problems.join(", ")
            )),
        }
    }

    /// Points the tabs of files at `old`, or in the folder `old`, to where they are now
    /// that it moved to `new`, and reads them again.
    fn retarget_tabs(&mut self, old: &Path, new: &Path) {
        for index in 0..self.tabs.len() {
            let Some(inside) = self.tabs[index]
                .path
                .as_ref()
                .and_then(|path| path.strip_prefix(old).ok())
            else {
                continue;
            };
            let path = if inside.as_os_str().is_empty() {
                new.to_path_buf()
            } else {
                new.join(inside)
            };
            self.tabs[index].folder = path.parent().unwrap_or(&self.root).to_path_buf();
            self.tabs[index].path = Some(path);
            let _ = self.reload_tab(index);
        }
    }

    /// Puts an error from the open dialog's action in the dialog, or closes it.
    fn dialog_result(&mut self, result: Result<(), String>) {
        match (result, &mut self.dialog) {
            (Ok(()), _) => self.dialog = None,
            (
                Err(message),
                Some(
                    Dialog::Rename { error, .. }
                    | Dialog::Move { error, .. }
                    | Dialog::NewFolder { error, .. }
                    | Dialog::NewTable { error, .. }
                    | Dialog::NoteFromRoll { error, .. }
                    | Dialog::AddField { error, .. },
                ),
            ) => *error = Some(message),
            (Err(message), _) => self.status = message,
        }
    }

    /// The text editor that the editing items of a right-click menu work on, and its
    /// widget id: the block being edited in the reading view, the long text field last
    /// typed in, or the note's editor.
    fn focused_text(&self) -> Option<&text_editor::Content> {
        let tab = self.active_tab()?;
        match &tab.edit {
            Some(ReadEdit::Block { content, .. }) => Some(content),
            Some(ReadEdit::Table { editor, .. }) => editor.focused_cell(),
            Some(ReadEdit::Field(_)) => None,
            None if tab.reading() => None,
            None if tab.shows_form() => tab.form.as_ref()?.focused_text(),
            None => Some(&tab.content),
        }
    }

    /// Like [`Self::focused_text`], to change it, with the widget id to focus it again.
    fn focused_text_mut(&mut self) -> Option<(&mut text_editor::Content, String)> {
        let tab = self.active_tab_mut()?;
        let reading = tab.reading();
        let shows_form = tab.shows_form();
        match &mut tab.edit {
            Some(ReadEdit::Block { content, .. }) => Some((content, BLOCK_EDITOR.into())),
            Some(ReadEdit::Table { editor, .. }) => editor.focused_cell_mut(),
            Some(ReadEdit::Field(_)) => None,
            None if reading => None,
            None if shows_form => tab.form.as_mut()?.focused_text_mut(),
            None => Some((&mut tab.content, EDITOR.into())),
        }
    }

    /// Changes the text editor being typed in (see [`Self::focused_text_mut`]) with
    /// `change`, and gives it back the focus. If there is none, says `hint` instead.
    fn change_focused_text(
        &mut self,
        change: impl FnOnce(&mut text_editor::Content),
        hint: &str,
    ) -> Task<Message> {
        if !self.active_tab().is_some_and(Tab::is_note) {
            return Task::none();
        }
        let Some((content, id)) = self.focused_text_mut() else {
            self.status = hint.into();
            return Task::none();
        };
        change(content);
        if let Some(tab) = self.active_tab_mut() {
            tab.modified = true;
        }
        iced::widget::operation::focus(id)
    }

    /// The formatting toolbar, while a note is being edited.
    fn format_bar(&self, c: Colors) -> Option<Element<'_, Message>> {
        let tab = self.active_tab().filter(|tab| tab.is_note())?;
        if tab.reading() && !matches!(tab.edit, Some(ReadEdit::Block { .. })) {
            return None;
        }
        let content = self.focused_text();
        // A form's long text fields are the note's `## ` sections, so headings in them
        // start at `###`.
        let types = format::TextType::ALL
            .into_iter()
            .filter(|kind| !(tab.shows_form() && matches!(kind, format::TextType::Heading(1 | 2))))
            .collect();
        Some(format_bar(
            c,
            types,
            content.map(|content| (format::text_type(content), format::list(content))),
        ))
    }

    fn editor_body<'a>(
        &'a self,
        c: Colors,
        suggestions: &Suggestions,
        dice: &'a dice::Icons,
    ) -> Element<'a, Message> {
        let Some(tab) = self.active_tab() else {
            return center(
                column![
                    text("No file open").size(20).color(c.text_muted),
                    text("Pick a file in the sidebar, or press Ctrl+N for a new note.")
                        .size(14)
                        .color(c.text_muted),
                    button(
                        text("Build your world with the walkthrough")
                            .size(14)
                            .color(c.accent)
                    )
                    .padding([4, 10])
                    .style(style::flat(c, false))
                    .on_press(Message::ShowWalkthrough),
                ]
                .spacing(8)
                .align_x(Center),
            )
            .style(style::panel(c.background))
            .into();
        };

        let accept = suggestions
            .notes
            .first()
            .map(|(link, _)| Message::CompleteLink(link.clone()));

        if let Some(preview) = &tab.preview {
            // The notes the link being typed could be, under the block or field it is in.
            let chips = || {
                let chips =
                    suggestion_chips(c, suggestions, true).map(|chips| -> Element<'_, Message> {
                        container(chips)
                            .padding([6, 10])
                            .width(Fill)
                            .style(style::panel(c.sidebar))
                            .into()
                    });
                // Always there, so the editor above it keeps its focus when it shows up.
                chips.unwrap_or_else(|| space().into())
            };
            let mut editing = preview::Editing {
                on_link: Message::PreviewLink,
                on_block: Message::EditBlock,
                on_hover: Message::HoverBlock,
                on_task: Message::ToggleTask,
                on_field: Message::EditField,
                on_roll: Message::RollField,
                roll_icon: dice.field.clone(),
                on_roll_table: Message::RollTable,
                on_edit_table: Message::EditTable,
                on_roll_into: Message::RollIntoField,
                on_pick: Message::PickTable,
                picks: &self.table_settings,
                on_each_column: Message::TableEachColumn,
                each_column: self.each_column_tables(&tab.note_path(), preview.source()),
                on_add: Message::AddText,
                on_page: Message::FinishEditing,
                block: None,
                field: None,
            };
            match &tab.edit {
                Some(ReadEdit::Block { index, content, .. }) => {
                    let editor = block_editor(c, content, self.link_colors(c), accept.clone());
                    editing.block = Some((*index, column![editor, chips()].spacing(4).into()));
                }
                Some(ReadEdit::Field(key)) => {
                    editing.field = tab
                        .form
                        .as_ref()
                        .and_then(|form| field_input(c, form, key, accept.clone(), &self.tags))
                        .map(|input| (key.clone(), column![input, chips()].spacing(4).into()));
                }
                Some(ReadEdit::Table { index, editor, .. }) => {
                    let editor =
                        editor.view(c, Message::TableEditor, shortcut, self.link_colors(c));
                    editing.block = Some((*index, column![editor, chips()].spacing(4).into()));
                }
                None => {}
            }
            let linked_from = self.linked_from(&tab.note_path());
            let preview =
                container(preview.view(c, self.index.names(), &self.web, linked_from, editing))
                    .width(Fill)
                    .height(Fill)
                    .style(style::panel(c.background));
            return context_menu::area(preview, |position| {
                Message::OpenContextMenu(Target::Reading, position)
            })
            .into();
        }

        if let Some(form) = &tab.form
            && !tab.source
        {
            let form = container(form.view(
                c,
                Message::Form,
                shortcut,
                self.link_colors(c),
                accept,
                editor_menu,
                &dice.field,
                &self.tags,
                &self.table_keys,
                &self.table_settings,
            ))
            .width(Fill)
            .height(Fill)
            .style(style::panel(c.background));

            return context_menu::area(
                stack![form, corner_button(c, Message::ShowAddField)],
                |position| Message::OpenContextMenu(Target::Form, position),
            )
            .into();
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
                if (key_press.modifiers.command() || key_press.modifiers.alt())
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

        let editor: Element<'_, Message> = if tab.is_note() {
            editor
                .highlight_with::<links::Highlighter>(self.link_colors(c), links::format)
                .into()
        } else {
            editor.into()
        };
        context_menu::area(editor, editor_menu)
            .click_first(tab.content.selection().is_none())
            .into()
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

    with_front_matter(&retyped, body)
}

/// The note's `text` with its name field set to `name`, if it has one or `wants_name`
/// (because its type has one). Returns `None` if there is nothing to change, or the
/// front matter can't be read.
fn with_name(text: &str, name: &str, wants_name: bool) -> Option<String> {
    let (front, body) = note::split_front_matter(text).ok()?;
    let mut front = match front {
        Some(yaml) => match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(yaml).ok()? {
            serde_yaml_ng::Value::Mapping(front) => front,
            serde_yaml_ng::Value::Null => serde_yaml_ng::Mapping::new(),
            _ => return None,
        },
        None => serde_yaml_ng::Mapping::new(),
    };
    if !wants_name && !front.contains_key("name") {
        return None;
    }
    // A name already there keeps its place.
    front.insert("name".into(), name.into());
    with_front_matter(&front, body)
}

/// A note with the front matter `front`, then `body`.
fn with_front_matter(front: &serde_yaml_ng::Mapping, body: &str) -> Option<String> {
    let yaml = serde_yaml_ng::to_string(front).ok()?;
    let body = body.trim_start_matches(['\r', '\n']);
    Some(if body.is_empty() {
        format!("---\n{yaml}---\n")
    } else {
        format!("---\n{yaml}---\n\n{body}")
    })
}

/// A small button in the file tree's header, with a tooltip saying what it does.
fn tree_button<'a>(
    c: Colors,
    label: &'a str,
    tip: &'a str,
    message: Message,
) -> Element<'a, Message> {
    tooltip(
        button(text(label).size(13))
            .padding([0, 6])
            .style(style::flat(c, false))
            .on_press(message),
        container(text(tip).size(12))
            .padding([2, 6])
            .style(style::dropdown(c)),
        tooltip::Position::Bottom,
    )
    .into()
}

/// Lists every folder in the tree.
fn folders_of(entries: &[Entry]) -> Vec<PathBuf> {
    let mut folders = Vec::new();
    for entry in entries {
        if let Some(children) = &entry.children {
            folders.push(entry.path.clone());
            folders.extend(folders_of(children));
        }
    }
    folders
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

/// What a scan of the campaign's notes found, besides the file tree.
#[derive(Default)]
struct Scan {
    /// A hash of each note's text.
    hashes: HashMap<PathBuf, u64>,
    /// The names of the notes whose name field holds characters their file name can't
    /// (see [`Workspace::titles`]).
    titles: HashMap<PathBuf, String>,
    /// Each note's other names, from its `aliases`.
    aliases: HashMap<PathBuf, Vec<String>>,
    /// Each note's links, for [`backlinks::Backlinks`].
    mentions: HashMap<PathBuf, Vec<(String, backlinks::Mention)>>,
}

/// Reads every note in the file tree for a [`Scan`].
fn scan_notes(tree: &[Entry]) -> Scan {
    use std::hash::{Hash, Hasher};

    let mut scan = Scan::default();
    for path in markdown_files(tree) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        scan.hashes.insert(path.clone(), hasher.finish());
        scan.mentions
            .insert(path.clone(), backlinks::mentions(&text));

        let Ok(note) = note::parse(&text) else {
            continue;
        };
        if let Some(title) = title(&path, &note) {
            scan.titles.insert(path.clone(), title);
        }
        let aliases: Vec<String> = match note.front.get(form::ALIASES_KEY) {
            Some(serde_yaml_ng::Value::Sequence(items)) => {
                items.iter().map(note::value_to_text).collect()
            }
            Some(serde_yaml_ng::Value::String(alias)) => vec![alias.clone()],
            _ => Vec::new(),
        };
        if !aliases.is_empty() {
            scan.aliases.insert(path, aliases);
        }
    }
    scan
}

/// The note `text`, with `name` among its aliases if its file name can't be `name`, so
/// Obsidian's link search finds it by the name as written. The alias it had for its old
/// name `old` (a file name), if any, goes.
fn with_alias(text: &str, name: &str, old: Option<&str>) -> String {
    let name = name.trim();
    let mut text = text.to_string();
    if let Some(old) = old
        && let Ok(without) = fix::remove_alias_for(&text, old, name)
    {
        text = without;
    }
    if campaign::note_file_name(name) == name {
        return text;
    }
    fix::add_alias(&text, name).unwrap_or(text)
}

/// The name to show for the note at `path` holding `text`, if it isn't its file name:
/// its name field, when that is the name the file was named after (like `Chapter 1:
/// The Inn` for `Chapter 1 - The Inn.md`).
fn title(path: &Path, note: &note::Note) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy();
    let name = note::value_to_text(note.front.get("name")?);
    let name = name.trim();
    (name != stem && campaign::note_file_name(name) == stem).then(|| name.to_string())
}

/// Which of the notes `removed` since the last scan were renamed or moved to which of
/// the notes `added`, as `(old, new)` paths, going by the notes' text hashes before and after.
///
/// A note counts as renamed if one of the new notes has exactly its text. If just one
/// note went and one came, in the same folder or with the same name, it counts as
/// renamed too, even if its text was also edited.
fn renamed_notes(
    removed: &[PathBuf],
    added: &[PathBuf],
    before: &HashMap<PathBuf, u64>,
    after: &HashMap<PathBuf, u64>,
) -> Vec<(PathBuf, PathBuf)> {
    let mut removed: Vec<&PathBuf> = removed.iter().collect();
    let mut added: Vec<&PathBuf> = added.iter().collect();
    removed.sort();
    added.sort();

    let mut moved = Vec::new();
    removed.retain(|old| {
        let hash = before.get(*old);
        match added.iter().position(|new| after.get(*new) == hash) {
            Some(index) => {
                moved.push(((*old).clone(), added.remove(index).clone()));
                false
            }
            None => true,
        }
    });

    if let ([old], [new]) = (removed.as_slice(), added.as_slice())
        && (old.parent() == new.parent() || old.file_name() == new.file_name())
    {
        moved.push(((*old).clone(), (*new).clone()));
    }
    moved
}

/// The text of the note at `path` as its tab in `tabs` has it, if it is open with
/// unsaved changes.
fn open_text(tabs: &[Tab], path: &Path) -> Option<String> {
    tabs.iter()
        .find(|tab| tab.modified && tab.path.as_deref() == Some(path))
        .and_then(|tab| tab.text().ok())
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
    let parts: Vec<&str> = target.split('/').map(str::trim).collect();
    let (name, folders) = parts.split_last().unwrap_or((&"", &[]));
    for part in folders {
        campaign::validate_name(part)
            .map_err(|error| format!("Can't make a note for [[{target}]]: {error}"))?;
        path.push(part);
    }
    // The file gets a name any system can hold; the name field keeps the link's name.
    path.push(campaign::note_file_name(name));
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
                front.insert("name".into(), (*name).into());
            }
            if campaign::note_file_name(name) != *name {
                front.insert(
                    form::ALIASES_KEY.into(),
                    serde_yaml_ng::Value::Sequence(vec![(*name).into()]),
                );
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

/// Whether `folder` already has a note called `name`.
fn path_taken(folder: &Path, name: &str) -> bool {
    let mut path = folder.join(campaign::note_file_name(name));
    path.as_mut_os_string().push(".md");
    path.exists()
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

/// Reads the `_type.yaml` of every folder in the campaign: what each says, and each
/// folder's type with what it inherits.
/// The tags that have notes in the campaign at `root`: those in the folder of each tags
/// field of `types`, and in the `tags` folder.
fn known_tags(root: &Path, types: &types::Types) -> form::KnownTags {
    let mut folders = vec![types::TAG_FOLDER.to_string()];
    for ty in types.values().flatten() {
        for field in &ty.fields {
            if field.kind == types::Kind::Tags && field.key != form::ALIASES_KEY {
                folders.push(field.tag_folder());
            }
        }
    }
    folders
        .into_iter()
        .map(|folder| {
            let tags = tags::known(root, &folder);
            (folder, tags)
        })
        .collect()
}

fn load_types(root: &Path, tree: &[Entry]) -> (types::Types, types::Types) {
    fn visit(entries: &[Entry], types: &mut types::Types) {
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

    let mut folders = Vec::new();
    collect_folders(tree, &mut folders);
    let all = types::resolve(root, &folders, &types);
    (types, all)
}

/// Saves a note as "<name>.md" in its folder, renaming the file if the name changed.
fn save_named(tab: &Tab, text: &str, label: &str) -> Result<PathBuf, String> {
    let name = tab
        .note_name()
        .ok_or_else(|| format!("Give the {label} a name before saving"))?;
    // The name field keeps the name as typed; the file gets a name any system can hold.
    let name = campaign::note_file_name(&name);

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

/// Whether `problem` is a broken link the user chose to leave as it is, in `ignored`.
fn is_ignored(ignored: &HashSet<(PathBuf, String)>, problem: &Problem) -> bool {
    match &problem.issue {
        check::Issue::Link { target, .. } => {
            ignored.contains(&(problem.path.clone(), links::normalize(target)))
        }
        _ => false,
    }
}

/// The number of files inside `folder` that have problems.
fn problem_files_in(problems: &[Problem], folder: &Path) -> usize {
    let files: HashSet<&Path> = problems
        .iter()
        .filter(|problem| problem.path.starts_with(folder))
        .map(|problem| problem.path.as_path())
        .collect();
    files.len()
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

/// Shown above a note with problems: the first one, and a button to fix them.
fn problems_banner<'a>(c: Colors, path: &Path, problems: &[&Problem]) -> Element<'a, Message> {
    let summary = match problems {
        [problem] => problem.message.clone(),
        [problem, rest @ ..] => format!("{} (and {} more)", problem.message, rest.len()),
        [] => String::new(),
    };
    let fixable = campaign::is_markdown(path);
    container(
        row![
            text("⚠").size(14).color(c.warning),
            text(summary)
                .size(13)
                .color(c.text)
                .wrapping(text::Wrapping::None),
            space::horizontal(),
        ]
        .push(fixable.then(|| {
            button(text("Fix...").size(13))
                .style(style::secondary(c))
                .on_press(Message::FixProblems(path.to_path_buf()))
        }))
        .spacing(8)
        .align_y(Center),
    )
    .padding([6, 16])
    .width(Fill)
    .clip(true)
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

/// Maps F1 and Ctrl+/, which open the Help window on any screen.
fn help_key(key: &Key, modifiers: Modifiers) -> Option<Message> {
    match key.as_ref() {
        Key::Named(keyboard::key::Named::F1) => Some(Message::ShowHelp(help::Page::Guide(0))),
        Key::Character("/") if modifiers.command() => {
            Some(Message::ShowHelp(help::Page::Shortcuts))
        }
        _ => None,
    }
}

/// Maps the workspace keyboard shortcuts to their messages.
/// Tab or Shift+Tab, which move between the fields of the form.
fn tab_key(key: &Key, modifiers: Modifiers) -> Option<Message> {
    let tab = matches!(key, Key::Named(keyboard::key::Named::Tab));
    (tab && !modifiers.command() && !modifiers.alt()).then(|| Message::TabField(modifiers.shift()))
}

fn shortcut(key: &Key, modifiers: Modifiers) -> Option<Message> {
    if let Some(message) = menu_key(key, modifiers) {
        return Some(message);
    }
    if !modifiers.command() {
        return None;
    }
    // Next in the walkthrough, from any answer box.
    if let Key::Named(keyboard::key::Named::Enter) = key.as_ref() {
        return Some(Message::Walkthrough(walkthrough_tab::Message::Next));
    }
    if let Some(command) = Command::from_key(key, modifiers) {
        return Some(Message::Run(command));
    }

    let Key::Character(c) = key.as_ref() else {
        return None;
    };
    match c.to_ascii_lowercase().as_str() {
        "b" => Some(Message::Format(format::Command::Bold)),
        "i" => Some(Message::Format(format::Command::Italic)),
        _ => None,
    }
}

/// Maps Alt and a menu's first letter, which opens the menu.
fn menu_key(key: &Key, modifiers: Modifiers) -> Option<Message> {
    if !modifiers.alt() || modifiers.command() || modifiers.shift() {
        return None;
    }
    let Key::Character(c) = key.as_ref() else {
        return None;
    };
    Menu::from_letter(c).map(Message::MenuKey)
}

/// Maps the keys that move about an open menu.
fn arrow_key(key: &Key) -> Option<Message> {
    use keyboard::key::Named;
    let arrow = match key.as_ref() {
        Key::Named(Named::ArrowUp) => Arrow::Up,
        Key::Named(Named::ArrowDown) => Arrow::Down,
        Key::Named(Named::ArrowLeft) => Arrow::Left,
        Key::Named(Named::ArrowRight) => Arrow::Right,
        Key::Named(Named::Enter | Named::Space) => Arrow::Enter,
        _ => return None,
    };
    Some(Message::MenuArrow(arrow))
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
        button(
            text("New here? Read the how-to guides (F1)")
                .size(13)
                .color(c.accent)
        )
        .style(style::flat(c, false))
        .on_press(Message::ShowHelp(help::Page::Guide(0))),
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
            label("Start from"),
            row![
                pick_list(
                    form.templates.clone(),
                    Some(form.template.clone()),
                    Message::TemplatePicked
                )
                .padding(8)
                .width(Fill)
                .style(style::choice(c))
                .menu_style(style::choice_menu(c)),
                button("Import...")
                    .style(style::secondary(c))
                    .padding(8)
                    .on_press(Message::ImportTemplate),
            ]
            .spacing(8),
            match &form.structure {
                Ok(structure) => text(template_summary(structure)).color(c.text_muted),
                Err(error) => text(format!("This template can't be read: {error}")).color(c.danger),
            }
            .size(12),
            button(text("Show my templates folder").size(12).color(c.accent))
                .padding(0)
                .style(style::flat(c, false))
                .on_press(Message::ShowTemplatesFolder),
        ]
        .spacing(4),
    ]
    .spacing(16);

    if form.has_walkthrough() {
        content = content.push(
            column![
                checkbox(form.walkthrough)
                    .label("Start the walkthrough")
                    .on_toggle(Message::ToggleNewWalkthrough)
                    .style(style::check(c)),
                text("Questions about your world, one at a time, that make your first notes. Skip any, and come back to it from the Campaign menu.")
                    .size(12)
                    .color(c.text_muted),
            ]
            .spacing(4),
        );
    }

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

/// Says where an export went, and how it can be used.
fn exported_notice(path: &Path) -> String {
    let file = path.display();
    let in_folder = |folder: Option<PathBuf>| {
        folder.is_some_and(|folder| path.parent().is_some_and(|parent| parent == folder))
    };
    if in_folder(templates::templates_folder()) {
        format!("Saved {file}. New campaigns can start from it, and you can share the file.")
    } else if in_folder(templates::types_folder()) {
        format!(
            "Saved {file}. New folders in any campaign can start from it, and you can share the file."
        )
    } else {
        format!("Saved {file}. To use it yourself, Import it.")
    }
}

/// What a campaign template makes, for the "New campaign" dialog.
fn template_summary(structure: &templates::Structure) -> String {
    let top = structure.top_folders();
    if top.is_empty() {
        return "An empty campaign folder. Add folders and their fields as you go.".into();
    }
    let subfolders = structure
        .folders
        .iter()
        .filter(|(folder, _)| folder.contains('/'))
        .count();
    let mut summary = format!("Adds these folders: {}", top.join(", "));
    if subfolders > 0 {
        summary.push_str(&format!(
            ", with {subfolders} subfolder{}",
            if subfolders == 1 { "" } else { "s" }
        ));
    }
    summary.push_str(&match structure.typed() {
        0 => ". None of them have fields.".to_string(),
        typed if typed == structure.folders.len() => ". Each has its own note fields.".into(),
        typed => format!(". {typed} of them have note fields."),
    });
    if structure.walkthrough.is_some() {
        summary.push_str(" It has its own walkthrough questions.");
    }
    summary
}

/// The "Open campaign" dialog: the campaigns opened before, the one opened last first,
/// and a button to find another.
fn open_campaign_dialog<'a>(c: Colors, open: &'a OpenCampaign) -> Element<'a, Message> {
    let today = chrono::Local::now().date_naive();
    let rows: Vec<Element<'a, Message>> = open
        .shown()
        .map(|campaign| {
            let found = campaign.path.is_dir();
            let detail = if found {
                format!("Opened {}", campaign.when(today))
            } else {
                "Can't be found".into()
            };
            row![
                button(
                    column![
                        text(campaign.name()).size(15).color(if found {
                            c.text
                        } else {
                            c.text_muted
                        }),
                        text(campaign.path.display().to_string())
                            .size(12)
                            .color(c.text_muted),
                    ]
                    .spacing(2)
                    .width(Fill),
                )
                .width(Fill)
                .padding([6, 10])
                .style(style::flat(c, false))
                .on_press(Message::OpenKnown(campaign.path.clone())),
                text(detail)
                    .size(12)
                    .color(if found { c.text_muted } else { c.danger }),
                tooltip(
                    button(text("×").size(16))
                        .padding([2, 8])
                        .style(style::flat(c, false))
                        .on_press(Message::ForgetCampaign(campaign.path.clone())),
                    container(text("Take off this list (the folder stays)").size(12))
                        .padding(6)
                        .style(style::dropdown(c)),
                    tooltip::Position::Left,
                ),
            ]
            .spacing(8)
            .align_y(Center)
            .into()
        })
        .collect();

    let list: Element<'a, Message> = if rows.is_empty() {
        text("No campaign's name or folder has that in it.")
            .size(13)
            .color(c.text_muted)
            .into()
    } else {
        scrollable(column(rows).spacing(2)).height(Fill).into()
    };

    let content = column![
        text("Open campaign").size(22).color(c.text),
        text_input("Find a campaign", &open.filter)
            .id(OPEN_FILTER)
            .on_input(Message::OpenFilter)
            .on_submit(Message::OpenFirstCampaign)
            .padding(8)
            .style(style::input(c)),
        container(list).height(360),
        row![
            text("Campaigns you open or make show up here.")
                .size(12)
                .color(c.text_muted)
                .width(Fill),
            button("Cancel")
                .style(style::secondary(c))
                .on_press(Message::CancelOpenCampaign),
            button("Browse for a Folder...")
                .style(style::primary(c))
                .on_press(Message::BrowseCampaign),
        ]
        .spacing(8)
        .align_y(Center),
    ]
    .spacing(16);

    container(content)
        .width(600)
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

/// A layer that shows `dialog` centered over the dimmed window, blocking clicks to it.
fn modal(dialog: Element<'_, Message>) -> Element<'_, Message> {
    opaque(center(opaque(dialog)).style(style::backdrop))
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

/// Asks for a campaign template (`.zip`) or a saved type (`.yaml`) to import.
async fn pick_import() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Import a template or type")
        .add_filter("Templates and types", &["zip", "yaml", "yml"])
        .pick_file()
        .await
        .map(|handle| handle.path().to_path_buf())
}

/// Asks where to save an exported template or type, starting in `folder` with `name`.
/// Returns where it was saved, `None` if the save dialog was cancelled.
async fn save_export(
    folder: Option<PathBuf>,
    name: String,
    bytes: Vec<u8>,
) -> Result<Option<PathBuf>, String> {
    let (title, filter, extensions): (_, _, &[&str]) = if name.ends_with(".zip") {
        ("Export campaign structure", "Campaign template", &["zip"])
    } else {
        ("Export type", "Dungeon Elements type", &["yaml"])
    };
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title(title)
        .add_filter(filter, extensions)
        .set_file_name(&name);
    if let Some(folder) = folder {
        let _ = std::fs::create_dir_all(&folder);
        dialog = dialog.set_directory(folder);
    }
    let Some(handle) = dialog.save_file().await else {
        return Ok(None);
    };
    let path = handle.path().to_path_buf();
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    Ok(Some(path))
}

/// Asks for Markdown files to add to the walkthrough, like the players' characters.
async fn pick_imports() -> Vec<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Add from Markdown files")
        .add_filter("Markdown", &["md"])
        .pick_files()
        .await
        .map(|handles| {
            handles
                .iter()
                .map(|handle| handle.path().to_path_buf())
                .collect()
        })
        .unwrap_or_default()
}

async fn pick_picture() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Insert picture")
        .add_filter("Pictures", &preview::PICTURES)
        .pick_file()
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

/// Asks whether to delete something, described by `question`. Returns true for yes.
async fn confirm_delete(question: String) -> bool {
    rfd::AsyncMessageDialog::new()
        .set_title("Delete")
        .set_description(question)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        .await
        == rfd::MessageDialogResult::Yes
}

/// Where deleted files go, by the name the system gives it.
#[cfg(target_os = "windows")]
const TRASH: &str = "Recycle Bin";
#[cfg(not(target_os = "windows"))]
const TRASH: &str = "Trash";

/// The menu item that shows a file in the system's file manager.
#[cfg(target_os = "windows")]
const SHOW_IN_FILE_MANAGER: &str = "Show in Explorer";
#[cfg(target_os = "macos")]
const SHOW_IN_FILE_MANAGER: &str = "Show in Finder";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const SHOW_IN_FILE_MANAGER: &str = "Show in File Manager";

/// Opens `folder` in the system's file manager.
fn open_folder(folder: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(folder)
        .spawn()
        .map(|_| ())
}

/// Opens the system's file manager at `path`: in the folder, or selecting the file where
/// the file manager can.
fn show_in_file_manager(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // Explorer wants the quotes around the path only, not the whole argument.
        std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn()
            .map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let folder = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        std::process::Command::new("xdg-open")
            .arg(folder)
            .spawn()
            .map(|_| ())
    }
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
/// The note's `text` with the block at `range` replaced by `edited`. Text added at the
/// end (an empty range there) gets a blank line before it, and a block edited down to
/// nothing takes the blank lines around it with it.
fn replace_block(text: &str, range: std::ops::Range<usize>, edited: &str) -> String {
    let edited = edited.trim_matches(['\n', '\r']).trim_end();
    let (before, after) = (&text[..range.start], &text[range.end..]);

    if edited.trim().is_empty() || (range.is_empty() && after.is_empty()) {
        let before = before.trim_end();
        let after = after.trim_start().trim_end();
        let mut parts: Vec<&str> = [before, edited, after]
            .into_iter()
            .filter(|part| !part.trim().is_empty())
            .collect();
        if parts.is_empty() {
            parts.push("");
        }
        let mut joined = parts.join("\n\n");
        if !joined.is_empty() {
            joined.push('\n');
        }
        return joined;
    }

    format!("{before}{edited}{after}")
}

/// The formatting toolbar: a dropdown for what the line is (a paragraph, a heading...),
/// then buttons for bold, italic, strikethrough, lists, links and pictures. Each button's
/// tooltip shows the Markdown it writes, for those who want to learn to type it.
///
/// `types` are the text types to offer; `line` is what the line with the cursor is (its text type and list), to show them
/// picked, or `None` when no text box has the cursor.
fn format_bar<'a>(
    c: Colors,
    types: Vec<format::TextType>,
    line: Option<(Option<format::TextType>, Option<format::List>)>,
) -> Element<'a, Message> {
    use format::{Command, List};

    let tip = |content: Element<'a, Message>, tip: &'a str| -> Element<'a, Message> {
        tooltip(
            content,
            container(text(tip).size(12))
                .padding([2, 6])
                .style(style::dropdown(c)),
            tooltip::Position::Bottom,
        )
        .into()
    };
    let tool = |label: Element<'a, Message>, command: Message, picked: bool, hint: &'a str| {
        tip(
            button(container(label).center_y(20))
                .padding([2, 8])
                .style(style::flat(c, picked))
                .on_press(command)
                .into(),
            hint,
        )
    };
    let font = |weight, style| iced::Font {
        weight,
        style,
        ..iced::Font::DEFAULT
    };
    let letter = |letter: &'a str, font: iced::Font| -> Element<'a, Message> {
        text(letter).size(14).font(font).into()
    };
    let bar = || -> Element<'a, Message> {
        container(rule::vertical(1).style(style::separator(c)))
            .height(20)
            .padding([0, 4])
            .into()
    };
    // A small task box like the reading view's, beside the word.
    let task_box = container(space())
        .width(11)
        .height(11)
        .style(style::task_box(c, false));

    let (text_type, list) = line.unwrap_or_default();
    // What the line is. (No tooltip: it would cover the list of choices.)
    let types = pick_list(types, text_type, |picked| {
        Message::Format(Command::Text(picked))
    })
    .placeholder(if line.is_some() { "List item" } else { "Text" })
    .text_size(13)
    .padding([3, 8])
    .width(124)
    .style(style::choice(c))
    .menu_style(style::choice_menu(c));
    let format = |command| Message::Format(command);
    let normal = iced::font::Weight::Normal;
    let upright = iced::font::Style::Normal;

    let tools = row![
        types,
        bar(),
        tool(
            letter("B", font(iced::font::Weight::Bold, upright)),
            format(Command::Bold),
            false,
            "Bold (Ctrl+B): **text**",
        ),
        tool(
            letter("I", font(normal, iced::font::Style::Italic)),
            format(Command::Italic),
            false,
            "Italic (Ctrl+I): *text*",
        ),
        tool(
            iced::widget::rich_text([iced::widget::span::<(), _>("S").strikethrough(true)])
                .size(14)
                .into(),
            format(Command::Strikethrough),
            false,
            "Strikethrough: ~~text~~",
        ),
        bar(),
        tool(
            text("• List").size(13).into(),
            format(Command::List(List::Bullet)),
            list == Some(List::Bullet),
            "Bulleted list: - item",
        ),
        tool(
            text("1. List").size(13).into(),
            format(Command::List(List::Numbered)),
            list == Some(List::Numbered),
            "Numbered list: 1. item",
        ),
        tool(
            row![task_box, text("Tasks").size(13)]
                .spacing(5)
                .align_y(Center)
                .into(),
            format(Command::List(List::Task)),
            list == Some(List::Task),
            "Task list: - [ ] task (click a box in the reading view to tick it)",
        ),
        bar(),
        tool(
            text("Link").size(13).into(),
            Message::InsertLink,
            false,
            "Link to a note (Ctrl+K): [[Note]]",
        ),
        tool(
            text("Picture").size(13).into(),
            Message::InsertPicture,
            false,
            "Add a picture from your computer (Ctrl+Shift+K): ![[picture.png]]",
        ),
    ]
    .spacing(2)
    .align_y(Center);

    container(tools)
        .padding([4, 10])
        .width(Fill)
        .style(style::panel(c.sidebar))
        .into()
}

/// The editor for a block of the note in the reading view.
fn block_editor(
    c: Colors,
    content: &text_editor::Content,
    link_colors: links::Colors,
    accept: Option<Message>,
) -> Element<'_, Message> {
    let editor = text_editor(content)
        .id(BLOCK_EDITOR)
        .on_action(Message::BlockEdit)
        .key_binding(move |key_press| {
            if let Some(binding) = links::tab_binding(&key_press, &accept) {
                return Some(binding);
            }
            if matches!(key_press.key, Key::Named(keyboard::key::Named::Escape)) {
                return Some(text_editor::Binding::Custom(Message::FinishEditing));
            }
            // Every editor sees every key press, so only the focused one handles shortcuts.
            if (key_press.modifiers.command() || key_press.modifiers.alt())
                && matches!(key_press.status, text_editor::Status::Focused { .. })
                && let Some(message) = shortcut(&key_press.key, key_press.modifiers)
            {
                return Some(text_editor::Binding::Custom(message));
            }
            text_editor::Binding::from_key_press(key_press)
        })
        .size(15)
        .padding(8)
        .style(style::field_editor(c))
        .highlight_with::<links::Highlighter>(link_colors, links::format);

    context_menu::area(editor, editor_menu)
        .click_first(content.selection().is_none())
        .into()
}

/// The notes to link to while a link is being typed, as buttons, with a hint on picking
/// the first; or `None` if there is nothing to suggest.
///
/// `narrow` stacks the hint over the buttons, for under a block or field in the reading view.
fn suggestion_chips<'a>(
    c: Colors,
    suggestions: &Suggestions,
    narrow: bool,
) -> Option<Element<'a, Message>> {
    if suggestions.notes.is_empty() && suggestions.create.is_none() && suggestions.make.is_empty() {
        return None;
    }
    let chips = suggestions.notes.iter().map(|(link, label)| {
        button(text(label.clone()).size(13))
            .padding([3, 10])
            .style(style::secondary(c))
            .on_press(Message::CompleteLink(link.clone()))
            .into()
    });
    let create = suggestions.create.as_ref().map(|(index, label)| {
        button(text(label.clone()).size(13))
            .padding([3, 10])
            .style(style::secondary(c))
            .on_press(Message::Form(form::Message::FollowLink(*index)))
    });
    let hint = match (suggestions.notes.len(), suggestions.in_field) {
        (0, _) => "",
        (_, true) => "Enter picks the first",
        (_, false) => "Tab inserts the first",
    };
    let make = suggestions.make.iter().map(|(folder, name, label)| {
        button(text(label.clone()).size(13))
            .padding([3, 10])
            .style(style::secondary(c))
            .on_press(Message::Walkthrough(walkthrough_tab::Message::Make(
                folder.clone(),
                name.clone(),
            )))
            .into()
    });
    let chips = row(chips).push(create).extend(make).spacing(6).wrap();
    if narrow {
        let hint = match hint {
            "" => "Link to".to_string(),
            hint => format!("Link to ({hint})"),
        };
        return Some(
            column![text(hint).size(12).color(c.text_muted), chips]
                .spacing(6)
                .into(),
        );
    }
    Some(
        row![
            text("Link to").size(13).color(c.text_muted),
            chips,
            space::horizontal(),
            text(hint).size(12).color(c.text_muted),
        ]
        .spacing(10)
        .align_y(Center)
        .into(),
    )
}

/// Opens the right-click menu of the text editor right-clicked at `position`.
fn editor_menu(position: Point) -> Message {
    Message::OpenContextMenu(Target::Editor, position)
}

/// The input for the field with front matter `key` in the reading view, if it has one.
///
/// In a link field, Enter picks `accept`, the first note suggested, if there is one.
fn field_input<'a>(
    c: Colors,
    form: &'a Form,
    key: &str,
    accept: Option<Message>,
    known_tags: &'a form::KnownTags,
) -> Option<Element<'a, Message>> {
    let (index, field, value) = form.short_field(key)?;
    let accept = accept.filter(|_| field.kind.links());
    match value {
        form::FieldValue::Text(value) => Some(
            text_input(&field.label(), value)
                .id(FIELD_EDITOR)
                .on_input(move |value| Message::Form(form::Message::Text(index, value)))
                .on_submit(accept.unwrap_or(Message::FinishEditing))
                .size(15)
                .padding([4, 8])
                .style(style::input(c))
                .into(),
        ),
        form::FieldValue::Choice(selected) => Some(
            pick_list(
                field.options.clone(),
                selected.map(str::to_string),
                move |choice| Message::Form(form::Message::Choice(index, choice)),
            )
            .text_size(15)
            .style(style::choice(c))
            .menu_style(style::choice_menu(c))
            .into(),
        ),
        form::FieldValue::Dice => Some(form.dice_view(c, Message::Form, index, None)),
        form::FieldValue::List => Some(form.list_view(c, Message::Form, index, accept, known_tags)),
        form::FieldValue::Date => Some(form.date_view(c, Message::Form, index)),
        form::FieldValue::Check(_) => None,
    }
}

/// Opens `url` in the default web browser.
fn open_in_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");

    command.arg(url).spawn().map(|_| ())
}

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
    fn edited_blocks_go_back_into_the_note() {
        let text = "---\nname: Ann\n---\nOne.\n\nTwo.\n\nThree.\n";
        let two = text.find("Two.").unwrap();
        let range = two..two + 4;

        assert_eq!(
            replace_block(text, range.clone(), "Second\n\nparagraph.\n"),
            "---\nname: Ann\n---\nOne.\n\nSecond\n\nparagraph.\n\nThree.\n"
        );
        assert_eq!(
            replace_block(text, range, "  \n"),
            "---\nname: Ann\n---\nOne.\n\nThree.\n"
        );
        assert_eq!(
            replace_block(text, text.len()..text.len(), "Four."),
            "---\nname: Ann\n---\nOne.\n\nTwo.\n\nThree.\n\nFour.\n"
        );
        assert_eq!(replace_block("", 0..0, "Hi"), "Hi\n");
    }

    #[test]
    fn moved_notes_take_the_new_type_and_keep_their_data() {
        let lore = types::parse(
            "type: lore\nfields:\n  - key: name\n  - key: category\n    kind: choice\n    options: [Myth]\n",
        )
        .unwrap();
        let item = types::parse(types::TEST_ITEM).unwrap();
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
    fn renamed_notes_get_the_new_name() {
        let text = "---\ntype: npc\nname: Mira\nrole: Smuggler\n---\n\nTall.\n";
        assert_eq!(
            with_name(text, "Mira Vell", false).unwrap(),
            "---\ntype: npc\nname: Mira Vell\nrole: Smuggler\n---\n\nTall.\n"
        );
        // A plain note has no name field to change, unless its type wants one.
        assert_eq!(with_name("Just text.\n", "Old Song", false), None);
        assert_eq!(
            with_name("Just text.\n", "Old Song", true).unwrap(),
            "---\nname: Old Song\n---\n\nJust text.\n"
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
