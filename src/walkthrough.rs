//! The world-building walkthrough: questions that ask the GM about their campaign one at a
//! time and write each answer straight into a note, linking the notes as it goes.
//!
//! The questions are data. The default set is built in (`assets/walkthrough.yaml`, made
//! for the starter folders), and a campaign can have its own in `_walkthrough.yaml` in the
//! campaign folder. Questions whose folder or field the campaign doesn't have are left out,
//! so the built-in set also works for older campaigns and changed types.
//!
//! What the GM has done is kept in `.dungeon_elements/walkthrough.yaml`: which notes the
//! walkthrough made, which questions were skipped, and where the GM was. Whether a
//! question is answered is read from its note, so the notes stay the truth.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

use crate::campaign;
use crate::form::Form;
use crate::links;
use crate::note;
use crate::types::{self, ContentType, Field, Kind, Types};

/// The file in the campaign folder with the campaign's own questions.
pub const FILE: &str = "_walkthrough.yaml";
/// Where the GM's progress is kept, in the campaign folder.
const PROGRESS_FILE: &str = ".dungeon_elements/walkthrough.yaml";
/// The questions used when a campaign has none of its own.
const BUILT_IN: &str = include_str!("../assets/walkthrough.yaml");
/// What Inspire me rolls on for a text question that names no table.
pub const FALLBACK_TABLE: &str = "Action and Theme";

/// A walkthrough's questions, in chapters, and the notes they fill in.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Questions {
    /// The notes the walkthrough makes, by the short name the questions use for them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub notes: BTreeMap<String, NoteSpec>,
    pub chapters: Vec<Chapter>,
}

/// A note (or, with `many`, a list of notes) the walkthrough makes.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NoteSpec {
    /// The folder it goes in, inside the campaign, like `locations/settlements`.
    pub folder: String,
    /// Its name, if it has a set one rather than asking for it. `{title}` is the
    /// campaign's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Whether a list of names makes a note for each.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub many: bool,
    /// Values filled in when the note is made, by field key. A value that is the name of
    /// another of the walkthrough's notes becomes a link to it.
    #[serde(default, skip_serializing_if = "Mapping::is_empty")]
    pub set: Mapping,
    /// A field of another of the walkthrough's notes that gets a link to this one when it
    /// is made, like the campaign's Players for each player character.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub add_to: Option<AddTo>,
}

