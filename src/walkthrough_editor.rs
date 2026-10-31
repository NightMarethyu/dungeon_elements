//! The Edit > Walkthrough Questions dialog, so a campaign's own walkthrough questions can be
//! changed without writing YAML.
//!
//! The chapters are on the left, under the notes the walkthrough makes; the selected
//! chapter's questions, or the notes, are on the right. Saving writes `_walkthrough.yaml`
//! in the campaign folder, or removes it to go back to the built-in questions.

use std::fs;
use std::path::{Path, PathBuf};

use iced::widget::{
    button, checkbox, column, container, pick_list, row, rule, scrollable, space, text, text_input,
};
use iced::{Center, Element, Fill};
use serde_yaml_ng::{Mapping, Value};

use crate::note;
use crate::style;
use crate::theme::Colors;
use crate::types::{self, ContentType};
use crate::walkthrough::{self, AddTo, Chapter, NoteSpec, Question, Questions};

/// The widget id of the list on the right, scrolled to the end for a new question or note.
pub const DETAIL: &str = "walkthrough-editor-detail";

#[derive(Debug, Clone)]
pub enum Message {
    SelectNotes,
    SelectChapter(usize),
    AddChapter,
    ChapterTitle(String),
    ChapterIntro(String),
    ChapterUp,
    ChapterDown,
    RemoveChapter,

    AddQuestion,
    Ask(usize, String),
    Hint(usize, String),
    GoesInto(usize, Choice),
    Field(usize, Choice),
    Table(usize, Choice),
    AddPick(usize, Choice),
    RemovePick(usize, usize),
    Import(usize, bool),
    ImportLabel(usize, String),
    MoveTo(usize, Choice),
    QuestionUp(usize),
    QuestionDown(usize),
    RemoveQuestion(usize),

    AddNote,
    NoteKey(usize, String),
    NoteFolder(usize, Choice),
    NoteMany(usize, bool),
    NoteNaming(usize, Naming),
    NoteName(usize, String),
    AddSet(usize),
    SetField(usize, usize, Choice),
    SetValue(usize, usize, String),
    RemoveSet(usize, usize),
    AddTo(usize, Choice),
    AddToField(usize, Choice),
    RemoveNote(usize),

    /// Copies the built-in questions, to change them.
    StartFromDefault,
    /// Goes back to the built-in questions, removing the campaign's own on saving.
    UseBuiltIn,
    /// Saves, then opens the walkthrough.
    TryIt,
    Save,
    Cancel,
}

/// An entry of a pick list: what it stands for (`None` for "none" or "the default"), and
/// how it reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    value: Option<String>,
    label: String,
}

impl Choice {
    fn new(value: &str, label: impl Into<String>) -> Self {
        Choice {
            value: Some(value.to_string()),
            label: label.into(),
        }
    }

