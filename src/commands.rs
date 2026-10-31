//! Everything the menus can do, as named commands.
//!
//! The menu bar is built from these, and the keyboard shortcuts are looked up here, so an
//! action is named once. Each command has an id that stays the same between versions,
//! for a tool pane or custom shortcuts to point to later.

use iced::keyboard::{Key, Modifiers, key::Named};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    // File
    NewNote,
    NewFolder,
    OpenFile,
    Save,
    SaveAs,
    SaveAll,
    CloseTab,
    CloseCampaign,
    NewCampaign,
    OpenCampaign,
    Exit,
    // Edit
    Cut,
    Copy,
    Paste,
    SelectAll,
    InsertLink,
    InsertPicture,
    AddField,
    FindInNotes,
    // View
    ReadingView,
    EditAsForm,
    EditAsMarkdown,
    Quests,
    RollTables,
    DiceRoller,
    FormattingToolbar,
    ShowExtensions,
    ShowTypeFiles,
    OpenInReading,
    ReloadThemes,
    RefreshTree,
    ExpandAll,
    CollapseAll,
    // Campaign
    Walkthrough,
    WalkthroughQuestions,
    NoteTypes,
    ExportStructure,
    ShowCampaignFolder,
    // Tables
    NewRollTable,
    AddRollTableHere,
    // Help
    HowTo,
    Shortcuts,
    WhatsNew,
    ReportProblem,
    GitHub,
    About,
}

use Command::*;

impl Command {
    pub const ALL: [Command; 46] = [
        NewNote,
        NewFolder,
        OpenFile,
        Save,
        SaveAs,
        SaveAll,
        CloseTab,
        CloseCampaign,
        NewCampaign,
        OpenCampaign,
        Exit,
        Cut,
        Copy,
        Paste,
        SelectAll,
        InsertLink,
        InsertPicture,
        AddField,
        FindInNotes,
        ReadingView,
        EditAsForm,
        EditAsMarkdown,
        Quests,
        RollTables,
        DiceRoller,
        FormattingToolbar,
        ShowExtensions,
        ShowTypeFiles,
        OpenInReading,
        ReloadThemes,
        RefreshTree,
        ExpandAll,
        CollapseAll,
        Walkthrough,
        WalkthroughQuestions,
        NoteTypes,
        ExportStructure,
        ShowCampaignFolder,
        NewRollTable,
        AddRollTableHere,
        HowTo,
        Shortcuts,
        WhatsNew,
        ReportProblem,
        GitHub,
        About,
    ];