impl NoteSpec {
    /// The set name the note gets, with the campaign's title filled in. A list of notes
    /// has none.
    pub fn set_name(&self, campaign: &Campaign) -> Option<String> {
        let name = self.name.as_ref().filter(|_| !self.many)?;
        Some(name.replace("{title}", &campaign.title()))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AddTo {
    pub note: String,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Chapter {
    /// What progress calls the chapter; made from the title if not given.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    pub title: String,
    /// A line shown at the top of the chapter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
    #[serde(default)]
    pub questions: Vec<Question>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    /// The question. `{name}` is the name of the note it is about.
    pub ask: String,
    /// A line under the question, to help answer it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Which of the walkthrough's notes the answer goes into.
    pub note: String,
    /// The key of the field the answer fills. With none, the answer is the note's name
    /// (or, for a `many` note, the names of the notes to make).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// The roll tables Inspire me rolls on, with commas between them, like a field's. An
    /// empty one leaves Inspire me out, for answers no table could give, like who plays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// For an answer that links to notes: the folders whose notes are offered to pick
    /// from, and where a new name can be made. With none, the field's own link folder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pick: Vec<String>,
}

impl Question {
    /// What progress calls the question: its note, then its field, like `factions.goal`.
    pub fn key(&self) -> String {
        match &self.field {
            Some(field) => format!("{}.{field}", self.note),
            None => self.note.clone(),
        }
    }

    /// The question about the note called `name`, with `{name}` filled in.
    pub fn ask_about(&self, name: Option<&str>) -> String {
        match name {
            // "What do the {name} want?" about The Pale Court shouldn't say "the The".
            Some(name) if name.to_lowercase().starts_with("the ") => self
                .ask
                .replace("the {name}", name)
                .replace("The {name}", name)
                .replace("{name}", name),
            Some(name) => self.ask.replace("{name}", name),
            None => self.ask.replace("{name}", "it"),
        }
    }

    /// The tables Inspire me rolls on for this question, which fills `field` (or, with no
    /// field, names notes of type `ty`): the question's own, or else the field's, or else
    /// [`FALLBACK_TABLE`] for text. Fields that hold a set kind of value, like a date or a
    /// choice, have none.
    pub fn tables(&self, field: Option<&Field>, ty: &ContentType) -> Vec<String> {
        let split = |tables: &str| -> Vec<String> {
            tables
                .split(',')
                .map(str::trim)
                .filter(|table| !table.is_empty())
                .map(str::to_string)
                .collect()
        };

        let (kind, own) = match field {
            Some(field) => (field.kind, field.table.as_deref()),
            // Names are rolled like the name field's.
            None => {
                let name = ty.name_field().map(|index| &ty.fields[index]);
                (Kind::Text, name.and_then(|field| field.table.as_deref()))
            }
        };
        let inspired = matches!(
            kind,
            Kind::Text | Kind::Longtext | Kind::Steps | Kind::Link | Kind::Links | Kind::Tags
        );
        if !inspired {
            return Vec::new();
        }
        if let Some(tables) = self.table.as_deref().or(own) {
            return split(tables);
        }
        match (field, kind) {
            (Some(_), Kind::Text | Kind::Longtext | Kind::Steps) => {
                vec![FALLBACK_TABLE.to_string()]
            }
            _ => Vec::new(),
        }
    }
}

impl Questions {
    /// The built-in questions.
    pub fn built_in() -> Self {
        parse(BUILT_IN).expect("the built-in walkthrough questions can be read")
    }

    /// The questions of the campaign at `root`: its own, or else the built-in ones.
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join(FILE);
        if !path.is_file() {
            return Ok(Self::built_in());
        }
        fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|yaml| parse(&yaml))
            .map_err(|error| format!("{FILE} can't be read: {error}"))
    }

    /// The questions that fit the campaign: those whose note's folder has a type with the
    /// question's field. Chapters left with no questions are left out too.
    pub fn fitting(&self, campaign: &Campaign) -> Self {
        let chapters = self
            .chapters
            .iter()
            .map(|chapter| Chapter {
                questions: chapter
                    .questions
                    .iter()
                    .filter(|question| self.fits(campaign, question))
                    .cloned()
                    .collect(),
                ..chapter.clone()
            })
            .filter(|chapter| !chapter.questions.is_empty())
            .collect();
        Questions {
            notes: self.notes.clone(),
            chapters,
        }
    }

    fn fits(&self, campaign: &Campaign, question: &Question) -> bool {
        let Some(ty) = self
            .notes
            .get(&question.note)
            .and_then(|spec| campaign.type_of(spec))
        else {
            return false;
        };
        match &question.field {
            Some(key) => ty.fields.iter().any(|field| &field.key == key),
            None => true,
        }
    }

    /// Whether any of the questions would be asked in a campaign with these typed
    /// folders (inside the campaign, like `locations/settlements`).
    pub fn asks_about(&self, typed: &[&str]) -> bool {
        self.notes
            .values()
            .any(|spec| typed.contains(&spec.folder.trim_matches('/')))
    }

    /// The questions in order, with the chapter each is in.
    pub fn all(&self) -> impl Iterator<Item = (&Chapter, &Question)> {
        self.chapters.iter().flat_map(|chapter| {
            chapter
                .questions
                .iter()
                .map(move |question| (chapter, question))
        })
    }
}

/// Reads walkthrough questions from YAML, checking that every question goes into one of
/// its notes and that no two questions fill the same field.
pub fn parse(yaml: &str) -> Result<Questions, String> {
    let mut questions: Questions =
        serde_yaml_ng::from_str(yaml).map_err(|error| error.to_string())?;

    for (name, spec) in &questions.notes {
        if spec.folder.trim_matches('/').is_empty() {
            return Err(format!("the note {name} has no folder"));
        }
        if let Some(add_to) = &spec.add_to {
            match questions.notes.get(&add_to.note) {
                None => {
                    return Err(format!(
                        "the note {name} is added to {}, which isn't one of the notes",
                        add_to.note
                    ));
                }
                Some(target) if target.many => {
                    return Err(format!(
                        "the note {name} is added to {}, which is a list of notes",
                        add_to.note
                    ));
                }
                Some(_) => {}
            }
        }
    }

    let mut chapters = HashSet::new();
    let mut keys = HashSet::new();
    for chapter in &mut questions.chapters {
        if chapter.id.is_empty() {
            chapter.id = types::key_from(&chapter.title);
        }
        if !chapters.insert(chapter.id.clone()) {
            return Err(format!("two chapters are called {}", chapter.id));
        }
        for question in &chapter.questions {
            if !questions.notes.contains_key(&question.note) {
                return Err(format!(
                    "\"{}\" goes into {}, which isn't one of the notes",
                    question.ask, question.note
                ));
            }
            if !keys.insert(question.key()) {
                return Err(format!("two questions fill {}", question.key()));
            }
        }
    }

    Ok(questions)
}

/// The campaign a walkthrough writes into.
pub struct Campaign<'a> {
    pub root: &'a Path,
    /// Each folder's type.
    pub types: &'a Types,
    /// The campaign's notes, to tell which links name a note already.
    pub index: &'a links::Index,
}

impl Campaign<'_> {
    fn folder(&self, spec: &NoteSpec) -> PathBuf {
        self.root.join(spec.folder.trim_matches('/'))
    }

    pub fn type_of(&self, spec: &NoteSpec) -> Option<&ContentType> {
        self.types.get(&self.folder(spec))?.as_ref().ok()
    }

    /// The campaign's title: its folder's name.
    fn title(&self) -> String {
        self.root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn relative(&self, path: &Path) -> String {
        relative(self.root, path)
    }
}

/// `path` inside the campaign at `root`, with `/` between folders.
fn relative(root: &Path, path: &Path) -> String {
    let path = path.strip_prefix(root).unwrap_or(path);
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// What the GM has done in a campaign's walkthrough.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Progress {
    /// The notes made for each of the walkthrough's notes, inside the campaign.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub notes: BTreeMap<String, Made>,
    /// The questions ([`Question::key`]) and chapters skipped.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub skipped: BTreeSet<String>,
    /// The question the GM was at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

/// The note, or notes, made for one of the walkthrough's notes.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Made {
    One(String),
    Many(Vec<String>),
}