    fn none(label: impl Into<String>) -> Self {
        Choice {
            value: None,
            label: label.into(),
        }
    }
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

pub struct Editor {
    root: PathBuf,
    /// The campaign's folders, like `locations/settlements`, with the type of their notes.
    folders: Vec<(String, Option<ContentType>)>,
    /// The roll tables Inspire me can roll on: the built-in ones, then the campaign's.
    tables: Vec<String>,
    /// The questions being edited, or `None` while the walkthrough asks the built-in ones.
    draft: Option<Draft>,
    /// Why the campaign's own questions can't be read, if they can't.
    broken: Option<String>,
    /// Whether the campaign had its own questions when the dialog opened.
    had_own: bool,
    selected: Selected,
    changed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Selected {
    Notes,
    Chapter(usize),
}

/// The questions as they are being edited. Text is kept as typed until it is saved.
#[derive(Debug, Clone, PartialEq)]
struct Draft {
    notes: Vec<NoteDraft>,
    chapters: Vec<ChapterDraft>,
}

#[derive(Debug, Clone, PartialEq)]
struct NoteDraft {
    /// The short name the questions use for it.
    key: String,
    folder: String,
    many: bool,
    naming: Naming,
    /// Its name, for a note that is always named the same.
    name: String,
    /// Values filled in when it is made: field key and value, as typed.
    set: Vec<(String, String)>,
    /// The note and field that link to it.
    add_to: Option<(String, String)>,
}

/// How a single note is named.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Naming {
    /// A question asks for its name.
    Asked,
    /// It is named after the campaign (`{title}`).
    Title,
    /// It always has the same name, like Session 1.
    Fixed,
}

impl Naming {
    const ALL: [Naming; 3] = [Naming::Asked, Naming::Title, Naming::Fixed];
}

impl std::fmt::Display for Naming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Naming::Asked => "a question asks for it",
            Naming::Title => "the campaign's title",
            Naming::Fixed => "always the same name:",
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ChapterDraft {
    /// The id it was read with, kept so progress still finds it after a rename.
    id: String,
    title: String,
    intro: String,
    questions: Vec<QuestionDraft>,
}

#[derive(Debug, Clone, PartialEq)]
struct QuestionDraft {
    ask: String,
    hint: String,
    note: String,
    field: Option<String>,
    table: Option<String>,
    pick: Vec<String>,
    import: Option<String>,
}

impl Draft {
    fn from_questions(questions: &Questions) -> Self {
        Draft {
            notes: questions
                .notes
                .iter()
                .map(|(key, spec)| NoteDraft {
                    key: key.clone(),
                    folder: spec.folder.trim_matches('/').to_string(),
                    many: spec.many,
                    naming: match spec.name.as_deref() {
                        None => Naming::Asked,
                        Some("{title}") => Naming::Title,
                        Some(_) => Naming::Fixed,
                    },
                    name: spec
                        .name
                        .clone()
                        .filter(|name| name != "{title}")
                        .unwrap_or_default(),
                    set: spec
                        .set
                        .iter()
                        .map(|(key, value)| (note::value_to_text(key), note::value_to_text(value)))
                        .collect(),
                    add_to: spec
                        .add_to
                        .as_ref()
                        .map(|add_to| (add_to.note.clone(), add_to.field.clone())),
                })
                .collect(),
            chapters: questions
                .chapters
                .iter()
                .map(|chapter| ChapterDraft {
                    id: chapter.id.clone(),
                    title: chapter.title.clone(),
                    intro: chapter.intro.clone().unwrap_or_default(),
                    questions: chapter
                        .questions
                        .iter()
                        .map(|question| QuestionDraft {
                            ask: question.ask.clone(),
                            hint: question.hint.clone().unwrap_or_default(),
                            note: question.note.clone(),
                            field: question.field.clone(),
                            table: question.table.clone(),
                            pick: question.pick.clone(),
                            import: question.import.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    fn to_questions(&self) -> Questions {
        let optional = |text: &str| Some(text.trim().to_string()).filter(|text| !text.is_empty());
        Questions {
            notes: self
                .notes
                .iter()
                .map(|draft| {
                    let set: Mapping = draft
                        .set
                        .iter()
                        .filter(|(key, _)| !key.is_empty())
                        .map(|(key, value)| (Value::from(key.as_str()), set_value(value)))
                        .collect();
                    let spec = NoteSpec {
                        folder: draft.folder.clone(),
                        name: match draft.naming {
                            _ if draft.many => None,
                            Naming::Asked => None,
                            Naming::Title => Some("{title}".to_string()),
                            Naming::Fixed => optional(&draft.name),
                        },
                        many: draft.many,
                        set,
                        add_to: draft.add_to.as_ref().map(|(note, field)| AddTo {
                            note: note.clone(),
                            field: field.clone(),
                        }),
                    };
                    (draft.key.trim().to_string(), spec)
                })
                .collect(),
            chapters: self
                .chapters
                .iter()
                .map(|chapter| Chapter {
                    // An id made from the title needn't be written.
                    id: if chapter.id == types::key_from(&chapter.title) {
                        String::new()
                    } else {
                        chapter.id.clone()
                    },
                    title: chapter.title.trim().to_string(),
                    intro: optional(&chapter.intro),
                    questions: chapter
                        .questions
                        .iter()
                        .map(|question| Question {
                            ask: question.ask.trim().to_string(),
                            hint: optional(&question.hint),
                            note: question.note.clone(),
                            field: question.field.clone(),
                            table: question.table.clone(),
                            pick: question.pick.clone(),
                            import: question.import.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    fn note(&self, key: &str) -> Option<&NoteDraft> {
        self.notes.iter().find(|note| note.key == key)
    }
}

/// A value to fill in, as YAML: a whole number stays a number, like a session's number.
fn set_value(text: &str) -> Value {
    let text = text.trim();
    match text.parse::<i64>() {
        Ok(number) => Value::from(number),
        Err(_) => Value::from(text),
    }
}

/// "Faction" as "Factions", "Story" as "Stories".
fn plural(label: &str) -> String {
    let lower = label.to_lowercase();
    if lower.ends_with('y') && !lower.ends_with("ay") && !lower.ends_with("ey") {
        format!("{}ies", &label[..label.len() - 1])
    } else if ["s", "x", "ch", "sh"]
        .iter()
        .any(|end| lower.ends_with(end))
    {
        format!("{label}es")
    } else {
        format!("{label}s")
    }
}

impl Editor {
    /// Opens the questions of the campaign at `root`, with its folders and their types,
    /// and the names of the campaign's roll tables.
    pub fn new(
        root: &Path,
        folders: Vec<(String, Option<ContentType>)>,
        campaign_tables: Vec<String>,
    ) -> Self {
        let mut tables: Vec<String> = crate::tables::BUILT_IN
            .iter()
            .map(|table| table.name.to_string())
            .collect();
        for table in campaign_tables {
            if !tables
                .iter()
                .any(|other| other.eq_ignore_ascii_case(&table))
            {
                tables.push(table);
            }
        }

        let file = root.join(walkthrough::FILE);
        let (draft, broken) = match fs::read_to_string(&file) {
            Err(_) => (None, None),
            Ok(yaml) => match walkthrough::parse(&yaml) {
                Ok(questions) => (Some(Draft::from_questions(&questions)), None),
                Err(error) => (None, Some(error)),
            },
        };
        let selected = match &draft {
            Some(draft) if !draft.chapters.is_empty() => Selected::Chapter(0),
            _ => Selected::Notes,
        };
        Editor {
            root: root.to_path_buf(),
            folders,
            tables,
            had_own: file.is_file(),
            draft,
            broken,
            selected,
            changed: false,
            error: None,
        }
    }

    pub fn has_changes(&self) -> bool {
        self.changed
    }

    /// Whether there are questions to try: the built-in ones, or the campaign's own.
    pub fn can_try(&self) -> bool {
        self.broken.is_none() || self.changed
    }

    fn type_of(&self, folder: &str) -> Option<&ContentType> {
        self.folders
            .iter()
            .find(|(name, _)| name == folder.trim_matches('/'))
            .and_then(|(_, ty)| ty.as_ref())
    }

    /// What's wrong with the questions, in plain words: first the mistakes that stop
    /// them from being saved, then the questions the walkthrough would leave out.
    pub fn problems(&self) -> (Vec<String>, Vec<String>) {
        let Some(draft) = &self.draft else {
            return (Vec::new(), Vec::new());
        };
        let mut mistakes = Vec::new();
        let mut left_out = Vec::new();

        for (index, note) in draft.notes.iter().enumerate() {
            let key = note.key.trim();
            if key.is_empty() {
                mistakes.push("A note it makes has no name.".to_string());
            } else if draft.notes[..index]
                .iter()
                .any(|other| other.key.trim() == key)
            {
                mistakes.push(format!("Two notes it makes are called {key}."));
            }
            if note.folder.trim_matches('/').is_empty() {
                mistakes.push(format!("The note {key} has no folder."));
            }
            if let Some((target, _)) = &note.add_to {
                match draft.note(target) {
                    None => mistakes.push(format!(
                        "The note {key} is linked from {target}, which isn't one of the notes it makes."
                    )),
                    Some(target) if target.many => mistakes.push(format!(
                        "The note {key} is linked from {}, which is several notes. Pick one that is a single note.",
                        target.key
                    )),
                    Some(_) => {}
                }
            }
        }

        let mut filled: Vec<(String, Option<String>)> = Vec::new();
        for chapter in &draft.chapters {
            let title = chapter.title.trim();
            if title.is_empty() {
                mistakes.push("A chapter has no title.".to_string());
            }
            if chapter.questions.is_empty() {
                left_out.push(format!(
                    "The chapter {title} has no questions, so it isn't shown."
                ));
            }
            for question in &chapter.questions {
                let ask = question.ask.trim();
                if ask.is_empty() {
                    mistakes.push(format!("A question in {title} has no text."));
                    continue;
                }
                let Some(note) = draft.note(&question.note) else {
                    mistakes.push(format!(
                        "\"{ask}\" goes into {}, which isn't one of the notes it makes.",
                        question.note
                    ));
                    continue;
                };
                let filling = (question.note.clone(), question.field.clone());
                if filled.contains(&filling) {
                    mistakes.push(format!(
                        "\"{ask}\" fills the same thing in {} as another question. Pick another field.",
                        note.key
                    ));
                }
                filled.push(filling);

                match (self.type_of(&note.folder), &question.field) {
                    (None, _) => left_out.push(format!(
                        "\"{ask}\" is left out: its note goes in {}, which has no fields.",
                        note.folder
                    )),
                    (Some(ty), Some(field))
                        if !ty.fields.iter().any(|other| &other.key == field) =>
                    {
                        left_out.push(format!(
                            "\"{ask}\" is left out: {} notes have no {field} field anymore.",
                            ty.label()
                        ))
                    }
                    _ => {}
                }
            }
        }

        if mistakes.is_empty()
            && let Err(error) = walkthrough::parse(&self.yaml())
        {
            mistakes.push(format!("They can't be saved: {error}."));
        }
        (mistakes, left_out)
    }

    fn yaml(&self) -> String {
        self.draft
            .as_ref()
            .map(|draft| serde_yaml_ng::to_string(&draft.to_questions()).unwrap_or_default())
            .unwrap_or_default()
    }

    /// Writes the questions to `_walkthrough.yaml`, or removes it to use the built-in
    /// questions again.
    pub fn save(&mut self) -> Result<(), String> {
        let file = self.root.join(walkthrough::FILE);
        if self.draft.is_none() {
            if file.exists() {
                fs::remove_file(&file).map_err(|error| error.to_string())?;
            }
        } else {
            let (mistakes, _) = self.problems();
            if let Some(mistake) = mistakes.first() {
                return Err(mistake.clone());
            }
            let yaml = format!(
                "# The questions of this campaign's world-building walkthrough. Change them in\n# Edit > Walkthrough Questions.\n{}",
                self.yaml()
            );
            fs::write(&file, yaml).map_err(|error| error.to_string())?;
        }
        self.changed = false;
        self.had_own = file.exists();
        self.broken = None;
        Ok(())
    }

    pub fn update(&mut self, message: Message) {
        self.error = None;
        match message {
            Message::StartFromDefault => {
                self.draft = Some(Draft::from_questions(&walkthrough::Questions::built_in()));
                self.broken = None;
                self.selected = Selected::Chapter(0);
                self.changed = true;
                return;
            }
            Message::UseBuiltIn => {
                self.draft = None;
                self.broken = None;
                self.selected = Selected::Notes;
                self.changed = self.had_own;
                return;
            }
            Message::SelectNotes => {
                self.selected = Selected::Notes;
                return;
            }
            Message::SelectChapter(index) => {
                self.selected = Selected::Chapter(index);
                return;
            }
            _ => {}
        }

        let selected = self.selected;
        let first_note = |draft: &Draft| {
            draft
                .notes
                .first()
                .map(|note| note.key.clone())
                .unwrap_or_default()
        };
        let Some(draft) = &mut self.draft else {
            return;
        };
        self.changed = true;
        let chapter = match selected {
            Selected::Chapter(index) => index.min(draft.chapters.len().saturating_sub(1)),
            Selected::Notes => 0,
        };

        match message {
            Message::AddChapter => {
                draft.chapters.push(ChapterDraft {
                    id: String::new(),
                    title: format!("Chapter {}", draft.chapters.len() + 1),
                    intro: String::new(),
                    questions: Vec::new(),
                });
                self.selected = Selected::Chapter(draft.chapters.len() - 1);
            }
            Message::ChapterTitle(title) => {
                if let Some(chapter) = draft.chapters.get_mut(chapter) {
                    chapter.title = title;
                }
            }
            Message::ChapterIntro(intro) => {
                if let Some(chapter) = draft.chapters.get_mut(chapter) {
                    chapter.intro = intro;
                }
            }
            Message::ChapterUp if chapter > 0 => {
                draft.chapters.swap(chapter, chapter - 1);
                self.selected = Selected::Chapter(chapter - 1);
            }
            Message::ChapterDown if chapter + 1 < draft.chapters.len() => {
                draft.chapters.swap(chapter, chapter + 1);
                self.selected = Selected::Chapter(chapter + 1);
            }
            Message::RemoveChapter if chapter < draft.chapters.len() => {
                draft.chapters.remove(chapter);
                self.selected = match draft.chapters.len() {
                    0 => Selected::Notes,
                    len => Selected::Chapter(chapter.min(len - 1)),
                };
            }

            Message::AddQuestion => {
                let note = first_note(draft);
                if let Some(chapter) = draft.chapters.get_mut(chapter) {
                    chapter.questions.push(QuestionDraft {
                        ask: String::new(),
                        hint: String::new(),
                        note,
                        field: None,
                        table: None,
                        pick: Vec::new(),
                        import: None,
                    });
                }
            }
            Message::MoveTo(index, to) => {
                let to = to
                    .value
                    .and_then(|to| to.parse::<usize>().ok())
                    .filter(|to| *to < draft.chapters.len() && *to != chapter);
                if let Some(to) = to
                    && index < draft.chapters[chapter].questions.len()
                {
                    let question = draft.chapters[chapter].questions.remove(index);
                    draft.chapters[to].questions.push(question);
                }
            }
            message => {
                if let Some(questions) = draft
                    .chapters
                    .get_mut(chapter)
                    .map(|chapter| &mut chapter.questions)
                    && Self::update_question(questions, &message)
                {
                    return;
                }
                Self::update_notes(draft, message);
            }
        }
    }

    /// Changes a question of the selected chapter; false for messages about anything else.
    fn update_question(questions: &mut Vec<QuestionDraft>, message: &Message) -> bool {
        let count = questions.len();
        let index = match message {
            Message::Ask(index, _)
            | Message::Hint(index, _)
            | Message::GoesInto(index, _)
            | Message::Field(index, _)
            | Message::Table(index, _)
            | Message::AddPick(index, _)
            | Message::RemovePick(index, _)
            | Message::Import(index, _)
            | Message::ImportLabel(index, _)
            | Message::QuestionUp(index)
            | Message::QuestionDown(index)
            | Message::RemoveQuestion(index) => *index,
            _ => return false,
        };
        if index >= count {
            return true;
        }
        let question = &mut questions[index];
        match message.clone() {
            Message::Ask(_, ask) => question.ask = ask,
            Message::Hint(_, hint) => question.hint = hint,
            Message::GoesInto(_, note) => {
                if let Some(note) = note.value {
                    question.note = note;
                    question.field = None;
                    question.pick.clear();
                }
            }
            Message::Field(_, field) => {
                question.field = field.value;
                question.pick.clear();
                if question.field.is_some() {
                    question.import = None;
                }
            }
            Message::Table(_, table) => question.table = table.value,
            Message::AddPick(_, folder) => {
                if let Some(folder) = folder.value
                    && !question.pick.contains(&folder)
                {
                    question.pick.push(folder);
                }
            }
            Message::RemovePick(_, at) if at < question.pick.len() => {
                question.pick.remove(at);
            }
            Message::Import(_, on) => {
                question.import = on.then(|| "Add from Files...".to_string());
            }
            Message::ImportLabel(_, label) => question.import = Some(label),
            Message::QuestionUp(_) if index > 0 => questions.swap(index, index - 1),
            Message::QuestionDown(_) if index + 1 < count => questions.swap(index, index + 1),
            Message::RemoveQuestion(_) => {
                questions.remove(index);
            }
            _ => {}
        }
        true
    }

    fn update_notes(draft: &mut Draft, message: Message) {
        match message {
            Message::AddNote => {
                let mut key = "new_note".to_string();
                let mut number = 1;
                while draft.note(&key).is_some() {
                    number += 1;
                    key = format!("new_note_{number}");
                }
                draft.notes.push(NoteDraft {
                    key,
                    folder: String::new(),
                    many: false,
                    naming: Naming::Asked,
                    name: String::new(),
                    set: Vec::new(),
                    add_to: None,
                });
            }
            Message::NoteKey(index, key) => {
                let Some(old) = draft.notes.get(index).map(|note| note.key.clone()) else {
                    return;
                };
                // Questions, links to it and values naming it follow the new name.
                let key = types::key_from(&key);
                for chapter in &mut draft.chapters {
                    for question in &mut chapter.questions {
                        if question.note == old {
                            question.note = key.clone();
                        }
                    }
                }
                for note in &mut draft.notes {
                    if let Some((target, _)) = &mut note.add_to
                        && *target == old
                    {
                        *target = key.clone();
                    }
                    for (_, value) in &mut note.set {
                        if *value == old {
                            *value = key.clone();
                        }
                    }
                }
                draft.notes[index].key = key;
            }
            Message::NoteFolder(index, folder) => {
                if let (Some(note), Some(folder)) = (draft.notes.get_mut(index), folder.value) {
                    note.folder = folder;
                }
            }
            Message::NoteMany(index, many) => {
                if let Some(note) = draft.notes.get_mut(index) {
                    note.many = many;
                }
            }
            Message::NoteNaming(index, naming) => {
                if let Some(note) = draft.notes.get_mut(index) {
                    note.naming = naming;
                }
            }
            Message::NoteName(index, name) => {
                if let Some(note) = draft.notes.get_mut(index) {
                    note.name = name;
                }
            }
            Message::AddSet(index) => {
                if let Some(note) = draft.notes.get_mut(index) {
                    note.set.push((String::new(), String::new()));
                }
            }
            Message::SetField(index, at, field) => {
                if let Some((key, _)) = draft
                    .notes
                    .get_mut(index)
                    .and_then(|note| note.set.get_mut(at))
                {
                    *key = field.value.unwrap_or_default();
                }
            }
            Message::SetValue(index, at, value) => {
                if let Some((_, old)) = draft
                    .notes
                    .get_mut(index)
                    .and_then(|note| note.set.get_mut(at))
                {
                    *old = value;
                }
            }
            Message::RemoveSet(index, at) => {
                if let Some(note) = draft.notes.get_mut(index)
                    && at < note.set.len()
                {
                    note.set.remove(at);
                }
            }
            Message::AddTo(index, target) => {
                if let Some(note) = draft.notes.get_mut(index) {
                    note.add_to = target.value.map(|target| (target, String::new()));
                }
            }
            Message::AddToField(index, field) => {
                if let Some((_, old)) = draft
                    .notes
                    .get_mut(index)
                    .and_then(|note| note.add_to.as_mut())
                {
                    *old = field.value.unwrap_or_default();
                }
            }
            Message::RemoveNote(index) if index < draft.notes.len() => {
                draft.notes.remove(index);
            }
            _ => {}
        }
    }

    /// How a note reads in the "Goes into" list: "factions: several new Factions".
    fn describe(&self, note: &NoteDraft) -> String {
        let Some(ty) = self.type_of(&note.folder) else {
            return match note.folder.is_empty() {
                true => format!("{}: no folder yet", note.key),
                false => format!("{}: in {}, which has no fields", note.key, note.folder),
            };
        };
        let label = ty.label();
        let name = note.name.trim();
        match note.naming {
            _ if note.many => format!("{}: several new {}", note.key, plural(&label)),
            Naming::Title => format!("{}: the {label}", note.key),
            Naming::Fixed if !name.is_empty() => format!("{}: {name}", note.key),
            _ => format!("{}: a new {label}", note.key),
        }
    }

    fn folder_choices(&self) -> Vec<Choice> {
        self.folders
            .iter()
            .map(|(folder, ty)| match ty {
                Some(ty) => Choice::new(folder, format!("{folder} ({} notes)", ty.label())),
                None => Choice::new(folder, format!("{folder} (no fields)")),
            })
            .collect()
    }

    /// The fields of the notes in `folder` to pick from, by label. The name isn't one: it
    /// is asked for with no field.
    fn field_choices(&self, folder: &str, links_only: bool) -> Vec<Choice> {
        self.type_of(folder)
            .map(|ty| {
                ty.fields
                    .iter()
                    .filter(|field| field.key != "name" && (!links_only || field.kind.links()))
                    .map(|field| Choice::new(&field.key, field.label()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A pick list's entry for `value` among `choices`, or one that shows the value as
    /// it is, for one that isn't there (like a field the type no longer has).
    fn chosen(choices: &[Choice], value: &str) -> Choice {
        choices
            .iter()
            .find(|choice| choice.value.as_deref() == Some(value))
            .cloned()
            .unwrap_or_else(|| Choice::new(value, format!("{value} (not there)")))
    }

    pub fn view(&self, c: Colors) -> Element<'_, Message> {
        let detail: Element<'_, Message> = match (&self.draft, self.selected) {
            (None, _) => self.built_in_view(c),
            (Some(draft), Selected::Notes) => self.notes_view(c, draft),
            (Some(draft), Selected::Chapter(index)) => match draft.chapters.get(index) {
                Some(chapter) => self.chapter_view(c, draft, chapter, index),
                None => space().into(),
            },
        };

        let list: Element<'_, Message> = match &self.draft {
            None => space().into(),
            Some(draft) => {
                let entry = |title: String, detail: String, selected: bool, message| {
                    button(
                        column![
                            text(title).size(14),
                            text(detail).size(12).color(c.text_muted),
                        ]
                        .spacing(2),
                    )
                    .width(Fill)
                    .padding([6, 10])
                    .style(style::flat(c, selected))
                    .on_press(message)
                    .into()
                };
                let mut entries: Vec<Element<'_, Message>> = vec![entry(
                    "Notes it makes".into(),
                    format!("{} notes", draft.notes.len()),
                    self.selected == Selected::Notes,
                    Message::SelectNotes,
                )];
                entries.push(text("Chapters").size(12).color(c.text_muted).into());
                for (index, chapter) in draft.chapters.iter().enumerate() {
                    let count = chapter.questions.len();
                    entries.push(entry(
                        match chapter.title.trim() {
                            "" => "Untitled chapter".into(),
                            title => title.to_string(),
                        },
                        format!("{count} question{}", if count == 1 { "" } else { "s" }),
                        self.selected == Selected::Chapter(index),
                        Message::SelectChapter(index),
                    ));
                }
                entries.push(space().height(4).into());
                entries.push(
                    button(text("Add Chapter").size(14))
                        .width(Fill)
                        .padding([6, 10])
                        .style(style::secondary(c))
                        .on_press(Message::AddChapter)
                        .into(),
                );
                scrollable(column(entries).spacing(4)).height(Fill).into()
            }
        };

        let (mistakes, left_out) = self.problems();
        let mut checks = column![].spacing(2);
        for mistake in mistakes.iter().take(3) {
            checks = checks.push(text(mistake.clone()).size(13).color(c.danger));
        }
        for note in left_out.iter().take(3usize.saturating_sub(mistakes.len())) {
            checks = checks.push(text(note.clone()).size(12).color(c.text_muted));
        }
        let more = (mistakes.len() + left_out.len()).saturating_sub(3);
        if more > 0 {
            checks = checks.push(
                text(format!("And {more} more."))
                    .size(12)
                    .color(c.text_muted),
            );
        }
        if let Some(error) = &self.error {
            checks = checks.push(text(error).size(13).color(c.danger));
        }

        let mut footer = row![].spacing(8).align_y(Center);
        if self.draft.is_some() {
            footer = footer.push(
                button("Use the Built-In Questions")
                    .style(style::secondary(c))
                    .on_press(Message::UseBuiltIn),
            );
        }
        footer = footer.push(space::horizontal());
        footer = footer.push(
            button("Cancel")
                .style(style::secondary(c))
                .on_press(Message::Cancel),
        );
        let can_save = mistakes.is_empty();
        footer = footer.push(
            button(if self.changed {
                "Save and Try It"
            } else {
                "Try It"
            })
            .style(style::secondary(c))
            .on_press_maybe((can_save && self.can_try()).then_some(Message::TryIt)),
        );
        footer = footer.push(
            button("Save")
                .style(style::primary(c))
                .on_press_maybe((self.changed && can_save).then_some(Message::Save)),
        );

        container(
            column![
                text("Walkthrough Questions").size(22).color(c.text),
                row![
                    container(list).width(220).height(Fill),
                    rule::vertical(1).style(style::separator(c)),
                    container(detail).width(Fill).height(Fill),
                ]
                .spacing(12)
                .height(Fill),
                checks,
                footer,
            ]
            .spacing(12),
        )
        .width(1100)
        .height(680)
        .padding(24)
        .style(style::dialog(c))
        .into()
    }

    /// What shows while the walkthrough asks the built-in questions.
    fn built_in_view(&self, c: Colors) -> Element<'_, Message> {
        let mut content = column![].spacing(12).max_width(560);
        if let Some(error) = &self.broken {
            content = content.push(
                text(format!(
                    "This campaign's own questions can't be read: {error}."
                ))
                .size(14)
                .color(c.danger),
            );
            content = content.push(
                text("Start over from the default questions to replace them, or use the built-in questions instead.")
                    .size(13)
                    .color(c.text_muted),
            );
        } else {
            content = content.push(
                text("This campaign asks the built-in questions.")
                    .size(14)
                    .color(c.text),
            );
            content = content.push(
                text("They are made for the starter folders, and get better as the app updates. To ask your own, start from them and change, add or remove questions. Questions about folders or fields the campaign doesn't have are left out.")
                    .size(13)
                    .color(c.text_muted),
            );
        }
        content = content.push(
            button("Start from the Default Questions")
                .style(style::primary(c))
                .on_press(Message::StartFromDefault),
        );
        if self.broken.is_some() {
            content = content.push(
                button("Use the Built-In Questions")
                    .style(style::secondary(c))
                    .on_press(Message::UseBuiltIn),
            );
        }
        content.into()
    }

    fn chapter_view<'a>(
        &'a self,
        c: Colors,
        draft: &'a Draft,
        chapter: &'a ChapterDraft,
        index: usize,
    ) -> Element<'a, Message> {
        let label = |label| text(label).size(12).color(c.text_muted);
        let small = |label: &'static str, message: Option<Message>| {
            button(text(label).size(13))
                .padding([4, 8])
                .style(style::flat(c, false))
                .on_press_maybe(message)
        };
        let count = draft.chapters.len();

        let header = column![
            row![
                column![
                    label("Chapter title"),
                    input(c, "The heroes", &chapter.title).on_input(Message::ChapterTitle),
                ]
                .spacing(4)
                .width(Fill),
                small("↑", (index > 0).then_some(Message::ChapterUp)),
                small("↓", (index + 1 < count).then_some(Message::ChapterDown)),
                small("Remove", Some(Message::RemoveChapter)),
            ]
            .spacing(6)
            .align_y(iced::Bottom),
            column![
                label("A line at the top of the chapter (optional)"),
                input(c, "What the chapter is about", &chapter.intro)
                    .on_input(Message::ChapterIntro),
            ]
            .spacing(4),
        ]
        .spacing(8);

        let notes: Vec<Choice> = draft
            .notes
            .iter()
            .map(|note| Choice::new(&note.key, self.describe(note)))
            .collect();
        let mut tables = vec![
            Choice::none("Automatic (the field's table)"),
            Choice::new("", "No Inspire me"),
        ];
        tables.extend(self.tables.iter().map(|table| Choice::new(table, table)));
        let chapters: Vec<Choice> = draft
            .chapters
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(other, chapter)| Choice::new(&other.to_string(), chapter.title.clone()))
            .collect();

        let questions = chapter.questions.iter().enumerate().map(|(at, question)| {
            let note = draft.note(&question.note);
            let many = note.is_some_and(|note| note.many);
            let folder = note.map(|note| note.folder.as_str()).unwrap_or_default();

            let mut fields = vec![Choice::none(if many { "Their names" } else { "Its name" })];
            fields.extend(self.field_choices(folder, false));
            let field = match &question.field {
                None => fields[0].clone(),
                Some(key) => Self::chosen(&fields, key),
            };
            let table = match &question.table {
                None => tables[0].clone(),
                // Several tables, like "Female names, Male names", roll one each.
                Some(table)
                    if table.contains(',')
                        && table.split(',').all(|part| {
                            self.tables
                                .iter()
                                .any(|known| known.eq_ignore_ascii_case(part.trim()))
                        }) =>
                {
                    Choice::new(table, table.clone())
                }
                Some(table) => Self::chosen(&tables, table),
            };

            let pick = |label: &'static str, element: Element<'a, Message>| {
                column![text(label).size(12).color(c.text_muted), element]
                    .spacing(4)
                    .width(Fill)
            };
            let list =
                |choices: Vec<Choice>, selected: Choice, on: fn(usize, Choice) -> Message| {
                    pick_list(choices, Some(selected), move |choice| on(at, choice))
                        .text_size(13)
                        .padding(6)
                        .width(Fill)
                        .style(style::choice(c))
                        .menu_style(style::choice_menu(c))
                };

            let mut card = column![
                row![
                    input(c, "The question, like: What do {name} want?", &question.ask)
                        .on_input(move |ask| Message::Ask(at, ask))
                        .width(Fill),
                    small("↑", (at > 0).then_some(Message::QuestionUp(at))),
                    small(
                        "↓",
                        (at + 1 < chapter.questions.len()).then_some(Message::QuestionDown(at))
                    ),
                    small("Remove", Some(Message::RemoveQuestion(at))),
                ]
                .spacing(6)
                .align_y(Center),
                input(
                    c,
                    "A hint under it, to help answer it (optional)",
                    &question.hint
                )
                .on_input(move |hint| Message::Hint(at, hint)),
                row![
                    pick(
                        "Goes into",
                        list(
                            notes.clone(),
                            Self::chosen(&notes, &question.note),
                            Message::GoesInto
                        )
                        .into()
                    ),
                    pick("Field", list(fields, field, Message::Field).into()),
                    pick(
                        "Inspire me from",
                        list(tables.clone(), table, Message::Table).into()
                    ),
                ]
                .spacing(8),
            ]
            .spacing(8);

            // A field that links to notes offers notes to pick from.
            let links = question.field.as_ref().is_some_and(|key| {
                self.type_of(folder).is_some_and(|ty| {
                    ty.fields
                        .iter()
                        .any(|field| &field.key == key && field.kind.links())
                })
            });
            if links {
                let mut chips = row![text("Pick from").size(12).color(c.text_muted)]
                    .spacing(6)
                    .align_y(Center);
                if question.pick.is_empty() {
                    chips = chips.push(
                        text("the folder the field links to")
                            .size(12)
                            .color(c.text_muted),
                    );
                }
                for (number, folder) in question.pick.iter().enumerate() {
                    chips = chips.push(
                        button(text(format!("{folder} ✕")).size(12))
                            .padding([2, 8])
                            .style(style::secondary(c))
                            .on_press(Message::RemovePick(at, number)),
                    );
                }
                chips = chips.push(
                    pick_list(self.folder_choices(), None::<Choice>, move |folder| {
                        Message::AddPick(at, folder)
                    })
                    .placeholder("Add a folder")
                    .text_size(12)
                    .padding(4)
                    .style(style::choice(c))
                    .menu_style(style::choice_menu(c)),
                );
                card = card.push(chips.wrap());
            }
            if many && question.field.is_none() {
                let mut import = row![
                    checkbox(question.import.is_some())
                        .label("A button that adds notes from Markdown files")
                        .on_toggle(move |on| Message::Import(at, on))
                        .text_size(13)
                        .style(style::check(c)),
                ]
                .spacing(8)
                .align_y(Center);
                if let Some(label) = &question.import {
                    import = import.push(
                        input(c, "Button label", label)
                            .on_input(move |label| Message::ImportLabel(at, label))
                            .width(260),
                    );
                }
                card = card.push(import);
            }
            if !chapters.is_empty() {
                card = card.push(row![
                    space::horizontal(),
                    pick_list(chapters.clone(), None::<Choice>, move |to| {
                        Message::MoveTo(at, to)
                    })
                    .placeholder("Move to another chapter")
                    .text_size(12)
                    .padding(4)
                    .style(style::choice(c))
                    .menu_style(style::choice_menu(c)),
                ]);
            }

            container(card)
                .padding(12)
                .width(Fill)
                .style(card_style(c))
                .into()
        });

        let add = button("Add Question")
            .style(style::secondary(c))
            .on_press_maybe((!draft.notes.is_empty()).then_some(Message::AddQuestion));
        let empty_notes = draft.notes.is_empty().then(|| {
            text("Add the notes it makes first, under Notes it makes.")
                .size(12)
                .color(c.text_muted)
        });

        scrollable(
            column![
                header,
                text("Write {name} in a question about one of several notes, like \"What do {name} want?\", and it is asked about each one.")
                    .size(12)
                    .color(c.text_muted),
                column(questions).spacing(10),
                row![add].push(empty_notes).spacing(12).align_y(Center),
            ]
            .spacing(14)
            .padding([0, 12]),
        )
        .id(DETAIL)
        .height(Fill)
        .into()
    }

    fn notes_view<'a>(&'a self, c: Colors, draft: &'a Draft) -> Element<'a, Message> {
        let label = |label| text(label).size(12).color(c.text_muted);
        let small = |label: &'static str, message: Message| {
            button(text(label).size(13))
                .padding([4, 8])
                .style(style::flat(c, false))
                .on_press(message)
        };
        let folders = self.folder_choices();
        let singles: Vec<Choice> = std::iter::once(Choice::none("Nothing"))
            .chain(
                draft
                    .notes
                    .iter()
                    .filter(|note| !note.many)
                    .map(|note| Choice::new(&note.key, note.key.clone())),
            )
            .collect();

        let cards = draft.notes.iter().enumerate().map(|(index, note)| {
            let fields = self.field_choices(&note.folder, false);
            let mut card = column![
                row![
                    column![
                        label("Name the questions use"),
                        input(c, "factions", &note.key)
                            .on_input(move |key| Message::NoteKey(index, key)),
                    ]
                    .spacing(4)
                    .width(200),
                    column![
                        label("Folder"),
                        pick_list(
                            folders.clone(),
                            (!note.folder.is_empty()).then(|| Self::chosen(&folders, &note.folder)),
                            move |folder| Message::NoteFolder(index, folder)
                        )
                        .placeholder("Pick a folder")
                        .text_size(13)
                        .padding(6)
                        .width(Fill)
                        .style(style::choice(c))
                        .menu_style(style::choice_menu(c)),
                    ]
                    .spacing(4)
                    .width(Fill),
                    small("Remove", Message::RemoveNote(index)),
                ]
                .spacing(8)
                .align_y(iced::Bottom),
                row![
                    checkbox(note.many)
                        .label("Several: a list of names makes a note for each")
                        .on_toggle(move |on| Message::NoteMany(index, on))
                        .text_size(13)
                        .style(style::check(c)),
                ]
                .push((!note.many).then(|| {
                    row![
                        text("Its name").size(12).color(c.text_muted),
                        pick_list(Naming::ALL, Some(note.naming), move |naming| {
                            Message::NoteNaming(index, naming)
                        })
                        .text_size(13)
                        .padding(4)
                        .style(style::choice(c))
                        .menu_style(style::choice_menu(c)),
                    ]
                    .push((note.naming == Naming::Fixed).then(|| {
                        input(c, "Session 1", &note.name)
                            .on_input(move |name| Message::NoteName(index, name))
                            .width(160)
                    }))
                    .spacing(6)
                    .align_y(Center)
                }))
                .spacing(16)
                .align_y(Center),
            ]
            .spacing(8);

            // Values filled in when it is made.
            for (at, (key, value)) in note.set.iter().enumerate() {
                let field = (!key.is_empty()).then(|| Self::chosen(&fields, key));
                card = card.push(
                    row![
                        text("Fill in").size(12).color(c.text_muted),
                        pick_list(fields.clone(), field, move |field| {
                            Message::SetField(index, at, field)
                        })
                        .placeholder("Pick a field")
                        .text_size(13)
                        .padding(4)
                        .width(200)
                        .style(style::choice(c))
                        .menu_style(style::choice_menu(c)),
                        text("with").size(12).color(c.text_muted),
                        input(c, "a value, or another note's name to link to it", value)
                            .on_input(move |value| Message::SetValue(index, at, value))
                            .width(Fill),
                        small("✕", Message::RemoveSet(index, at)),
                    ]
                    .spacing(6)
                    .align_y(Center),
                );
            }

            let target = match &note.add_to {
                None => singles[0].clone(),
                Some((target, _)) => Self::chosen(&singles, target),
            };
            let mut link = row![
                button(text("Fill In a Field").size(12))
                    .padding([3, 8])
                    .style(style::secondary(c))
                    .on_press(Message::AddSet(index)),
                space().width(12),
                text("Also link to it from").size(12).color(c.text_muted),
                pick_list(
                    singles
                        .iter()
                        .filter(|choice| choice.value.as_deref() != Some(note.key.as_str()))
                        .cloned()
                        .collect::<Vec<_>>(),
                    Some(target),
                    move |target| Message::AddTo(index, target)
                )
                .text_size(13)
                .padding(4)
                .width(160)
                .style(style::choice(c))
                .menu_style(style::choice_menu(c)),
            ]
            .spacing(6)
            .align_y(Center);
            if let Some((target, field)) = &note.add_to {
                let folder = draft
                    .note(target)
                    .map(|target| target.folder.as_str())
                    .unwrap_or_default();
                let fields = self.field_choices(folder, true);
                link = link.push(
                    pick_list(
                        fields.clone(),
                        (!field.is_empty()).then(|| Self::chosen(&fields, field)),
                        move |field| Message::AddToField(index, field),
                    )
                    .placeholder("its field")
                    .text_size(13)
                    .padding(4)
                    .width(160)
                    .style(style::choice(c))
                    .menu_style(style::choice_menu(c)),
                );
            }
            card = card.push(link);

            container(card)
                .padding(12)
                .width(Fill)
                .style(card_style(c))
                .into()
        });

        scrollable(
            column![
                text("The notes the walkthrough makes. Each question goes into one of them.")
                    .size(13)
                    .color(c.text),
                column(cards).spacing(10),
                button("Add Note")
                    .style(style::secondary(c))
                    .on_press(Message::AddNote),
            ]
            .spacing(14)
            .padding([0, 12]),
        )
        .id(DETAIL)
        .height(Fill)
        .into()
    }
}

/// A question's or note's box: darker than the dialog, so its inputs and boxes show.
fn card_style(c: Colors) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| {
        container::Style::default().background(c.background).border(
            iced::Border::default()
                .color(c.border)
                .width(c.border_width.max(1.0))
                .rounded(c.corner_radius.min(6.0)),
        )
    }
}

fn input<'a>(c: Colors, placeholder: &str, value: &str) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value)
        .size(14)
        .padding(6)
        .style(style::input(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folders() -> Vec<(String, Option<ContentType>)> {
        types::STARTER_TYPES
            .iter()
            .map(|(folder, _)| (folder.to_string(), types::starter(folder)))
            .collect()
    }

    fn temp_folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!(
            "dungeon_elements_walkthrough_editor_{name}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn the_default_questions_round_trip() {
        let built_in = Questions::built_in();
        let draft = Draft::from_questions(&built_in);
        let yaml = serde_yaml_ng::to_string(&draft.to_questions()).unwrap();
        assert_eq!(walkthrough::parse(&yaml).unwrap(), built_in);
    }

    #[test]
    fn starts_from_the_default_questions_and_saves_them() {
        let root = temp_folder("save");
        let mut editor = Editor::new(&root, folders(), Vec::new());
        assert!(editor.draft.is_none());
        assert!(!editor.has_changes());

        editor.update(Message::StartFromDefault);
        editor.update(Message::Ask(0, "What is the story called?".into()));
        let (mistakes, left_out) = editor.problems();
        assert!(mistakes.is_empty(), "{mistakes:?}");
        assert!(left_out.is_empty(), "{left_out:?}");
        editor.save().unwrap();

        let saved = Questions::load(&root).unwrap();
        assert_eq!(
            saved.chapters[0].questions[0].ask,
            "What is the story called?"
        );
        assert_eq!(saved.chapters[0].id, "campaign");
        assert_eq!(saved.notes, Questions::built_in().notes);

        // Back to the built-in questions removes the file.
        let mut editor = Editor::new(&root, folders(), Vec::new());
        assert!(editor.draft.is_some());
        editor.update(Message::UseBuiltIn);
        assert!(editor.has_changes());
        editor.save().unwrap();
        assert!(!root.join(walkthrough::FILE).exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn renaming_a_note_follows_it_everywhere() {
        let root = temp_folder("rename");
        let mut editor = Editor::new(&root, folders(), Vec::new());
        editor.update(Message::StartFromDefault);
        let draft = editor.draft.as_ref().unwrap();
        let start = draft.notes.iter().position(|n| n.key == "start").unwrap();
        editor.update(Message::NoteKey(start, "Home Town".into()));

        let questions = editor.draft.as_ref().unwrap().to_questions();
        assert!(questions.notes.contains_key("home_town"));
        assert!(!questions.notes.contains_key("start"));
        assert_eq!(
            questions.notes["locals"].set.get("location"),
            Some(&Value::from("home_town"))
        );
        assert!(
            questions
                .all()
                .any(|(_, question)| question.note == "home_town")
        );
        assert!(editor.problems().0.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tells_mistakes_in_plain_words() {
        let root = temp_folder("mistakes");
        let mut editor = Editor::new(&root, folders(), Vec::new());
        editor.update(Message::StartFromDefault);
        editor.update(Message::AddQuestion);
        let (mistakes, _) = editor.problems();
        assert_eq!(mistakes, ["A question in The campaign has no text."]);
        assert!(editor.save().is_err());
        assert!(!root.join(walkthrough::FILE).exists());

        // A question whose field is gone is left out, but can still be saved.
        let last = editor.draft.as_ref().unwrap().chapters[0].questions.len() - 1;
        editor.update(Message::RemoveQuestion(last));
        editor.draft.as_mut().unwrap().chapters[0].questions[1].field = Some("gone".into());
        let (mistakes, left_out) = editor.problems();
        assert!(mistakes.is_empty(), "{mistakes:?}");
        assert_eq!(
            left_out,
            [
                "\"Sum it up in one sentence.\" is left out: Campaign notes have no gone field anymore."
            ]
        );
        editor.save().unwrap();

        // Two questions filling the same field.
        editor.draft.as_mut().unwrap().chapters[0].questions[1].field = Some("system".into());
        assert!(editor.problems().0[0].contains("fills the same thing"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_broken_file_can_be_replaced() {
        let root = temp_folder("broken");
        fs::write(root.join(walkthrough::FILE), "chapters: [").unwrap();
        let mut editor = Editor::new(&root, folders(), Vec::new());
        assert!(editor.broken.is_some());
        assert!(!editor.can_try());
        editor.update(Message::StartFromDefault);
        editor.save().unwrap();
        assert!(Questions::load(&root).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plurals() {
        assert_eq!(plural("Faction"), "Factions");
        assert_eq!(plural("Story"), "Stories");
        assert_eq!(plural("Day"), "Days");
        assert_eq!(plural("Boss"), "Bosses");
    }
}