    /// The id that names the command in settings, which never changes once released.
    #[allow(dead_code)] // For the tool pane and custom shortcuts (Phase 3).
    pub fn id(self) -> &'static str {
        self.info().0
    }

    /// The command's name in the menus.
    pub fn label(self) -> &'static str {
        self.info().1
    }

    /// What the command does, in a few words, for hover text.
    #[allow(dead_code)] // For the tool pane's hover text (Phase 3).
    pub fn description(self) -> &'static str {
        self.info().2
    }

    /// The keys that run the command, as the menus show them.
    pub fn shortcut(self) -> Option<&'static str> {
        self.info().3
    }

    /// The command whose shortcut is `key` with `modifiers`, if any.
    ///
    /// Cut, Copy, Paste and Select All are left to the text boxes, which do them on
    /// their own; the menus only show their keys.
    pub fn from_key(key: &Key, modifiers: Modifiers) -> Option<Command> {
        Command::ALL.into_iter().find(|command| {
            !matches!(command, Cut | Copy | Paste | SelectAll)
                && command
                    .shortcut()
                    .is_some_and(|keys| matches_key(keys, key, modifiers))
        })
    }

    fn info(
        self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        Option<&'static str>,
    ) {
        match self {
            NewNote => ("new_note", "New Note", "Make a new note", Some("Ctrl+N")),
            NewFolder => ("new_folder", "New Folder...", "Make a new folder", None),
            OpenFile => (
                "open_file",
                "Open File...",
                "Open a file from your computer",
                Some("Ctrl+O"),
            ),
            Save => ("save", "Save", "Save the note", Some("Ctrl+S")),
            SaveAs => (
                "save_as",
                "Save As...",
                "Save the note somewhere else",
                Some("Ctrl+Shift+S"),
            ),
            SaveAll => (
                "save_all",
                "Save All",
                "Save every note with unsaved changes",
                Some("Ctrl+Alt+S"),
            ),
            CloseTab => ("close_tab", "Close Tab", "Close the tab", Some("Ctrl+W")),
            CloseCampaign => (
                "close_campaign",
                "Close Campaign",
                "Close the campaign and go back to the start screen",
                None,
            ),
            NewCampaign => (
                "new_campaign",
                "New Campaign...",
                "Start a new campaign",
                None,
            ),
            OpenCampaign => (
                "open_campaign",
                "Open Campaign...",
                "Open another campaign",
                None,
            ),
            Exit => ("exit", "Exit", "Quit Dungeon Elements", None),
            Cut => ("cut", "Cut", "Cut the selected text", Some("Ctrl+X")),
            Copy => ("copy", "Copy", "Copy the selected text", Some("Ctrl+C")),
            Paste => ("paste", "Paste", "Paste text", Some("Ctrl+V")),
            SelectAll => (
                "select_all",
                "Select All",
                "Select all the text in the box",
                Some("Ctrl+A"),
            ),
            InsertLink => (
                "insert_link",
                "Insert Link",
                "Insert a link, or turn the selection into one",
                Some("Ctrl+K"),
            ),
            InsertPicture => (
                "insert_picture",
                "Insert Picture...",
                "Insert a picture from your computer",
                Some("Ctrl+Shift+K"),
            ),
            AddField => (
                "add_field",
                "Add Field to This Note...",
                "Add a field to this note only",
                None,
            ),
            FindInNotes => (
                "find_in_notes",
                "Find in Notes",
                "Search your notes",
                Some("Ctrl+Shift+F"),
            ),
            ReadingView => (
                "reading_view",
                "Reading View",
                "Switch between reading and editing the note",
                Some("Ctrl+R"),
            ),
            EditAsForm => (
                "edit_as_form",
                "Edit as Form",
                "Edit the note's fields in a form",
                None,
            ),
            EditAsMarkdown => (
                "edit_as_markdown",
                "Edit as Markdown",
                "Edit the note's Markdown text",
                Some("Ctrl+E"),
            ),
            Quests => (
                "quests",
                "Quests",
                "Show the Quests list",
                Some("Ctrl+Shift+Q"),
            ),
            RollTables => (
                "roll_tables",
                "Roll Tables",
                "Show the Roll Tables list",
                Some("Ctrl+Shift+T"),
            ),
            DiceRoller => (
                "dice_roller",
                "Dice Roller",
                "Open or close the dice roller",
                Some("Ctrl+Shift+D"),
            ),
            FormattingToolbar => (
                "formatting_toolbar",
                "Formatting Toolbar",
                "Show the formatting buttons above the text",
                None,
            ),
            ShowExtensions => (
                "show_extensions",
                "Show File Extensions",
                "Show .md and other file endings in the file tree",
                None,
            ),
            ShowTypeFiles => (
                "show_type_files",
                "Show Type Files (.yaml)",
                "Show the files that hold the note types",
                None,
            ),
            OpenInReading => (
                "open_in_reading",
                "Open Notes in Reading View",
                "Open notes ready to read instead of to edit",
                None,
            ),
            ReloadThemes => (
                "reload_themes",
                "Reload Themes",
                "Load the theme files again",
                None,
            ),
            RefreshTree => (
                "refresh_tree",
                "Refresh",
                "Read the campaign folder again",
                None,
            ),
            ExpandAll => (
                "expand_all",
                "Expand All Folders",
                "Open every folder in the file tree",
                None,
            ),
            CollapseAll => (
                "collapse_all",
                "Collapse All Folders",
                "Close every folder in the file tree",
                None,
            ),
            Walkthrough => (
                "walkthrough",
                "Campaign Walkthrough...",
                "Build the campaign by answering questions",
                None,
            ),
            WalkthroughQuestions => (
                "walkthrough_questions",
                "Walkthrough Questions...",
                "Change the walkthrough's questions",
                None,
            ),
            NoteTypes => (
                "note_types",
                "Note Types...",
                "Change the fields each kind of note has",
                None,
            ),
            ExportStructure => (
                "export_structure",
                "Export Campaign Structure...",
                "Save the folders and types as a template",
                None,
            ),
            ShowCampaignFolder => (
                "show_campaign_folder",
                "Show Campaign Folder",
                "Open the campaign folder on your computer",
                None,
            ),
            NewRollTable => (
                "new_roll_table",
                "New Roll Table...",
                "Make a new roll table",
                None,
            ),
            AddRollTableHere => (
                "add_roll_table_here",
                "Add Roll Table Here",
                "Add a roll table to the end of this note",
                None,
            ),
            HowTo => (
                "how_to",
                "How-To Guides",
                "Open the how-to guides",
                Some("F1"),
            ),
            Shortcuts => (
                "shortcuts",
                "Keyboard Shortcuts",
                "Show every keyboard shortcut",
                Some("Ctrl+/"),
            ),
            WhatsNew => (
                "whats_new",
                "What's New",
                "See what changed in this version",
                None,
            ),
            ReportProblem => (
                "report_problem",
                "Report a Problem...",
                "Tell us about a problem on GitHub",
                None,
            ),
            GitHub => (
                "github",
                "Dungeon Elements on GitHub",
                "Open the project's page",
                None,
            ),
            About => (
                "about",
                "About Dungeon Elements",
                "The version, license and themes folder",
                None,
            ),
        }
    }
}

