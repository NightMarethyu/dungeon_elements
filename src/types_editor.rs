//! The Edit > Types dialog: the campaign's folders on the left, the selected folder's type on the right.
//!
//! A subfolder's type shows the fields it inherits from its parent folder's, which can be
//! changed or removed for the subfolder only, then the fields only it has.

use std::fs;
use std::path::{Path, PathBuf};

use iced::widget::{
    button, checkbox, column, container, pick_list, row, rule, scrollable, space, text, text_input,
};
use iced::{Center, Element, Fill};

use crate::style;
use crate::theme::Colors;
use crate::types::{self, ContentType, Field, Kind, Types, Width};

/// The widget id of the list of folders, scrolled to the end for a new type.
pub const LIST: &str = "types-list";
/// The widget id of the type's name, focused for a new type.
pub const NAME_INPUT: &str = "types-name";

#[derive(Debug, Clone)]
pub enum Message {
    Select(usize),
    /// Gives the selected folder a type, so its notes get fields.
    AddType,
    Label(String),
    Id(String),
    FieldLabel(usize, String),
    FieldKey(usize, String),
    FieldKind(usize, Kind),
    FieldWidth(usize, Width),
    FieldOptions(usize, String),
    FieldFolder(usize, LinksTo),
    /// The folder a tags field's tag notes go in, as typed.
    TagFolder(usize, String),
    /// Whether a date field picks its date from a calendar.
    FieldCalendar(usize, bool),
    /// The roll table a text field rolls on, as typed.
    FieldTable(usize, String),
    MoveUp(usize),
    MoveDown(usize),
    RemoveField(usize),
    AddField,
    /// Copies the inherited field with this key into the subfolder's own, to change it.
    ChangeInherited(String),
    /// Leaves the inherited field with this key out of the subfolder's notes.
    RemoveInherited(String),
    /// Brings back an inherited field that was removed.
    KeepInherited(String),
    /// Gives the selected folder, which has no type, a type saved from another campaign.
    UseSaved(SavedType),
    /// Saves the selected folder's type as a file, to use in other campaigns or share.
    ExportType,
    /// Saves the campaign's folders and their types (not its notes) as a template.
    ExportStructure,
    /// Adds a template or a type from a file to the ones every campaign can use.
    Import,
    /// Adds a new type, with a new folder in the campaign folder for its notes.
    AddNew,
    /// The name of the folder a new type's notes go in.
    FolderName(String),
    /// Leaves out the new type being added.
    Discard,
    Save,
    Cancel,
}

/// A type saved for any campaign to use, as offered for a folder with no type.
#[derive(Debug, Clone)]
pub struct SavedType(ContentType);

impl PartialEq for SavedType {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id && self.0.label == other.0.label
    }
}

impl std::fmt::Display for SavedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.label())
    }
}

/// The folder a link field links to, as picked in the dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct LinksTo(Option<String>);

impl std::fmt::Display for LinksTo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(folder) => f.write_str(folder),
            None => f.write_str("Any folder"),
        }
    }
}

pub struct Editor {
    /// The campaign folder, where new types' folders are made.
    root: PathBuf,
    folders: Vec<Folder>,
    selected: usize,
    /// The types saved for any campaign to use.
    saved: Vec<SavedType>,
    pub error: Option<String>,
    /// What an export or import did.
    pub notice: Option<String>,
}

struct Folder {
    path: PathBuf,
    /// The folder's path inside the campaign, like `npcs/villains`.
    name: String,
    /// The folder it is in, if that is in the list (folders in the campaign folder have none).
    parent: Option<usize>,
    state: State,
    /// Whether the type changed since the dialog opened.
    changed: bool,
    /// Whether the folder is made on saving, for a type added with "Add New". Its `name`
    /// is as typed, and its `path` is worked out from it then.
    new: bool,
}

enum State {
    /// The folder has no `_type.yaml`.
    Untyped,
    /// The `_type.yaml` can't be read; it has to be fixed by hand.
    Broken(String),
    Typed(Box<Draft>),
}

/// A type being edited. Text is kept as typed until it is saved.
struct Draft {
    /// The type as it was read, so settings the dialog doesn't show (like `icon`) are kept.
    base: ContentType,
    id: String,
    label: String,
    fields: Vec<FieldDraft>,
    /// The keys of the inherited fields the folder's notes don't have.
    removed: Vec<String>,
}

struct FieldDraft {
    label: String,
    /// Empty for a key made from the label when saved.
    key: String,
    kind: Kind,
    width: Width,
    /// A choice field's options, comma separated.
    options: String,
    /// A link field's folder, or the folder a tags field's tag notes go in.
    folder: Option<String>,
    /// Whether a date field picks its date from a calendar.
    calendar: bool,
    /// The roll table a text field rolls on, as typed.
    table: String,
}

