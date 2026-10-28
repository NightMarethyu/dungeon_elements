//! The Edit > Types dialog: the campaign's folders on the left, the selected folder's type on the right.
//!
//! A subfolder's type shows the fields it inherits from its parent folder's, which can be
//! changed or removed for the subfolder only, then the fields only it has.

use std::fs;
use std::path::{Path, PathBuf};

use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, space, text, text_input,
};
use iced::{Center, Element, Fill};

use crate::style;
use crate::theme::Colors;
use crate::types::{self, ContentType, Field, Kind, Types, Width};

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
    Save,
    Cancel,
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
    folders: Vec<Folder>,
    selected: usize,
    pub error: Option<String>,
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
    /// A link field's folder.
    folder: Option<String>,
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
            folder: self.folder.clone().filter(|_| self.kind.links()),
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
            })
            .collect();

        Editor {
            folders,
            selected: 0,
            error: None,
        }
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
            Message::Select(_) | Message::AddType | Message::Save | Message::Cancel => {}
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

            match problem {
                Ok(yaml) => files.push((folder.path.clone(), yaml)),
                Err(error) => {
                    self.selected = index;
                    return Err(format!("{}: {error}", folder.name));
                }
            }
        }

        for (folder, yaml) in &files {
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
            let name = folder.name.rsplit('/').next().unwrap_or(&folder.name);

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

        let list: Element<'_, Message> = if self.folders.is_empty() {
            text("This campaign has no folders yet.")
                .size(13)
                .color(c.text_muted)
                .into()
        } else {
            scrollable(list).height(Fill).into()
        };

        let detail = match self.folders.get(self.selected) {
            None => space().into(),
            Some(folder) => self.folder_view(c, folder, self.inherited(self.selected)),
        };

        let mut footer = row![].spacing(8).align_y(Center);
        if let Some(error) = &self.error {
            footer = footer.push(text(error).size(13).color(c.danger).width(Fill));
        } else {
            footer = footer.push(
                text("Changes apply to every note in the folder. Data in removed fields stays in the notes, as each note's own field.")
                    .size(12)
                    .color(c.text_muted)
                    .width(Fill),
            );
        }
        footer = footer.push(
            button("Cancel")
                .style(style::secondary(c))
                .on_press(Message::Cancel),
        );
        footer = footer.push(
            button("Save").style(style::primary(c)).on_press_maybe(
                self.folders
                    .iter()
                    .any(|f| f.changed)
                    .then_some(Message::Save),
            ),
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

        let header = row![
            column![
                label("Type name"),
                input("Item", &draft.label).on_input(Message::Label)
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn edits_and_saves_a_type() {
        let (root, mut editor) = editor(types::STARTER_TYPES[0].1);

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
        let (root, _) = editor(types::STARTER_TYPES[0].1);
        let items = root.join("items");
        let weapons = items.join("weapons");
        fs::create_dir_all(&weapons).unwrap();
        let mut own = Types::new();
        own.insert(items.clone(), types::parse(types::STARTER_TYPES[0].1));
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