/// Whether `keys`, written like "Ctrl+Shift+T", is `key` pressed with `modifiers`.
fn matches_key(keys: &str, key: &Key, modifiers: Modifiers) -> bool {
    let mut parts: Vec<&str> = keys.split('+').collect();
    // "Ctrl++" would be a plus key; none of ours ends that way.
    let Some(last) = parts.pop() else {
        return false;
    };
    let wants = |name: &str| parts.contains(&name);
    if wants("Ctrl") != modifiers.command()
        || wants("Shift") != modifiers.shift()
        || wants("Alt") != modifiers.alt()
    {
        return false;
    }
    match key.as_ref() {
        Key::Character(c) => c.eq_ignore_ascii_case(last),
        Key::Named(Named::F1) => last == "F1",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: &str) -> Key {
        Key::Character(c.into())
    }

    #[test]
    fn ids_are_unique_and_every_command_is_listed() {
        let mut ids: Vec<&str> = Command::ALL.iter().map(|command| command.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Command::ALL.len());
    }

    #[test]
    fn shortcuts_are_unique() {
        let mut keys: Vec<&str> = Command::ALL.iter().filter_map(|c| c.shortcut()).collect();
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count);
    }

    #[test]
    fn keys_find_their_command() {
        let ctrl = Modifiers::CTRL;
        assert_eq!(Command::from_key(&key("r"), ctrl), Some(ReadingView));
        assert_eq!(
            Command::from_key(&key("D"), ctrl | Modifiers::SHIFT),
            Some(DiceRoller)
        );
        assert_eq!(Command::from_key(&key("s"), ctrl), Some(Save));
        assert_eq!(
            Command::from_key(&key("s"), ctrl | Modifiers::ALT),
            Some(SaveAll)
        );
        assert_eq!(Command::from_key(&key("n"), Modifiers::empty()), None);
        // Ctrl+D is kept for Duplicate.
        assert_eq!(Command::from_key(&key("d"), ctrl), None);
        // Text boxes do their own copying.
        assert_eq!(Command::from_key(&key("c"), ctrl), None);
    }
}