impl FieldDraft {
    fn from(field: &Field) -> Self {
        FieldDraft {
            label: field.label(),
            key: field.key.clone(),
            kind: field.kind,
            width: field.width,
            options: field.options.join(", "),
            folder: field.folder.clone(),
            calendar: field.calendar != Some(false),
            table: field.table.clone().unwrap_or_default(),
        }
    }

    fn to_field(&self) -> Field {
        let key = match self.key.trim() {
            "" => types::key_from(&self.label),
            key => key.to_string(),
        };

        Field {
            key,
            label: Some(self.label.trim().to_string()).filter(|label| !label.is_empty()),
            kind: self.kind,
            options: if self.kind == Kind::Choice {
                self.options
                    .split(',')
                    .map(str::trim)
                    .filter(|option| !option.is_empty())
                    .map(String::from)
                    .collect()
            } else {
                Vec::new()
            },
            width: self.width,
            folder: self
                .folder
                .clone()
                .filter(|_| self.kind.links() || self.kind == Kind::Tags)
                .filter(|folder| !folder.trim().is_empty()),
            calendar: (self.kind == Kind::Date && !self.calendar).then_some(false),
            table: Some(self.table.trim().to_string())
                .filter(|table| !table.is_empty() && self.kind.rolls_tables()),
        }
    }
}

impl Draft {
    fn from(ty: &ContentType) -> Self {
        Draft {
            base: ty.clone(),
            id: ty.id.clone(),
            label: ty.label(),
            fields: ty.fields.iter().map(FieldDraft::from).collect(),
            removed: ty.removed_fields.clone(),
        }
    }

    fn to_type(&self) -> ContentType {
        ContentType {
            id: self.id.trim().to_string(),
            label: Some(self.label.trim().to_string()).filter(|label| !label.is_empty()),
            fields: self.fields.iter().map(FieldDraft::to_field).collect(),
            removed_fields: self.removed.clone(),
            ..self.base.clone()
        }
    }
}

impl Editor {
    /// Lists every folder in the campaign (parents first), with the type its own
    /// `_type.yaml` gives it, if it has one.
    pub fn new(root: &Path, folders: &[PathBuf], own: &Types) -> Self {
        let folders = folders
            .iter()
            .map(|path| Folder {
                name: path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/"),
                parent: folders
                    .iter()
                    .position(|parent| Some(parent.as_path()) == path.parent()),
                state: match own.get(path) {
                    None => State::Untyped,
                    Some(Err(error)) => State::Broken(error.clone()),
                    Some(Ok(ty)) => State::Typed(Box::new(Draft::from(ty))),
                },
                path: path.clone(),
                changed: false,
                new: false,
            })
            .collect();

        Editor {
            root: root.to_path_buf(),
            folders,
            selected: 0,
            saved: Vec::new(),
            error: None,
            notice: None,
        }
    }

    /// Offers `saved` to folders with no type.
    pub fn set_saved(&mut self, saved: Vec<ContentType>) {
        self.saved = saved.into_iter().map(SavedType).collect();
    }

    /// Whether any type changed since the dialog opened.
    pub fn has_changes(&self) -> bool {
        self.folders.iter().any(|folder| folder.changed)
    }

    /// The selected folder's type on its own, with what it inherits, to export: the file
    /// name to suggest and its YAML.
    pub fn export_type(&self) -> Result<(String, String), String> {
        let ty = self
            .effective(self.selected)
            .ok_or("This folder has no type to export")?;
        Ok((
            crate::templates::type_file_name(&ty),
            crate::templates::standalone(&ty)?,
        ))
    }

    /// The type of the folder at `index` as it is now in the dialog, with what it inherits.
    fn effective(&self, index: usize) -> Option<ContentType> {
        let folder = self.folders.get(index)?;
        let parent = folder.parent.and_then(|parent| self.effective(parent));
        match (&folder.state, parent) {
            (State::Typed(draft), Some(parent)) => Some(types::inherit(&parent, &draft.to_type())),
            (State::Typed(draft), None) => Some(draft.to_type()),
            (State::Untyped, parent) => parent,
            (State::Broken(_), _) => None,
        }
    }

    /// The type the folder at `index` inherits from the folder it is in.
    fn inherited(&self, index: usize) -> Option<ContentType> {
        self.folders
            .get(index)?
            .parent
            .and_then(|parent| self.effective(parent))
    }

    /// Shows `folder` first, if it is in the list.
    pub fn select(&mut self, folder: &Path) {
        if let Some(index) = self.folders.iter().position(|f| f.path == folder) {
            self.selected = index;
        }
    }