impl Made {
    fn paths(&self) -> Vec<String> {
        match self {
            Made::One(path) => vec![path.clone()],
            Made::Many(paths) => paths.clone(),
        }
    }

    fn paths_mut(&mut self) -> Vec<&mut String> {
        match self {
            Made::One(path) => vec![path],
            Made::Many(paths) => paths.iter_mut().collect(),
        }
    }
}

impl Progress {
    /// The progress in the campaign at `root`, or none if it has no progress file.
    pub fn load(root: &Path) -> Self {
        fs::read_to_string(root.join(PROGRESS_FILE))
            .ok()
            .and_then(|yaml| serde_yaml_ng::from_str(&yaml).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, root: &Path) -> Result<(), String> {
        let path = root.join(PROGRESS_FILE);
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).map_err(|error| error.to_string())?;
        }
        let yaml = serde_yaml_ng::to_string(self).map_err(|error| error.to_string())?;
        fs::write(&path, yaml).map_err(|error| error.to_string())
    }

    /// Whether the question or chapter `key` was skipped.
    pub fn is_skipped(&self, key: &str) -> bool {
        self.skipped.contains(key)
    }

    /// Skips the question or chapter `key`, or with `skip` false, stops skipping it.
    pub fn skip(&mut self, key: &str, skip: bool) {
        if skip {
            self.skipped.insert(key.to_string());
        } else {
            self.skipped.remove(key);
        }
    }

    /// Follows notes renamed or moved in the campaign at `root`, from the first path to the
    /// second. Returns whether any were the walkthrough's.
    pub fn renamed(&mut self, root: &Path, moved: &[(PathBuf, PathBuf)]) -> bool {
        let mut changed = false;
        for (old, new) in moved {
            let (old, new) = (relative(root, old), relative(root, new));
            for made in self.notes.values_mut() {
                for path in made.paths_mut() {
                    if *path == old {
                        *path = new.clone();
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    /// The notes made for the walkthrough's note `name` that are still there. A note
    /// renamed or moved outside the app is found again by its name in its folder; a note
    /// with a set name that the walkthrough hasn't made yet is found by that name.
    pub fn notes(&self, campaign: &Campaign, questions: &Questions, name: &str) -> Vec<PathBuf> {
        let Some(spec) = questions.notes.get(name) else {
            return Vec::new();
        };
        let folder = campaign.folder(spec);
        let in_folder = |file: &str| -> Option<PathBuf> {
            let wanted = Path::new(file)
                .file_name()?
                .to_string_lossy()
                .to_lowercase();
            fs::read_dir(&folder)
                .ok()?
                .flatten()
                .map(|entry| entry.path())
                .find(|path| {
                    path.file_name()
                        .is_some_and(|found| found.to_string_lossy().to_lowercase() == wanted)
                })
        };

        let mut found = Vec::new();
        for path in self.notes.get(name).map(Made::paths).unwrap_or_default() {
            let full = campaign.root.join(&path);
            let path = if full.is_file() {
                Some(full)
            } else {
                in_folder(&path)
            };
            if let Some(path) = path
                && !found.contains(&path)
            {
                found.push(path);
            }
        }
        if found.is_empty()
            && let Some(name) = spec.set_name(campaign)
        {
            found.extend(in_folder(&note_file(&name)));
        }
        found
    }

    fn record(
        &mut self,
        campaign: &Campaign,
        questions: &Questions,
        name: &str,
        paths: &[PathBuf],
    ) {
        let many = questions.notes.get(name).is_some_and(|spec| spec.many);
        let mut paths: Vec<String> = paths.iter().map(|path| campaign.relative(path)).collect();
        if paths.is_empty() {
            self.notes.remove(name);
        } else if many {
            self.notes.insert(name.to_string(), Made::Many(paths));
        } else {
            self.notes
                .insert(name.to_string(), Made::One(paths.remove(0)));
        }
    }

    /// The notes question `question` is asked about: one for each note made for a list of
    /// notes, or else the note it goes into (none if that isn't made yet). A question with
    /// no field is asked once, whatever there is.
    pub fn subjects(
        &self,
        campaign: &Campaign,
        questions: &Questions,
        question: &Question,
    ) -> Vec<PathBuf> {
        let notes = self.notes(campaign, questions, &question.note);
        if question.field.is_none() {
            return notes;
        }
        let many = questions
            .notes
            .get(&question.note)
            .is_some_and(|spec| spec.many);
        if many {
            notes
        } else {
            notes.into_iter().take(1).collect()
        }
    }

    /// What question `question` holds now about note `which` of its [`Self::subjects`]:
    /// for a question with no field the names of the notes made (or the name a note will
    /// get), else what the field holds.
    pub fn current(
        &self,
        campaign: &Campaign,
        questions: &Questions,
        question: &Question,
        which: usize,
    ) -> Result<Current, String> {
        let notes = self.subjects(campaign, questions, question);
        let Some(field) = &question.field else {
            let mut names: Vec<String> = notes.iter().map(|path| name_of(path)).collect();
            if names.is_empty()
                && let Some(name) = questions
                    .notes
                    .get(&question.note)
                    .and_then(|spec| spec.set_name(campaign))
            {
                names.push(name);
            }
            return Ok(Current::Names(names));
        };
        let Some(path) = notes.get(which) else {
            return Ok(Current::NoNote);
        };
        let spec = &questions.notes[&question.note];
        let ty = campaign
            .type_of(spec)
            .ok_or_else(|| format!("{} has no type", spec.folder))?;
        let form = read_form(campaign, ty, path)?;
        Ok(Current::Value(form.value(field)?))
    }

    /// Whether question `question` is answered for every one of its subjects (and has at
    /// least one). A question with no field is answered once its notes are made.
    pub fn answered(
        &self,
        campaign: &Campaign,
        questions: &Questions,
        question: &Question,
    ) -> bool {
        let subjects = self.subjects(campaign, questions, question);
        if question.field.is_none() || subjects.is_empty() {
            return !subjects.is_empty();
        }
        (0..subjects.len()).all(|which| {
            matches!(
                self.current(campaign, questions, question, which),
                Ok(Current::Value(Some(_)))
            )
        })
    }

    /// Writes `answer` to question `question`, about note `which` of its
    /// [`Self::subjects`] for a field question. The progress is changed in memory; save it
    /// after.
    pub fn answer(
        &mut self,
        campaign: &Campaign,
        questions: &Questions,
        question: &Question,
        which: usize,
        answer: Answer,
    ) -> Result<Written, String> {
        let spec = questions
            .notes
            .get(&question.note)
            .ok_or_else(|| format!("there is no note {}", question.note))?;
        let ty = campaign
            .type_of(spec)
            .ok_or_else(|| format!("the folder {} has no type", spec.folder))?
            .clone();
        let mut written = Written::default();

        match (&question.field, answer) {
            (None, Answer::Names(names)) => {
                let current = self.notes(campaign, questions, &question.note);
                let mut kept: Vec<PathBuf> = Vec::new();
                let mut seen = HashSet::new();
                for name in names.iter().map(|name| name.trim()) {
                    if name.is_empty() || !seen.insert(name.to_lowercase()) {
                        continue;
                    }
                    if !spec.many && !kept.is_empty() {
                        break;
                    }
                    match current
                        .iter()
                        .find(|path| name_of(path).eq_ignore_ascii_case(name))
                    {
                        Some(path) => kept.push(path.clone()),
                        None if !spec.many && !current.is_empty() => {
                            // A note's new name: it is renamed, links and all.
                            written.rename = Some((current[0].clone(), name.to_string()));
                            kept.push(current[0].clone());
                        }
                        None => {
                            let path =
                                self.make(campaign, questions, spec, &ty, name, &mut written)?;
                            kept.push(path);
                        }
                    }
                }
                self.record(campaign, questions, &question.note, &kept);
            }
            (Some(key), Answer::Value(value)) => {
                let notes = self.subjects(campaign, questions, question);
                let path = match notes.get(which) {
                    Some(path) => path.clone(),
                    // A note with a set name is made by its first answer.
                    None => match spec.set_name(campaign) {
                        Some(name) if which == 0 => {
                            let path =
                                self.make(campaign, questions, spec, &ty, &name, &mut written)?;
                            self.record(
                                campaign,
                                questions,
                                &question.note,
                                std::slice::from_ref(&path),
                            );
                            path
                        }
                        _ => {
                            return Err(format!(
                                "There is no {} to write this into yet",
                                ty.label()
                            ));
                        }
                    },
                };
                let path = &path;
                let mut form = read_form(campaign, &ty, path)?;
                if !form.set_value(key, value.as_ref()) {
                    return Err(format!("{} has no field {key}", ty.label()));
                }
                write_form(&form, path)?;
                written.changed.push(path.clone());
                if let Some(field) = form.field(key) {
                    let field = field.clone();
                    make_linked(campaign, &field, value.as_ref(), path, &mut written)?;
                }
            }
            (None, Answer::Value(_)) => return Err("This question asks for names".into()),
            (Some(_), Answer::Names(_)) => return Err("This question asks for a value".into()),
        }
        Ok(written)
    }

    /// Makes (or finds, if there is one) the note called `name` for `spec`, filling in its
    /// set values and linking it from the note it is added to.
    fn make(
        &mut self,
        campaign: &Campaign,
        questions: &Questions,
        spec: &NoteSpec,
        ty: &ContentType,
        name: &str,
        written: &mut Written,
    ) -> Result<PathBuf, String> {
        campaign::validate_name(name)?;
        let folder = campaign.folder(spec);
        let path = folder.join(note_file(name));
        if path.is_file() {
            // A note of that name is used rather than making a second.
            return Ok(path);
        }
        let path = crate::create_linked_note(campaign.root, name, Some(&folder), Some(ty))?;
        written.made.push(path.clone());

        let mut form = read_form(campaign, ty, &path)?;
        for (key, value) in &spec.set {
            let key = note::value_to_text(key);
            let value = match value {
                Value::String(other) if questions.notes.contains_key(other) => {
                    let linked = self.notes(campaign, questions, other);
                    let Some(field) = form.field(&key) else {
                        continue;
                    };
                    match link_value(field.kind, &linked) {
                        Some(value) => value,
                        None => continue,
                    }
                }
                value => value.clone(),
            };
            form.set_value(&key, Some(&value));
        }
        write_form(&form, &path)?;

        if let Some(add_to) = &spec.add_to
            && let Some(target) = self.notes(campaign, questions, &add_to.note).first()
            && let Some(target_spec) = questions.notes.get(&add_to.note)
            && let Some(target_ty) = campaign.type_of(target_spec)
        {
            let mut form = read_form(campaign, target_ty, target)?;
            let link = Value::String(format!("[[{}]]", stem(&path)));
            let value = match form.field(&add_to.field).map(|field| field.kind) {
                Some(Kind::Links) => {
                    let mut items = match form.value(&add_to.field)? {
                        Some(Value::Sequence(items)) => items,
                        _ => Vec::new(),
                    };
                    (!items.contains(&link)).then(|| {
                        items.push(link);
                        Value::Sequence(items)
                    })
                }
                Some(Kind::Link) => Some(link),
                _ => None,
            };
            if let Some(value) = value {
                form.set_value(&add_to.field, Some(&value));
                write_form(&form, target)?;
                written.changed.push(target.clone());
            }
        }
        Ok(path)
    }
}

/// What a question holds now, from [`Progress::current`].
#[derive(Debug, Clone, PartialEq)]
pub enum Current {
    /// The names of the notes made (or the set name a note will get).
    Names(Vec<String>),
    /// What the field holds, `None` if it is empty.
    Value(Option<Value>),
    /// The note the question fills in isn't made yet.
    NoNote,
}

/// An answer to a question.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// For a question with no field: the note's name, or the names of the notes to make.
    Names(Vec<String>),
    /// For a field: its value, as [`Form::value`] gives it.
    Value(Option<Value>),
}

/// What writing an answer did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Written {
    /// Notes made.
    pub made: Vec<PathBuf>,
    /// Notes changed that were there before.
    pub changed: Vec<PathBuf>,
    /// A note to rename, to the name given. Renaming rewrites links to it across the
    /// campaign, so the app does it, and the progress follows ([`Progress::renamed`]).
    pub rename: Option<(PathBuf, String)>,
}

/// A value of a field of `kind` that links to the notes at `paths`, if it can.
fn link_value(kind: Kind, paths: &[PathBuf]) -> Option<Value> {
    let link = |path: &PathBuf| Value::String(format!("[[{}]]", stem(path)));
    match kind {
        Kind::Link => paths.first().map(link),
        Kind::Links if !paths.is_empty() => Some(Value::Sequence(paths.iter().map(link).collect())),
        _ => None,
    }
}

/// Makes a note for each `[[link]]` in `value`, the new value of `field` in the note at
/// `from`, that names no note yet, in the folder the field links to.
pub fn make_linked(
    campaign: &Campaign,
    field: &Field,
    value: Option<&Value>,
    from: &Path,
    written: &mut Written,
) -> Result<(), String> {
    // Long text only links to notes; it doesn't make them, as in the form.
    if !matches!(field.kind, Kind::Link | Kind::Links | Kind::Steps) {
        return Ok(());
    }
    let Some(value) = value else {
        return Ok(());
    };
    let text = match (field.kind, value) {
        // A link field holds the name, with or without its brackets.
        (Kind::Link, Value::String(name)) if !name.contains("[[") => format!("[[{name}]]"),
        (_, Value::Sequence(items)) => items
            .iter()
            .map(note::value_to_text)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => note::value_to_text(value),
    };

    let folder = types::link_folder(campaign.root, campaign.types, field);
    let ty = folder
        .as_ref()
        .and_then(|folder| campaign.types.get(folder)?.as_ref().ok());
    for link in links::find(&text) {
        let target = text[link.target].trim();
        let made_now = written
            .made
            .iter()
            .any(|path| stem(path).eq_ignore_ascii_case(target));
        if target.is_empty() || made_now || campaign.index.resolve(target, from).is_some() {
            continue;
        }
        let path = crate::create_linked_note(campaign.root, target, folder.as_deref(), ty)?;
        written.made.push(path);
    }
    Ok(())
}

/// The file name of a note called `name`.
fn note_file(name: &str) -> String {
    format!("{}.md", campaign::note_file_name(name))
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The name of the note at `path`: its name field, or else its file name.
pub fn name_of(path: &Path) -> String {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| note::parse(&text).ok())
        .and_then(|note| note.front.get("name").map(note::value_to_text))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| stem(path))
}

/// The note at `path` read as a form of type `ty`.
fn read_form(campaign: &Campaign, ty: &ContentType, path: &Path) -> Result<Form, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("{} can't be read: {error}", campaign.relative(path)))?;
    let note = note::parse(&text)
        .map_err(|error| format!("{} can't be read: {error}", campaign.relative(path)))?;
    Ok(Form::from_note(ty.clone(), note))
}