    pub fn update(&mut self, message: Message) {
        self.error = None;
        self.notice = None;

        if let Message::AddNew = message {
            // It starts with only a name field, and gets its name, id and folder next.
            let ty = types::parse("type: new\nfields:\n  - key: name\n    label: Name\n")
                .expect("a type with only a name field is valid");
            let mut draft = Draft::from(&ty);
            draft.id = String::new();
            draft.label = String::new();
            self.folders.push(Folder {
                path: self.root.clone(),
                name: String::new(),
                parent: None,
                state: State::Typed(Box::new(draft)),
                changed: true,
                new: true,
            });
            self.selected = self.folders.len() - 1;
            return;
        }

        if let Message::Discard = message {
            if self.folders.get(self.selected).is_some_and(|f| f.new) {
                self.folders.remove(self.selected);
                self.selected = self.selected.saturating_sub(1);
            }
            return;
        }

        if let Message::Select(index) = message {
            if index < self.folders.len() {
                self.selected = index;
            }
            return;
        }

        let inherited = self.inherited(self.selected);
        let Some(folder) = self.folders.get_mut(self.selected) else {
            return;
        };

        if let Message::UseSaved(SavedType(ty)) = message {
            if let State::Untyped = folder.state {
                folder.state = State::Typed(Box::new(Draft::from(&ty)));
                folder.changed = true;
            }
            return;
        }

        if let Message::AddType = message {
            if let State::Untyped = folder.state {
                let ty = match &inherited {
                    // A kind of its parent's type, named for one of its notes ("Weapon"),
                    // which has the parent's fields until some are added.
                    Some(_) => {
                        let last = folder.name.rsplit('/').next().unwrap_or(&folder.name);
                        let label = types::singular(last);
                        ContentType {
                            id: types::key_from(&label),
                            label: Some(label),
                            icon: None,
                            fields: Vec::new(),
                            removed_fields: Vec::new(),
                            parents: Vec::new(),
                        }
                    }
                    None => {
                        let id = types::key_from(&folder.name.replace('/', " "));
                        types::parse(&format!(
                            "type: {}\nfields:\n  - key: name\n    label: Name\n",
                            serde_yaml_ng::to_string(&id).unwrap_or_default().trim()
                        ))
                        .expect("a type with only a name field is valid")
                    }
                };
                folder.state = State::Typed(Box::new(Draft::from(&ty)));
                folder.changed = true;
            }
            return;
        }

        let State::Typed(draft) = &mut folder.state else {
            return;
        };
        folder.changed = true;

        let fields = &mut draft.fields;
        match message {
            // A new type's id and folder follow its name ("Faction": `faction`, factions)
            // until they are changed by hand.
            Message::Label(value) if folder.new => {
                if draft.id == types::key_from(&draft.label) {
                    draft.id = types::key_from(&value);
                }
                if folder.name == plural(&draft.label) {
                    folder.name = plural(&value);
                }
                draft.label = value;
            }
            Message::FolderName(value) if folder.new => folder.name = value,
            Message::Label(value) => draft.label = value,
            Message::Id(value) => draft.id = value,
            Message::FieldLabel(index, value) => {
                if let Some(field) = fields.get_mut(index) {
                    field.label = value;
                }
            }
            Message::FieldKey(index, value) => {
                if let Some(field) = fields.get_mut(index) {
                    field.key = value;
                }
            }
            Message::FieldKind(index, value) => {
                if let Some(field) = fields.get_mut(index) {
                    field.kind = value;
                }
            }
            Message::FieldWidth(index, value) => {
                if let Some(field) = fields.get_mut(index) {
                    field.width = value;
                }
            }
            Message::FieldOptions(index, value) => {
                if let Some(field) = fields.get_mut(index) {
                    field.options = value;
                }
            }
            Message::FieldFolder(index, LinksTo(value)) => {
                if let Some(field) = fields.get_mut(index) {
                    field.folder = value;
                }
            }
            Message::TagFolder(index, value) => {
                if let Some(field) = fields.get_mut(index) {
                    field.folder = Some(value);
                }
            }
            Message::FieldCalendar(index, value) => {
                if let Some(field) = fields.get_mut(index) {
                    field.calendar = value;
                }
            }
            Message::FieldTable(index, value) => {
                if let Some(field) = fields.get_mut(index) {
                    field.table = value;
                }
            }
            Message::MoveUp(index) => {
                if index > 0 && index < fields.len() {
                    fields.swap(index, index - 1);
                }
            }
            Message::MoveDown(index) => {
                if index + 1 < fields.len() {
                    fields.swap(index, index + 1);
                }
            }
            Message::RemoveField(index) => {
                if index < fields.len() {
                    fields.remove(index);
                }
            }
            Message::AddField => fields.push(FieldDraft::from(&Field::for_note("", Kind::Text))),
            Message::ChangeInherited(key) => {
                let field = inherited
                    .iter()
                    .flat_map(|ty| &ty.fields)
                    .find(|field| field.key == key);
                if let Some(field) = field
                    && fields.iter().all(|own| own.key != key)
                {
                    fields.push(FieldDraft::from(field));
                }
                draft.removed.retain(|removed| *removed != key);
            }
            Message::RemoveInherited(key) => {
                if key != "name" && !draft.removed.contains(&key) {
                    draft.removed.push(key);
                }
            }
            Message::KeepInherited(key) => draft.removed.retain(|removed| *removed != key),
            Message::Select(_)
            | Message::AddType
            | Message::UseSaved(_)
            | Message::ExportType
            | Message::ExportStructure
            | Message::Import
            | Message::AddNew
            | Message::FolderName(_)
            | Message::Discard
            | Message::Save
            | Message::Cancel => {}
        }
    }

    /// Checks every changed type, then writes their `_type.yaml` files.
    ///
    /// Returns the folders that were written. Nothing is written if any type has a problem.
    pub fn save(&mut self) -> Result<Vec<PathBuf>, String> {
        let mut files = Vec::new();

        for (index, folder) in self.folders.iter().enumerate() {
            let (true, State::Typed(draft)) = (folder.changed, &folder.state) else {
                continue;
            };

            let ty = draft.to_type();
            // The name can come from the type it is a kind of.
            let all = self.effective(index).unwrap_or_else(|| ty.clone());
            let problem = if ty.fields.iter().any(|field| field.key.is_empty()) {
                Err("every field needs a name".to_string())
            } else if !all.fields.iter().any(|field| field.key == "name") {
                Err("it needs a field with the key \"name\": it is the note's filename".to_string())
            } else if all.parents.contains(&ty.id) {
                Err(format!(
                    "its id `{}` is the id of the type it is a kind of; give it one of its own",
                    ty.id
                ))
            } else {
                ty.to_yaml()
            };

            // A new type's folder must be one that can be made.
            let problem = match folder.new {
                false => problem,
                true => {
                    let name = folder.name.trim();
                    let taken = self.folders.iter().enumerate().any(|(other, f)| {
                        other != index && f.name.trim().eq_ignore_ascii_case(name)
                    });
                    if name.is_empty() {
                        Err("give its notes a folder".to_string())
                    } else if let Err(error) = crate::campaign::validate_name(name) {
                        Err(format!("its folder: {error}"))
                    } else if taken || self.root.join(name).exists() {
                        Err(format!("there is already a folder called {name}"))
                    } else {
                        problem
                    }
                }
            };

            let path = match folder.new {
                true => self.root.join(folder.name.trim()),
                false => folder.path.clone(),
            };
            match problem {
                Ok(yaml) => files.push((path, yaml)),
                Err(error) => {
                    self.selected = index;
                    let name = match folder.name.trim() {
                        "" => "The new type",
                        name => name,
                    };
                    return Err(format!("{name}: {error}"));
                }
            }
        }

        for (folder, yaml) in &files {
            fs::create_dir_all(folder)
                .map_err(|error| format!("Could not make {}: {error}", folder.display()))?;
            fs::write(folder.join(types::TYPE_FILE), yaml)
                .map_err(|error| format!("Could not save {}: {error}", folder.display()))?;
        }

        Ok(files.into_iter().map(|(folder, _)| folder).collect())
    }

    pub fn view(&self, c: Colors) -> Element<'_, Message> {
        let list = column(self.folders.iter().enumerate().map(|(index, folder)| {
            let inherited = self.inherited(index);
            let detail = match (&folder.state, &inherited) {
                (State::Untyped, None) => "no type".to_string(),
                (State::Untyped, Some(parent)) => {
                    format!("{}, like the folder it's in", parent.label())
                }
                (State::Broken(_), _) => "can't be read".to_string(),
                (State::Typed(draft), None) => draft.label.clone(),
                (State::Typed(draft), Some(parent)) => {
                    format!("{}, a kind of {}", draft.label, parent.label())
                }
            };
            let changed = if folder.changed { " •" } else { "" };
            // Subfolders are indented under the folder they are in, and named by their own name.
            let depth = folder.name.matches('/').count();
            let name = match folder.name.rsplit('/').next().unwrap_or(&folder.name) {
                "" if folder.new => "New type",
                name => name,
            };
            let detail = match (folder.new, detail.is_empty()) {
                (true, true) => "new folder".to_string(),
                (true, false) => format!("{detail}, new folder"),
                _ => detail,
            };

            button(row![
                space().width(depth as f32 * 14.0),
                column![
                    text(format!("{name}{changed}")).size(14),
                    text(detail).size(12).color(c.text_muted),
                ]
                .spacing(2),
            ])
            .width(Fill)
            .padding([6, 10])
            .style(style::flat(c, index == self.selected))
            .on_press(Message::Select(index))
            .into()
        }));