fn write_form(form: &Form, path: &Path) -> Result<(), String> {
    let text = form.to_markdown()?;
    fs::write(path, text).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::Structure;

    /// A new campaign with the starter folders, in a folder of its own.
    struct Test {
        root: PathBuf,
        types: Types,
    }

    impl Test {
        fn new(name: &str) -> Self {
            let parent = std::env::temp_dir().join(format!(
                "dungeon_elements_walkthrough_{name}_{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&parent);
            fs::create_dir_all(&parent).unwrap();
            let root = campaign::create(&parent, "Ashes", &Structure::starter()).unwrap();
            let mut test = Test {
                root,
                types: Types::new(),
            };
            test.read_types();
            test
        }

        fn read_types(&mut self) {
            let mut folders = Vec::new();
            let mut own = Types::new();
            fn walk(folder: &Path, folders: &mut Vec<PathBuf>, own: &mut Types) {
                let mut entries: Vec<PathBuf> = fs::read_dir(folder)
                    .unwrap()
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.is_dir())
                    .collect();
                entries.sort();
                for path in entries {
                    if let Some(ty) = types::load(&path) {
                        own.insert(path.clone(), ty);
                    }
                    folders.push(path.clone());
                    walk(&path, folders, own);
                }
            }
            walk(&self.root, &mut folders, &mut own);
            self.types = types::resolve(&self.root, &folders, &own);
        }

        fn index(&self) -> links::Index {
            let mut notes = Vec::new();
            fn walk(folder: &Path, notes: &mut Vec<PathBuf>) {
                for entry in fs::read_dir(folder).unwrap().flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        walk(&path, notes);
                    } else if campaign::is_markdown(&path) {
                        notes.push(path);
                    }
                }
            }
            walk(&self.root, &mut notes);
            links::Index::new(&self.root, notes)
        }

        fn read(&self, path: &str) -> String {
            fs::read_to_string(self.root.join(path)).unwrap()
        }
    }

    impl Drop for Test {
        fn drop(&mut self) {
            if let Some(parent) = self.root.parent() {
                let _ = fs::remove_dir_all(parent);
            }
        }
    }

    fn question<'a>(questions: &'a Questions, key: &str) -> &'a Question {
        questions
            .all()
            .map(|(_, question)| question)
            .find(|question| question.key() == key)
            .unwrap_or_else(|| panic!("no question {key}"))
    }

    fn names(names: &[&str]) -> Answer {
        Answer::Names(names.iter().map(|name| name.to_string()).collect())
    }

    fn text(value: &str) -> Answer {
        Answer::Value(Some(Value::String(value.into())))
    }

    #[test]
    fn built_in_questions_can_be_read() {
        let questions = Questions::built_in();
        assert_eq!(questions.chapters.len(), 7);
        assert_eq!(questions.chapters[0].id, "campaign");
        assert!(questions.all().count() > 30);
    }

    #[test]
    fn every_built_in_question_fits_the_starter_folders() {
        let test = Test::new("fits");
        let index = test.index();
        let campaign = Campaign {
            root: &test.root,
            types: &test.types,
            index: &index,
        };
        let questions = Questions::built_in();
        let fitting = questions.fitting(&campaign);
        let left_out: Vec<String> = questions
            .all()
            .filter(|(_, question)| !fitting.all().any(|(_, fits)| fits == *question))
            .map(|(_, question)| question.key())
            .collect();
        assert!(left_out.is_empty(), "left out: {left_out:?}");

        // Every set value and added link names a field the note's type has.
        for (name, spec) in &questions.notes {
            let ty = campaign.type_of(spec).unwrap();
            for key in spec.set.keys() {
                let key = note::value_to_text(key);
                assert!(
                    ty.fields.iter().any(|field| field.key == key),
                    "{name} sets {key}"
                );
            }
            if let Some(add_to) = &spec.add_to {
                let target = campaign.type_of(&questions.notes[&add_to.note]).unwrap();
                assert!(target.fields.iter().any(|field| field.key == add_to.field));
            }
        }

        // Every folder offered to pick from is a starter folder.
        for (_, question) in questions.all() {
            for folder in &question.pick {
                assert!(
                    test.root.join(folder).is_dir(),
                    "{} picks {folder}",
                    question.key()
                );
            }
        }
    }

    #[test]
    fn questions_whose_folder_or_field_is_missing_are_left_out() {
        let mut test = Test::new("missing");
        fs::remove_dir_all(test.root.join("lore")).unwrap();
        fs::write(
            test.root.join("factions/_type.yaml"),
            "type: faction\nlabel: Faction\nfields:\n  - key: name\n  - key: goal\n",
        )
        .unwrap();
        test.read_types();
        let index = test.index();
        let campaign = Campaign {
            root: &test.root,
            types: &test.types,
            index: &index,
        };
        let fitting = Questions::built_in().fitting(&campaign);
        let keys: Vec<String> = fitting.all().map(|(_, question)| question.key()).collect();

        assert!(!keys.iter().any(|key| key.starts_with("magic")));
        assert!(
            !fitting
                .chapters
                .iter()
                .any(|chapter| chapter.id == "extraordinary")
        );
        assert!(keys.contains(&"factions.goal".to_string()));
        assert!(!keys.contains(&"factions.plan".to_string()));
    }

    #[test]
    fn bad_questions_say_what_is_wrong() {
        let error = parse(
            "chapters:\n  - title: One\n    questions:\n      - ask: Who?\n        note: hero\n",
        )
        .unwrap_err();
        assert!(error.contains("isn't one of the notes"), "{error}");

        let error = parse(
            "notes:\n  hero: { folder: players }\nchapters:\n  - title: One\n    questions:\n      - { ask: A, note: hero }\n      - { ask: B, note: hero }\n",
        )
        .unwrap_err();
        assert!(error.contains("two questions fill hero"), "{error}");

        let error = parse(
            "notes:\n  hero: { folder: players, add_to: { note: party, field: members } }\nchapters: []\n",
        )
        .unwrap_err();
        assert!(error.contains("party"), "{error}");
    }

    #[test]
    fn a_name_makes_the_note_with_its_set_values() {
        let test = Test::new("name");
        let index = test.index();
        let campaign = Campaign {
            root: &test.root,
            types: &test.types,
            index: &index,
        };
        let questions = Questions::built_in();
        let mut progress = Progress::default();

        let written = progress
            .answer(
                &campaign,
                &questions,
                question(&questions, "world"),
                0,
                names(&["Eldmoor"]),
            )
            .unwrap();
        assert_eq!(written.made, vec![test.root.join("locations/Eldmoor.md")]);
        let text = test.read("locations/Eldmoor.md");
        assert!(text.contains("type: location"), "{text}");
        assert!(text.contains("name: Eldmoor"), "{text}");
        assert!(text.contains("kind: World"), "{text}");
        assert_eq!(
            progress.notes.get("world"),
            Some(&Made::One("locations/Eldmoor.md".into()))
        );

        // Regions are part of the world.
        progress
            .answer(
                &campaign,
                &questions,
                question(&questions, "regions"),
                0,
                names(&["The Reach", "Saltmarsh"]),
            )
            .unwrap();
        let text = test.read("locations/Saltmarsh.md");
        assert!(text.contains("kind: Region"), "{text}");
        assert!(text.contains("part_of: '[[Eldmoor]]'"), "{text}");
    }

    #[test]
    fn a_list_answer_makes_a_note_for_each_and_follow_ups_go_through_them() {
        let test = Test::new("list");
        let index = test.index();
        let campaign = Campaign {
            root: &test.root,
            types: &test.types,
            index: &index,
        };
        let questions = Questions::built_in();
        let mut progress = Progress::default();
        let factions = question(&questions, "factions");
        let goal = question(&questions, "factions.goal");

        assert!(!progress.answered(&campaign, &questions, factions));
        progress
            .answer(
                &campaign,
                &questions,
                factions,
                0,
                names(&["Iron Guild", "Pale Court", "iron guild"]),
            )
            .unwrap();
        assert!(progress.answered(&campaign, &questions, factions));

        let subjects = progress.subjects(&campaign, &questions, goal);
        assert_eq!(
            subjects,
            vec![
                test.root.join("factions/Iron Guild.md"),
                test.root.join("factions/Pale Court.md")
            ]
        );
        assert_eq!(
            goal.ask_about(Some("Iron Guild")),
            "What do the Iron Guild want?"
        );
        assert_eq!(
            goal.ask_about(Some("The Pale Court")),
            "What do The Pale Court want?"
        );

        progress
            .answer(&campaign, &questions, goal, 1, text("The crown"))
            .unwrap();
        assert!(
            test.read("factions/Pale Court.md")
                .contains("goal: The crown")
        );
        assert_eq!(
            progress.current(&campaign, &questions, goal, 1),
            Ok(Current::Value(Some(Value::String("The crown".into()))))
        );
        // One of two answered isn't the whole question.
        assert!(!progress.answered(&campaign, &questions, goal));

        // Taking a name out of the list keeps its note, but drops it from the walkthrough.
        progress
            .answer(&campaign, &questions, factions, 0, names(&["Pale Court"]))
            .unwrap();
        assert!(test.root.join("factions/Iron Guild.md").is_file());
        assert_eq!(progress.subjects(&campaign, &questions, goal).len(), 1);
        assert!(progress.answered(&campaign, &questions, goal));
    }

    #[test]
    fn a_link_answer_makes_the_note_it_names() {
        let test = Test::new("link");
        let index = test.index();
        let campaign = Campaign {
            root: &test.root,
            types: &test.types,
            index: &index,
        };
        let questions = Questions::built_in();
        let mut progress = Progress::default();
        progress
            .answer(
                &campaign,
                &questions,
                question(&questions, "factions"),
                0,
                names(&["Iron Guild"]),
            )
            .unwrap();

        let written = progress
            .answer(
                &campaign,
                &questions,
                question(&questions, "factions.leader"),
                0,
                text("Mara Voss"),
            )
            .unwrap();
        assert_eq!(written.made, vec![test.root.join("npcs/Mara Voss.md")]);
        assert!(test.read("npcs/Mara Voss.md").contains("type: npc"));
        assert!(
            test.read("factions/Iron Guild.md")
                .contains("leader: '[[Mara Voss]]'")
        );
    }

    #[test]
    fn made_notes_are_added_to_the_note_named() {
        let test = Test::new("add_to");
        let index = test.index();
        let campaign = Campaign {
            root: &test.root,
            types: &test.types,
            index: &index,
        };
        let questions = Questions::built_in();
        let mut progress = Progress::default();

        // The campaign note starts with the campaign's title.
        let name = question(&questions, "campaign");
        assert_eq!(
            progress.current(&campaign, &questions, name, 0),
            Ok(Current::Names(vec!["Ashes".into()]))
        );
        progress
            .answer(&campaign, &questions, name, 0, names(&["Ashes"]))
            .unwrap();
        progress
            .answer(
                &campaign,
                &questions,
                question(&questions, "start"),
                0,
                names(&["Brindle"]),
            )
            .unwrap();
        progress
            .answer(
                &campaign,
                &questions,
                question(&questions, "heroes"),
                0,
                names(&["Kestrel", "Odo"]),
            )
            .unwrap();

        let text = test.read("campaign/Ashes.md");
        assert!(text.contains("home_base: '[[Brindle]]'"), "{text}");
        assert!(text.contains("- '[[Kestrel]]'\n- '[[Odo]]'"), "{text}");
        // Heroes are from the starting town.
        assert!(test.read("players/Odo.md").contains("home: '[[Brindle]]'"));
    }

    #[test]
    fn a_note_with_a_set_name_is_made_by_its_first_answer() {
        let test = Test::new("set_name");
        let index = test.index();
        let campaign = Campaign {
            root: &test.root,
            types: &test.types,
            index: &index,
        };
        let questions = Questions::built_in();
        let mut progress = Progress::default();
        let opening = question(&questions, "first_session.strong_start");
        assert_eq!(
            progress.current(&campaign, &questions, opening, 0),
            Ok(Current::NoNote)
        );

        let written = progress
            .answer(&campaign, &questions, opening, 0, text("An ambush"))
            .unwrap();
        assert_eq!(written.made, [test.root.join("sessions/Session 1.md")]);
        let text = test.read("sessions/Session 1.md");
        assert!(text.contains("status: Planned"), "{text}");
        assert!(text.contains("An ambush"), "{text}");
        assert!(progress.answered(&campaign, &questions, opening));
    }

    #[test]
    fn a_new_name_for_a_made_note_renames_it() {
        let test = Test::new("rename");
        let index = test.index();
        let campaign = Campaign {
            root: &test.root,
            types: &test.types,
            index: &index,
        };
        let questions = Questions::built_in();
        let mut progress = Progress::default();
        let world = question(&questions, "world");
        progress
            .answer(&campaign, &questions, world, 0, names(&["Eldmoor"]))
            .unwrap();

        let written = progress
            .answer(&campaign, &questions, world, 0, names(&["Elder Moor"]))
            .unwrap();
        let old = test.root.join("locations/Eldmoor.md");
        assert_eq!(written.rename, Some((old.clone(), "Elder Moor".into())));

        // The app renames it, and the progress follows.
        let new = test.root.join("locations/Elder Moor.md");
        fs::rename(&old, &new).unwrap();
        assert!(progress.renamed(&test.root, &[(old, new.clone())]));
        assert_eq!(progress.notes(&campaign, &questions, "world"), vec![new]);
    }

    #[test]
    fn notes_renamed_outside_the_app_are_found_by_name() {
        let test = Test::new("outside");
        let index = test.index();
        let campaign = Campaign {
            root: &test.root,
            types: &test.types,
            index: &index,
        };
        let questions = Questions::built_in();
        let mut progress = Progress {
            notes: BTreeMap::from([("world".into(), Made::One("locations/eldmoor.md".into()))]),
            ..Progress::default()
        };
        fs::write(
            test.root.join("locations/Eldmoor.md"),
            "---\ntype: location\n---\n",
        )
        .unwrap();
        assert_eq!(
            progress.notes(&campaign, &questions, "world"),
            vec![test.root.join("locations/Eldmoor.md")]
        );

        progress
            .notes
            .insert("world".into(), Made::One("locations/Gone.md".into()));
        assert!(progress.notes(&campaign, &questions, "world").is_empty());
    }

    #[test]
    fn progress_is_saved_and_read_back() {
        let test = Test::new("progress");
        let mut progress = Progress::default();
        progress
            .notes
            .insert("world".into(), Made::One("locations/Eldmoor.md".into()));
        progress.notes.insert(
            "factions".into(),
            Made::Many(vec!["factions/A.md".into(), "factions/B.md".into()]),
        );
        progress.skip("magic", true);
        progress.at = Some("power.goal".into());
        progress.save(&test.root).unwrap();

        let yaml = test.read(".dungeon_elements/walkthrough.yaml");
        assert!(yaml.contains("world: locations/Eldmoor.md"), "{yaml}");
        assert_eq!(Progress::load(&test.root), progress);

        progress.skip("magic", false);
        assert!(!progress.is_skipped("magic"));
    }

    #[test]
    fn inspire_me_uses_the_question_then_the_field_then_action_and_theme() {
        let questions = Questions::built_in();
        let npc = types::starter("npcs").unwrap();
        let wants = npc
            .fields
            .iter()
            .find(|field| field.key == "wants")
            .unwrap();
        let status = npc
            .fields
            .iter()
            .find(|field| field.key == "status")
            .unwrap();
        let notes = npc
            .fields
            .iter()
            .find(|field| field.key == "notes")
            .unwrap();

        let locals = question(&questions, "locals");
        assert_eq!(locals.tables(None, &npc), ["Female names", "Male names"]);

        let mut ask = question(&questions, "locals.wants").clone();
        ask.table = None;
        assert_eq!(ask.tables(Some(wants), &npc), ["Wants"]);
        assert_eq!(ask.tables(Some(notes), &npc), [FALLBACK_TABLE]);
        assert!(ask.tables(Some(status), &npc).is_empty());
        ask.table = Some(String::new());
        assert!(ask.tables(Some(notes), &npc).is_empty());
    }

    #[test]
    fn a_campaign_can_have_its_own_questions() {
        let test = Test::new("own");
        fs::write(
            test.root.join(FILE),
            "notes:\n  hero: { folder: players, many: true }\nchapters:\n  - title: Heroes\n    questions:\n      - { ask: Who?, note: hero }\n",
        )
        .unwrap();
        let questions = Questions::load(&test.root).unwrap();
        assert_eq!(questions.chapters[0].id, "heroes");

        fs::write(test.root.join(FILE), "chapters: [").unwrap();
        assert!(Questions::load(&test.root).unwrap_err().starts_with(FILE));
    }
}