        let add = button(text("Add New").size(14))
            .width(Fill)
            .padding([6, 10])
            .style(style::secondary(c))
            .on_press(Message::AddNew);
        let list: Element<'_, Message> = if self.folders.is_empty() {
            column![
                text("This campaign has no folders yet.")
                    .size(13)
                    .color(c.text_muted),
                add,
            ]
            .spacing(8)
            .into()
        } else {
            scrollable(list.push(space().height(8)).push(add))
                .id(LIST)
                .height(Fill)
                .into()
        };

        let detail = match self.folders.get(self.selected) {
            None => space().into(),
            Some(folder) => self.folder_view(c, folder, self.inherited(self.selected)),
        };

        let changed = self.has_changes();
        let message = if let Some(error) = &self.error {
            text(error).size(13).color(c.danger)
        } else if let Some(notice) = &self.notice {
            text(notice).size(13).color(c.text)
        } else if changed {
            text("Changes apply to every note in the folder. Data in removed fields stays in the notes, as each note's own field. Save the changes to export them.")
                .size(12)
                .color(c.text_muted)
        } else {
            text("Changes apply to every note in the folder. Export a type, or the whole campaign's folders and types, to use them in other campaigns or share them.")
                .size(12)
                .color(c.text_muted)
        };

        let can_export_type = !changed && self.effective(self.selected).is_some();
        let mut footer = row![
            button("Import...")
                .style(style::secondary(c))
                .on_press(Message::Import),
            button("Export Type...")
                .style(style::secondary(c))
                .on_press_maybe(can_export_type.then_some(Message::ExportType)),
            button("Export Campaign Structure...")
                .style(style::secondary(c))
                .on_press_maybe((!changed).then_some(Message::ExportStructure)),
            space::horizontal(),
        ]
        .spacing(8)
        .align_y(Center);
        footer = footer.push(
            button("Cancel")
                .style(style::secondary(c))
                .on_press(Message::Cancel),
        );
        footer = footer.push(
            button("Save")
                .style(style::primary(c))
                .on_press_maybe(changed.then_some(Message::Save)),
        );

        container(
            column![
                text("Types").size(22).color(c.text),
                row![
                    container(list).width(200).height(Fill),
                    rule::vertical(1).style(style::separator(c)),
                    container(detail).width(Fill).height(Fill),
                ]
                .spacing(12)
                .height(Fill),
                message.width(Fill),
                footer,
            ]
            .spacing(16),
        )
        .width(1040)
        .height(600)
        .padding(24)
        .style(style::dialog(c))
        .into()
    }

    /// A list of the saved types to give a folder with no type, if there are any.
    fn saved_picker<'a>(&'a self, c: Colors, label: &'a str) -> Element<'a, Message> {
        if self.saved.is_empty() {
            return space().into();
        }
        column![
            text(label).size(13).color(c.text_muted),
            pick_list(self.saved.as_slice(), None::<SavedType>, Message::UseSaved)
                .placeholder("Pick a saved type")
                .padding(6)
                .style(style::choice(c))
                .menu_style(style::choice_menu(c)),
        ]
        .spacing(6)
        .into()
    }

    fn folder_view<'a>(
        &'a self,
        c: Colors,
        folder: &'a Folder,
        inherited: Option<ContentType>,
    ) -> Element<'a, Message> {
        let label = |label| text(label).size(12).color(c.text_muted);
        let input = |placeholder: &str, value: &str| {
            text_input(placeholder, value)
                .size(14)
                .padding(6)
                .style(style::input(c))
        };

        let draft = match &folder.state {
            State::Untyped if let Some(parent) = &inherited => {
                let fields = parent
                    .fields
                    .iter()
                    .map(|field| format!("{} ({})", field.label(), field.kind))
                    .collect::<Vec<_>>()
                    .join(", ");
                return column![
                    text(format!(
                        "Notes in {} are {} notes, like the folder it's in.",
                        folder.name,
                        parent.label()
                    ))
                    .size(14)
                    .color(c.text),
                    text(format!("Their fields: {fields}."))
                        .size(13)
                        .color(c.text_muted),
                    text(format!(
                        "Make it a kind of {0} to give its notes fields of their own as well, like Weapons in Items: every {0} field, and the ones you add.",
                        parent.label()
                    ))
                    .size(13)
                    .color(c.text_muted),
                    button(text(format!("Make it a kind of {}", parent.label())))
                        .style(style::primary(c))
                        .on_press(Message::AddType),
                    self.saved_picker(c, "Or make it a kind of a saved type:"),
                ]
                .spacing(12)
                .into();
            }
            State::Untyped => {
                return column![
                    text(format!("Notes in {} have no fields.", folder.name))
                        .size(14)
                        .color(c.text),
                    text("Give the folder a type to open its notes as a form.")
                        .size(13)
                        .color(c.text_muted),
                    button("Give this folder a type")
                        .style(style::primary(c))
                        .on_press(Message::AddType),
                    self.saved_picker(c, "Or start from a type saved for every campaign:"),
                ]
                .spacing(12)
                .into();
            }
            State::Broken(error) => {
                return column![
                    text(format!(
                        "{}/{} can't be read:",
                        folder.name,
                        types::TYPE_FILE
                    ))
                    .size(14)
                    .color(c.text),
                    text(error).size(13).color(c.danger),
                    text("Turn on View > Show Type Files to open and fix it.")
                        .size(13)
                        .color(c.text_muted),
                ]
                .spacing(12)
                .into();
            }
            State::Typed(draft) => draft,
        };

        // A new type's notes go in a new folder, named here.
        let new_folder = folder.new.then(|| {
            row![
                column![
                    label("Folder for its notes (made in the campaign folder)"),
                    input("factions", &folder.name).on_input(Message::FolderName),
                ]
                .spacing(4)
                .width(Fill),
                button(text("Discard").size(13))
                    .padding([6, 10])
                    .style(style::secondary(c))
                    .on_press(Message::Discard),
            ]
            .spacing(12)
            .align_y(iced::Bottom)
        });

        let header = row![
            column![
                label("Type name"),
                input("Item", &draft.label)
                    .id(NAME_INPUT)
                    .on_input(Message::Label)
            ]
            .spacing(4)
            .width(Fill),
            column![
                label("Id (stored in each note as type:)"),
                input("item", &draft.id).on_input(Message::Id)
            ]
            .spacing(4)
            .width(Fill),
        ]
        .spacing(12);

        let header = column![header].push(new_folder).spacing(12);

        let small = |label: &'static str, message: Option<Message>| {
            button(text(label).size(13))
                .padding([4, 8])
                .style(style::flat(c, false))
                .on_press_maybe(message)
        };

        let count = draft.fields.len();
        let fields = draft.fields.iter().enumerate().map(|(index, field)| {
            let controls = row![
                input("Field name", &field.label)
                    .on_input(move |value| Message::FieldLabel(index, value))
                    .width(Fill),
                input("from the name", &field.key)
                    .on_input(move |value| Message::FieldKey(index, value))
                    .width(120),
                pick_list(Kind::ALL, Some(field.kind), move |kind| {
                    Message::FieldKind(index, kind)
                })
                .text_size(14)
                .padding(6)
                .width(140)
                .style(style::choice(c))
                .menu_style(style::choice_menu(c)),
                pick_list(Width::ALL, Some(field.width), move |width| {
                    Message::FieldWidth(index, width)
                })
                .text_size(14)
                .padding(6)
                .width(110)
                .style(style::choice(c))
                .menu_style(style::choice_menu(c)),
                small("↑", (index > 0).then_some(Message::MoveUp(index))),
                small("↓", (index + 1 < count).then_some(Message::MoveDown(index))),
                // The name is the note's filename, so every type keeps it (a subfolder's
                // own change to it can go, since the inherited one comes back).
                small(
                    "Remove",
                    (field.key != "name" || inherited.is_some())
                        .then_some(Message::RemoveField(index))
                ),
            ]
            .spacing(6)
            .align_y(Center);

            let mut block = column![controls].spacing(6);
            if let Some(parent) = &inherited
                && parent.fields.iter().any(|inherited| inherited.key == field.key)
            {
                block = block.push(
                    text(format!(
                        "Changes {}'s field for these notes; it stays where {} has it. Remove it to use {}'s again.",
                        parent.label(),
                        parent.label(),
                        parent.label()
                    ))
                    .size(12)
                    .color(c.text_muted),
                );
            }
            if field.kind == Kind::Choice {
                block = block.push(
                    input("Options, separated by commas", &field.options)
                        .on_input(move |value| Message::FieldOptions(index, value)),
                );
            }
            if field.kind == Kind::Date {
                block = block.push(
                    row![
                        checkbox(field.calendar)
                            .label("Pick the date from a calendar")
                            .on_toggle(move |on| Message::FieldCalendar(index, on))
                            .text_size(13)
                            .style(style::check(c)),
                        text(if field.calendar {
                            "Saved as 2026-10-28, which Obsidian reads as a date. New notes start with today's."
                        } else {
                            "A line of text instead, for dates that aren't on a calendar, like a Stardate."
                        })
                        .size(12)
                        .color(c.text_muted),
                    ]
                    .spacing(12)
                    .align_y(Center),
                );
            }
            if field.kind.rolls_tables() {
                block = block.push(
                    row![
                        text("Rolls on").size(13).color(c.text_muted),
                        input("Roll tables, like Female names, Male names", &field.table)
                            .on_input(move |value| Message::FieldTable(index, value))
                            .width(220),
                        text("Gives the field a die button that rolls on the table and fills the field in.")
                            .size(12)
                            .color(c.text_muted),
                    ]
                    .spacing(8)
                    .align_y(Center),
                );
            }
            if field.kind == Kind::Tags && field.key != crate::form::ALIASES_KEY {
                block = block.push(
                    row![
                        text("Tag notes in").size(13).color(c.text_muted),
                        input(types::TAG_FOLDER, field.folder.as_deref().unwrap_or_default())
                            .on_input(move |value| Message::TagFolder(index, value))
                            .width(220),
                        text("Each tag gets a note there, listing links to the notes tagged with it.")
                            .size(12)
                            .color(c.text_muted),
                    ]
                    .spacing(8)
                    .align_y(Center),
                );
            }
            if field.kind.links() {
                let mut choices = vec![LinksTo(None)];
                choices.extend(self.folders.iter().map(|f| LinksTo(Some(f.name.clone()))));
                block = block.push(
                    row![
                        text("Links to").size(13).color(c.text_muted),
                        pick_list(choices, Some(LinksTo(field.folder.clone())), move |to| {
                            Message::FieldFolder(index, to)
                        })
                        .text_size(14)
                        .padding(6)
                        .width(220)
                        .style(style::choice(c))
                        .menu_style(style::choice_menu(c)),
                        text("Its notes are suggested first, and new notes the field names are made there.")
                            .size(12)
                            .color(c.text_muted),
                    ]
                    .spacing(8)
                    .align_y(Center),
                );
            }
            block.into()
        });

        let header_row = row![
            label("Field name").width(Fill),
            label("Key").width(120),
            label("Kind").width(140),
            label("Width").width(110),
            space().width(130),
        ]
        .spacing(6);

        let mut body = column![header].spacing(16);

        // A subfolder's type lists what it inherits first, as its notes show it.
        if let Some(parent) = &inherited {
            let rows = parent.fields.iter().map(|field| {
                let changed = draft.fields.iter().any(|own| own.key == field.key);
                let removed = draft.removed.contains(&field.key);
                let detail = if changed {
                    "changed below".to_string()
                } else if removed {
                    "removed from these notes".to_string()
                } else {
                    field.kind.to_string()
                };
                let key = field.key.clone();
                let mut controls = row![
                    text(field.label())
                        .size(14)
                        .color(if removed { c.text_muted } else { c.text })
                        .width(Fill),
                    text(detail).size(13).color(c.text_muted).width(200),
                ]
                .spacing(6)
                .align_y(Center);
                if removed {
                    controls =
                        controls.push(small("Bring back", Some(Message::KeepInherited(key))));
                } else if !changed {
                    controls =
                        controls.push(small("Change", Some(Message::ChangeInherited(key.clone()))));
                    controls = controls.push(small(
                        "Remove",
                        (field.key != "name").then_some(Message::RemoveInherited(key)),
                    ));
                }
                Element::from(controls)
            });
            body = body.push(
                column![
                    text(format!(
                        "From {} (the folder it's in). Changes there show here too.",
                        parent.label()
                    ))
                    .size(13)
                    .color(c.text),
                    column(rows).spacing(4),
                ]
                .spacing(6),
            );
            body = body.push(
                text(format!("Fields only {} notes have", draft.label))
                    .size(13)
                    .color(c.text),
            );
        }

        scrollable(
            body.push(column![header_row, column(fields).spacing(8)].spacing(6))
                .push(
                    button("Add field")
                        .style(style::secondary(c))
                        .on_press(Message::AddField),
                )
                .padding([0, 12]),
        )
        .height(Fill)
        .into()
    }
}

/// The folder a new type's notes go in, from its name: "Faction" makes `factions`.
fn plural(label: &str) -> String {
    let key = types::key_from(label);
    if key.is_empty() {
        return key;
    }
    let consonant_y =
        key.ends_with('y') && !key[..key.len() - 1].ends_with(['a', 'e', 'i', 'o', 'u']);
    if consonant_y {
        format!("{}ies", &key[..key.len() - 1])
    } else if ["s", "x", "ch", "sh"].iter().any(|end| key.ends_with(end)) {
        format!("{key}es")
    } else {
        format!("{key}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_a_new_type_with_its_folder() {
        let (root, mut dialog) = editor(types::TEST_ITEM);
        dialog.update(Message::AddNew);
        assert!(dialog.has_changes());
        // Nothing is made until it has a name.
        assert!(dialog.save().unwrap_err().starts_with("The new type"));

        dialog.update(Message::Label("Deity".into()));
        dialog.update(Message::AddField);
        dialog.update(Message::FieldLabel(1, "Domain".into()));
        let saved = dialog.save().unwrap();
        assert_eq!(saved, [root.join("deities")]);
        let ty = types::load(&root.join("deities")).unwrap().unwrap();
        assert_eq!((ty.id.as_str(), ty.fields.len()), ("deity", 2));

        // A folder that's there already can't be a new type's.
        let (other, mut dialog) = editor(types::TEST_ITEM);
        dialog.update(Message::AddNew);
        dialog.update(Message::Label("Item".into()));
        dialog.update(Message::FolderName("items".into()));
        assert!(dialog.save().unwrap_err().contains("already a folder"));
        dialog.update(Message::Discard);
        assert!(!dialog.has_changes());

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(other).unwrap();
    }

    #[test]
    fn names_folders_for_new_types() {
        assert_eq!(plural("Faction"), "factions");
        assert_eq!(plural("Deity"), "deities");
        assert_eq!(plural("Key"), "keys");
        assert_eq!(plural("Boss"), "bosses");
        assert_eq!(plural("Magic Item"), "magic_items");
    }

    fn editor(yaml: &str) -> (PathBuf, Editor) {
        // Each test gets its own folder, since tests run at the same time.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "dungeon_elements_types_editor_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        let items = root.join("items");
        fs::create_dir_all(&items).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();

        let mut types = Types::new();
        types.insert(items.clone(), types::parse(yaml));
        let editor = Editor::new(&root, &[items, root.join("notes")], &types);
        (root, editor)
    }

    #[test]
    fn exports_types_and_uses_saved_ones() {
        let (root, mut editor) = editor(types::TEST_ITEM);
        let (name, yaml) = editor.export_type().unwrap();
        assert_eq!(name, "Item.type.yaml");
        assert_eq!(types::parse(&yaml).unwrap().id, "item");

        // Notes has no type, so it has nothing to export, and can start from a saved one.
        editor.update(Message::Select(1));
        assert!(editor.export_type().is_err());
        editor.set_saved(vec![types::parse(types::starter_yaml("npcs")).unwrap()]);
        let saved = editor.saved[0].clone();
        editor.update(Message::UseSaved(saved));
        assert!(editor.has_changes());
        editor.save().unwrap();
        assert_eq!(types::load(&root.join("notes")).unwrap().unwrap().id, "npc");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn edits_and_saves_a_type() {
        let (root, mut editor) = editor(types::TEST_ITEM);

        editor.update(Message::RemoveField(1)); // Value
        editor.update(Message::AddField);
        let last = 5;
        editor.update(Message::FieldLabel(last, "Hit Points".into()));
        editor.update(Message::FieldKind(last, Kind::Number));
        editor.update(Message::MoveUp(last));
        editor.update(Message::FieldOptions(1, "Common, Rare,".into()));
        editor.update(Message::FieldFolder(2, LinksTo(Some("npcs".into()))));

        assert_eq!(editor.save().unwrap(), [root.join("items")]);

        let ty = types::load(&root.join("items")).unwrap().unwrap();
        let keys: Vec<_> = ty.fields.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(
            keys,
            ["name", "rarity", "owner", "history", "hit_points", "stats"]
        );
        assert_eq!(ty.fields[1].options, ["Common", "Rare"]);
        assert_eq!(ty.fields[4].kind, Kind::Number);
        assert_eq!(ty.fields[2].folder.as_deref(), Some("npcs"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_bad_types_and_writes_nothing() {
        let (root, mut editor) = editor("type: item\nfields:\n  - key: name\n");

        editor.update(Message::AddField);
        assert!(editor.save().is_err(), "a field with no name");

        editor.update(Message::FieldLabel(1, "Name".into()));
        assert!(editor.save().is_err(), "two fields with the key name");

        editor.update(Message::RemoveField(1));
        editor.update(Message::RemoveField(0));
        assert!(editor.save().is_err(), "no name field");
        assert!(!root.join("items").join(types::TYPE_FILE).exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn makes_a_subfolder_a_kind_of_its_parent() {
        let (root, _) = editor(types::TEST_ITEM);
        let items = root.join("items");
        let weapons = items.join("weapons");
        fs::create_dir_all(&weapons).unwrap();
        let mut own = Types::new();
        own.insert(items.clone(), types::parse(types::TEST_ITEM));
        let mut editor = Editor::new(&root, &[items, weapons.clone()], &own);

        editor.update(Message::Select(1));
        assert_eq!(editor.inherited(1).unwrap().id, "item");
        assert_eq!(
            editor.effective(1).unwrap().fields.len(),
            6,
            "Items' fields"
        );

        editor.update(Message::AddType);
        editor.update(Message::AddField);
        editor.update(Message::FieldLabel(0, "Damage".into()));
        editor.update(Message::RemoveInherited("owner".into()));
        editor.update(Message::RemoveInherited("name".into()));
        editor.update(Message::ChangeInherited("rarity".into()));
        editor.update(Message::FieldOptions(1, "Common, Magic".into()));
        assert_eq!(editor.save().unwrap(), std::slice::from_ref(&weapons));

        let own = types::load(&weapons).unwrap().unwrap();
        assert_eq!(own.id, "weapon");
        assert_eq!(own.label(), "Weapon");
        assert_eq!(own.removed_fields, ["owner"]);
        let keys: Vec<_> = own.fields.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(
            keys,
            ["damage", "rarity"],
            "only what it adds or changes is saved"
        );
        assert_eq!(own.fields[1].options, ["Common", "Magic"]);

        // Its own id can't be its parent's.
        editor.update(Message::Id("item".into()));
        assert!(editor.save().is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn gives_a_folder_a_type() {
        let (root, mut editor) = editor("type: item\nfields:\n  - key: name\n");

        editor.update(Message::Select(1));
        editor.update(Message::AddType);
        assert_eq!(editor.save().unwrap(), [root.join("notes")]);

        let ty = types::load(&root.join("notes")).unwrap().unwrap();
        assert_eq!(ty.id, "notes");
        assert_eq!(ty.label(), "Notes");

        fs::remove_dir_all(root).unwrap();
    }
}
