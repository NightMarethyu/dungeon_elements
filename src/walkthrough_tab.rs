//! The walkthrough tab: the walkthrough's chapters on the left, and one question at a time
//! on the right with its answer box, Back, Skip and Next.
//!
//! The answer box is the input of the field the question fills, from a form of the note
//! the answer goes into, so a date shows the calendar and a dice field the dice picker.
//! Questions that ask for a name get a form of their own with one field, and questions
//! that ask for a list of names a box for each, with Add Another. The app writes the answers (see
//! [`crate::walkthrough`]); this keeps the GM's place and draws the tab.

use std::path::{Path, PathBuf};

use iced::widget::{button, column, container, row, rule, scrollable, space, text, text_input};
use iced::{Center, Element, Fill};
use serde_yaml_ng::Value;

use crate::form::Form;
use crate::note;
use crate::style;
use crate::theme::Colors;
use crate::types::{ContentType, Field, Kind, Width};
use crate::walkthrough::{self, Answer, Campaign, Current, Progress, Question, Questions, Written};

/// The key of the one field of a form for names.
const NAMES_KEY: &str = "name";

#[derive(Debug, Clone)]
pub enum Message {
    /// Opens a chapter at its first question.
    Chapter(usize),
    Back,
    Next,
    Skip,
    SkipChapter,
    /// Opens a note the walkthrough made.
    Open(PathBuf),
    /// Rolls on a table for ideas and puts the roll in the answer.
    Inspire(String),
    /// Name `index` of a list of names was typed in.
    Name(usize, String),
    /// Text was pasted into name `index`, which is now this. The app then reads the
    /// clipboard for [`Message::PastedNames`], since a one-line box drops the line breaks.
    PasteNames(usize, String),
    /// What was pasted into name `index`, which held the first text before. A list fills
    /// a box for each name.
    PastedNames(usize, String, String),
    /// Enter in name `index`: the next box, a new one, or after an empty last box the next
    /// question.
    NameEntered(usize),
    AddName,
    RemoveName(usize),
    /// Links the answer to a note picked from the notes offered, by its link text.
    Pick(String),
    /// Makes a note named this in the folder, and links the answer to it.
    Make(PathBuf, String),
}

/// What the app does after a list of names changed.
pub enum NamesDone {
    /// Puts the keyboard in name box `index`.
    Focus(usize),
    /// Reads the clipboard for what was pasted in name box `index`, which held this.
    ReadPaste(usize, String),
    /// Goes to the next question.
    Next,
}

/// The widget id of name box `index` of a list of names.
pub fn name_id(index: usize) -> String {
    format!("walkthrough-name-{index}")
}

/// A place in the walkthrough.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Question `question` of chapter `chapter`, about note `which` of its subjects.
    Question {
        chapter: usize,
        question: usize,
        which: usize,
    },
    /// The end of a chapter, listing the notes it made.
    Summary(usize),
    /// The end of the walkthrough.
    Done,
}

/// How far a chapter has got.
#[derive(Debug, Clone, Default)]
struct ChapterState {
    answered: usize,
    total: usize,
    skipped: bool,
}

pub struct Walkthrough {
    /// The tab's id, among the note tabs' ids.
    pub id: u64,
    /// The questions that fit the campaign.
    questions: Questions,
    pub progress: Progress,
    steps: Vec<Step>,
    at: usize,
    chapters: Vec<ChapterState>,
    /// The answer being typed: a form of the note it goes into, or of the name asked for.
    pub form: Option<Form>,
    /// The names being typed, for a question that asks for a list of them.
    names: Option<Vec<String>>,
    /// The label over the names.
    names_label: String,
    /// The field of `form` that holds the answer.
    pub field: Option<usize>,
    /// The note the answer goes into, if it is made.
    pub subject: Option<PathBuf>,
    subject_name: Option<String>,
    /// Tables to roll on for ideas, shown as Inspire me buttons under the answer.
    tables: Vec<String>,
    /// The notes made in the chapter (or the whole walkthrough) a summary is for.
    made: Vec<(String, PathBuf)>,
    /// What the answer was when the question was shown, so an answer left as it was
    /// isn't written again.
    loaded: Option<Answer>,
    pub error: Option<String>,
}

impl Walkthrough {
    /// Opens the walkthrough of the campaign where the GM left off, or at the first
    /// question that has no answer. `open_text` gives the text of a note as its open tab
    /// has it, unsaved changes and all, if it is open.
    pub fn open(
        id: u64,
        campaign: &Campaign,
        open_text: &dyn Fn(&Path) -> Option<String>,
    ) -> Result<Self, String> {
        let questions = Questions::load(campaign.root)?.fitting(campaign);
        if questions.chapters.is_empty() {
            return Err(
                "None of the walkthrough's questions fit this campaign's folders and types".into(),
            );
        }
        let mut walkthrough = Walkthrough {
            id,
            questions,
            progress: Progress::load(campaign.root),
            steps: Vec::new(),
            at: 0,
            chapters: Vec::new(),
            form: None,
            names: None,
            names_label: String::new(),
            field: None,
            subject: None,
            subject_name: None,
            tables: Vec::new(),
            made: Vec::new(),
            loaded: None,
            error: None,
        };
        walkthrough.steps = walkthrough.find_steps(campaign);
        let saved = walkthrough.progress.at.clone();
        walkthrough.at = saved
            .and_then(|at| {
                walkthrough
                    .steps
                    .iter()
                    .position(|step| walkthrough.step_key(step) == at)
            })
            .unwrap_or_else(|| walkthrough.first_unanswered(campaign));
        walkthrough.refresh(campaign, open_text);
        Ok(walkthrough)
    }

    /// Reads everything again from the notes, keeping the GM's place.
    pub fn refresh(&mut self, campaign: &Campaign, open_text: &dyn Fn(&Path) -> Option<String>) {
        let step = self.step().clone();
        self.steps = self.find_steps(campaign);
        self.at = self
            .steps
            .iter()
            .position(|found| *found == step)
            .or_else(|| {
                // The note it was about is gone: the chapter's next step.
                let chapter = chapter_of(&step)?;
                self.steps
                    .iter()
                    .position(|found| chapter_of(found).is_some_and(|found| found >= chapter))
            })
            .unwrap_or(self.steps.len() - 1);
        self.chapters = self
            .questions
            .chapters
            .iter()
            .map(|chapter| ChapterState {
                answered: chapter
                    .questions
                    .iter()
                    .filter(|question| self.progress.answered(campaign, &self.questions, question))
                    .count(),
                total: chapter.questions.len(),
                skipped: self.progress.is_skipped(&chapter.id),
            })
            .collect();
        self.load_step(campaign, open_text);
    }

    /// Every place in the walkthrough, in order: each question once, or once for each
    /// note of a list it asks about, then each chapter's summary, then the end.
    fn find_steps(&self, campaign: &Campaign) -> Vec<Step> {
        let mut steps = Vec::new();
        for (chapter, questions) in self.questions.chapters.iter().enumerate() {
            for (index, question) in questions.questions.iter().enumerate() {
                let many = self
                    .questions
                    .notes
                    .get(&question.note)
                    .is_some_and(|spec| spec.many);
                let count = if question.field.is_some() && many {
                    self.progress
                        .subjects(campaign, &self.questions, question)
                        .len()
                } else {
                    1
                };
                steps.extend((0..count).map(|which| Step::Question {
                    chapter,
                    question: index,
                    which,
                }));
            }
            steps.push(Step::Summary(chapter));
        }
        steps.push(Step::Done);
        steps
    }

    /// The first question with no answer that wasn't skipped, or else the end.
    fn first_unanswered(&self, campaign: &Campaign) -> usize {
        self.steps
            .iter()
            .position(|step| match step {
                Step::Question {
                    chapter, question, ..
                } => {
                    let chapter = &self.questions.chapters[*chapter];
                    let question = &chapter.questions[*question];
                    !self.progress.is_skipped(&chapter.id)
                        && !self.progress.is_skipped(&question.key())
                        && !self.progress.answered(campaign, &self.questions, question)
                }
                _ => false,
            })
            .unwrap_or(self.steps.len() - 1)
    }

    /// What progress calls a step.
    fn step_key(&self, step: &Step) -> String {
        match step {
            Step::Question {
                chapter,
                question,
                which,
            } => {
                let chapter = &self.questions.chapters[*chapter];
                format!(
                    "{}/{}/{which}",
                    chapter.id,
                    chapter.questions[*question].key()
                )
            }
            Step::Summary(chapter) => format!("{}/done", self.questions.chapters[*chapter].id),
            Step::Done => "done".into(),
        }
    }

    pub fn step(&self) -> &Step {
        &self.steps[self.at]
    }

    /// The question being asked, and which of its notes it is about.
    pub fn question(&self) -> Option<(&Question, usize)> {
        match self.step() {
            Step::Question {
                chapter,
                question,
                which,
            } => Some((
                &self.questions.chapters[*chapter].questions[*question],
                *which,
            )),
            _ => None,
        }
    }

    fn chapter(&self) -> Option<usize> {
        chapter_of(self.step())
    }

    /// Reads the answer box for the step: a form of the note the answer goes into, or of
    /// the names asked for.
    fn load_step(&mut self, campaign: &Campaign, open_text: &dyn Fn(&Path) -> Option<String>) {
        self.progress.at = Some(self.step_key(self.step()));
        self.form = None;
        self.names = None;
        self.field = None;
        self.subject = None;
        self.subject_name = None;
        self.tables = Vec::new();
        self.made = Vec::new();
        self.loaded = None;

        let Some((question, which)) = self.question() else {
            // A summary lists the notes made in its chapter, the end every note made.
            let chapters: Vec<usize> = match self.step() {
                Step::Summary(chapter) => vec![*chapter],
                _ => (0..self.questions.chapters.len()).collect(),
            };
            let mut notes: Vec<&str> = Vec::new();
            for chapter in chapters {
                for question in &self.questions.chapters[chapter].questions {
                    if !notes.contains(&question.note.as_str()) {
                        notes.push(&question.note);
                    }
                }
            }
            let mut made = Vec::new();
            for name in notes {
                for path in self.progress.notes(campaign, &self.questions, name) {
                    if !made.iter().any(|(_, found)| *found == path) {
                        made.push((walkthrough::name_of(&path), path));
                    }
                }
            }
            self.made = made;
            return;
        };
        let question = question.clone();
        let Some(spec) = self.questions.notes.get(&question.note) else {
            return;
        };
        let Some(ty) = campaign.type_of(spec).cloned() else {
            self.error = Some(format!("The folder {} has no type", spec.folder));
            return;
        };

        let Some(key) = &question.field else {
            let names = match self
                .progress
                .current(campaign, &self.questions, &question, 0)
            {
                Ok(Current::Names(names)) => names,
                _ => Vec::new(),
            };
            self.tables = question.tables(None, &ty);
            if spec.many {
                self.names_label = "Names".into();
                let mut boxes = names;
                // An empty box to type the next one in.
                boxes.push(String::new());
                self.names = Some(boxes);
            } else {
                self.form = Some(names_form(&ty, &names));
                self.field = Some(0);
            }
            // A set name the note hasn't got yet is written when Next is pressed.
            let made = self
                .progress
                .notes(campaign, &self.questions, &question.note)
                .iter()
                .map(|path| walkthrough::name_of(path))
                .collect();
            self.loaded = Some(Answer::Names(made));
            return;
        };

        let subjects = self.progress.subjects(campaign, &self.questions, &question);
        let Some(field) = ty.fields.iter().find(|field| &field.key == key) else {
            return;
        };
        let path = subjects.get(which);
        let note = match path {
            Some(path) => {
                let text = open_text(path).or_else(|| std::fs::read_to_string(path).ok());
                match text.as_deref().map(note::parse) {
                    Some(Ok(note)) => Some(note),
                    Some(Err(error)) => {
                        self.error =
                            Some(format!("{} can't be read: {error}", campaign_name(path)));
                        return;
                    }
                    None => {
                        self.error = Some(format!("{} can't be read", campaign_name(path)));
                        return;
                    }
                }
            }
            // A note with a set name is made by its first answer, so it is asked anyway.
            None => match spec.set_name(campaign) {
                Some(name) if which == 0 => {
                    self.subject_name = Some(name);
                    None
                }
                _ => return,
            },
        };
        self.tables = question.tables(Some(field), &ty);
        // The tables show as Inspire me buttons, not as the field's own die buttons too.
        let mut shown = ty.clone();
        if let Some(field) = shown.fields.iter_mut().find(|field| &field.key == key) {
            field.table = None;
        }
        let mut form = match note {
            Some(note) => Form::from_note(shown, note),
            None => Form::blank(shown),
        };
        form.lock();
        self.field = form.index_of(key);
        self.form = Some(form);
        if let Some(path) = path {
            self.subject_name = Some(walkthrough::name_of(path));
            self.subject = Some(path.clone());
        }
        self.loaded = self.answer();
    }

    /// The answer typed in the answer box, if it has one.
    pub fn answer(&self) -> Option<Answer> {
        let (question, _) = self.question()?;
        if let Some(names) = &self.names {
            return Some(Answer::Names(
                names
                    .iter()
                    .map(|name| name.trim())
                    .filter(|name| !name.is_empty())
                    .map(str::to_string)
                    .collect(),
            ));
        }
        let form = self.form.as_ref()?;
        match &question.field {
            None => {
                let names = match form.value(NAMES_KEY).ok().flatten() {
                    Some(value) => note::value_to_text(&value),
                    None => String::new(),
                };
                let names: Vec<String> = names
                    .lines()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_string)
                    .collect();
                Some(Answer::Names(names))
            }
            Some(key) => Some(Answer::Value(form.value(key).ok()?)),
        }
    }

    /// Whether `answer` leaves things as they were, so there is nothing to write. An
    /// empty list of names leaves the notes made as they are.
    pub fn unchanged(&self, answer: &Answer) -> bool {
        match answer {
            Answer::Names(names) if names.is_empty() => true,
            answer => self.loaded.as_ref() == Some(answer),
        }
    }

    /// Writes `answer` to the question being asked, which counts as answered from then on.
    pub fn write(&mut self, campaign: &Campaign, answer: Answer) -> Result<Written, String> {
        let Some((question, which)) = self.question() else {
            return Ok(Written::default());
        };
        let question = question.clone();
        let written = self
            .progress
            .answer(campaign, &self.questions, &question, which, answer)?;
        self.answered();
        Ok(written)
    }

    /// Notes that the question being asked was answered, so it isn't skipped any more.
    pub fn answered(&mut self) {
        if let Some((question, _)) = self.question() {
            let key = question.key();
            self.progress.skip(&key, false);
        }
    }

    /// Whether the answer is a list of names, so a roll is added to it.
    pub fn asks_for_names(&self) -> bool {
        self.names.is_some()
    }

    /// The widget the keyboard goes to at a question: the answer's input, or the first
    /// empty name box.
    pub fn focus_id(&self) -> Option<String> {
        if let Some(names) = &self.names {
            let empty = names.iter().position(|name| name.trim().is_empty());
            return Some(name_id(empty.unwrap_or(names.len().saturating_sub(1))));
        }
        self.form.as_ref()?.first_id(self.field?)
    }

    /// Adds a rolled name to the list: in the first empty box, or a new one.
    pub fn add_name(&mut self, name: &str) -> Option<NamesDone> {
        let names = self.names.as_mut()?;
        let index = match names.iter().position(|name| name.trim().is_empty()) {
            Some(index) => index,
            None => {
                names.push(String::new());
                names.len() - 1
            }
        };
        names[index] = name.to_string();
        Some(NamesDone::Focus(index))
    }

    /// Changes the list of names as `message` says.
    pub fn update_names(&mut self, message: Message) -> Option<NamesDone> {
        let names = self.names.as_mut()?;
        match message {
            Message::Name(index, name) => {
                *names.get_mut(index)? = name;
                None
            }
            Message::PasteNames(index, pasted) => {
                let before = std::mem::replace(names.get_mut(index)?, pasted);
                Some(NamesDone::ReadPaste(index, before))
            }
            Message::PastedNames(index, before, pasted) => {
                let mut list = split_names(&pasted);
                if list.len() < 2 || index >= names.len() {
                    return None;
                }
                // The box keeps what it had, or takes the first name.
                let mut at = index + 1;
                if before.trim().is_empty() {
                    names[index] = list.remove(0);
                } else {
                    names[index] = before;
                }
                for name in list {
                    names.insert(at, name);
                    at += 1;
                }
                Some(NamesDone::Focus(at - 1))
            }
            Message::NameEntered(index) => {
                if index + 1 < names.len() {
                    Some(NamesDone::Focus(index + 1))
                } else if names[index].trim().is_empty() {
                    Some(NamesDone::Next)
                } else {
                    names.push(String::new());
                    Some(NamesDone::Focus(index + 1))
                }
            }
            Message::AddName => match names.iter().position(|name| name.trim().is_empty()) {
                Some(index) => Some(NamesDone::Focus(index)),
                None => {
                    names.push(String::new());
                    Some(NamesDone::Focus(names.len() - 1))
                }
            },
            Message::RemoveName(index) => {
                if index < names.len() {
                    names.remove(index);
                }
                if names.is_empty() {
                    names.push(String::new());
                }
                Some(NamesDone::Focus(index.min(names.len() - 1)))
            }
            _ => None,
        }
    }

    /// Goes to the next step. Notes an answer just made have their questions next.
    pub fn next(&mut self, campaign: &Campaign, open_text: &dyn Fn(&Path) -> Option<String>) {
        let at = self.find_again(campaign);
        self.go(at + 1, campaign, open_text);
    }

    pub fn back(&mut self, campaign: &Campaign, open_text: &dyn Fn(&Path) -> Option<String>) {
        let at = self.find_again(campaign);
        self.go(at.saturating_sub(1), campaign, open_text);
    }

    /// Works out the steps again, after an answer that may have made notes, and returns
    /// where the step shown is among them.
    fn find_again(&mut self, campaign: &Campaign) -> usize {
        let step = self.step().clone();
        self.steps = self.find_steps(campaign);
        self.steps
            .iter()
            .position(|found| *found == step)
            .unwrap_or(self.at.min(self.steps.len() - 1))
    }

    /// Skips the question, and goes to the next step.
    pub fn skip(&mut self, campaign: &Campaign, open_text: &dyn Fn(&Path) -> Option<String>) {
        if let Some((question, _)) = self.question() {
            let key = question.key();
            self.progress.skip(&key, true);
        }
        self.next(campaign, open_text);
    }

    /// Skips the rest of the chapter, and goes to the first question of the next.
    pub fn skip_chapter(
        &mut self,
        campaign: &Campaign,
        open_text: &dyn Fn(&Path) -> Option<String>,
    ) {
        let Some(chapter) = self.chapter() else {
            return;
        };
        let id = self.questions.chapters[chapter].id.clone();
        self.progress.skip(&id, true);
        let next = self
            .steps
            .iter()
            .position(|step| chapter_of(step).is_none_or(|found| found > chapter))
            .unwrap_or(self.steps.len() - 1);
        self.at = next;
        self.refresh(campaign, open_text);
    }

    /// Opens chapter `chapter` at its first question. A chapter that was skipped isn't
    /// any more.
    pub fn open_chapter(
        &mut self,
        chapter: usize,
        campaign: &Campaign,
        open_text: &dyn Fn(&Path) -> Option<String>,
    ) {
        let Some(at) = self
            .steps
            .iter()
            .position(|step| chapter_of(step) == Some(chapter))
        else {
            return;
        };
        if let Some(chapter) = self.questions.chapters.get(chapter) {
            let id = chapter.id.clone();
            self.progress.skip(&id, false);
        }
        self.at = at;
        self.refresh(campaign, open_text);
    }

    fn go(&mut self, at: usize, campaign: &Campaign, open_text: &dyn Fn(&Path) -> Option<String>) {
        self.at = at.min(self.steps.len() - 1);
        self.error = None;
        self.refresh(campaign, open_text);
    }

    /// The question that asks for the name of the note `note`, for saying to answer it
    /// first.
    fn name_question(&self, note: &str) -> Option<&Question> {
        self.questions
            .all()
            .map(|(_, question)| question)
            .find(|question| question.note == note && question.field.is_none())
    }

    /// Draws the tab. `answer` is the answer box, drawn by the form, and `chips` the
    /// notes to link to while one is being typed in it.
    pub fn view<'a, M: Clone + 'a>(
        &'a self,
        c: Colors,
        answer: Option<Element<'a, M>>,
        chips: Option<Element<'a, M>>,
        on: fn(Message) -> M,
    ) -> Element<'a, M> {
        let current = self.chapter();
        let chapters = column(self.questions.chapters.iter().enumerate().map(
            |(index, chapter)| -> Element<'a, M> {
                let state = &self.chapters[index];
                let progress = if state.skipped && state.answered == 0 {
                    "Skipped".to_string()
                } else if state.skipped {
                    format!("{} of {}, the rest skipped", state.answered, state.total)
                } else if state.answered == state.total {
                    "Done".to_string()
                } else {
                    format!("{} of {}", state.answered, state.total)
                };
                button(
                    column![
                        text(&chapter.title).size(14),
                        text(progress).size(12).color(c.text_muted),
                    ]
                    .spacing(2),
                )
                .width(Fill)
                .padding([8, 12])
                .style(style::flat(c, current == Some(index)))
                .on_press(on(Message::Chapter(index)))
                .into()
            },
        ))
        .spacing(2);
        let sidebar = container(scrollable(
            column![text("Walkthrough").size(12).color(c.text_muted), chapters]
                .spacing(10)
                .padding(12),
        ))
        .width(240)
        .height(Fill)
        .style(style::panel(c.sidebar));

        let back = button(text("Back").size(14))
            .padding([6, 14])
            .style(style::secondary(c))
            .on_press_maybe((self.at > 0).then(|| on(Message::Back)));

        let body: Element<'a, M> = match self.step() {
            Step::Question {
                chapter,
                question,
                which,
            } => {
                let chapter_index = *chapter;
                let chapter = &self.questions.chapters[*chapter];
                let question = &chapter.questions[*question];
                let mut body = column![text(&chapter.title).size(13).color(c.text_muted)]
                    .spacing(12)
                    .max_width(760);
                let first = self
                    .steps
                    .iter()
                    .position(|step| chapter_of(step) == Some(chapter_index))
                    == Some(self.at);
                if first && let Some(intro) = &chapter.intro {
                    body = body.push(text(intro).size(14).color(c.text_muted));
                }
                body = body.push(
                    text(question.ask_about(self.subject_name.as_deref()))
                        .size(22)
                        .color(c.text),
                );
                if let Some(hint) = &question.hint {
                    body = body.push(text(hint).size(13).color(c.text_muted));
                }
                let many = self
                    .questions
                    .notes
                    .get(&question.note)
                    .is_some_and(|spec| spec.many);
                if many && question.field.is_some() {
                    let count = self
                        .steps
                        .iter()
                        .filter(|step| {
                            matches!(step, Step::Question { chapter: c, question: q, .. }
                                if *c == chapter_index && chapter.questions[*q].key() == question.key())
                        })
                        .count();
                    body = body.push(
                        text(format!("{} of {count}", which + 1))
                            .size(12)
                            .color(c.text_muted),
                    );
                }

                match (answer, &self.names) {
                    (_, Some(names)) => body = body.push(self.names_view(c, names, on)),
                    (Some(answer), None) => body = body.push(answer),
                    (None, None) if self.error.is_none() => {
                        let first = self
                            .name_question(&question.note)
                            .map(|asked| format!("Answer \"{}\" first", asked.ask_about(None)))
                            .unwrap_or_else(|| "There is no note to write this into yet".into());
                        body = body.push(text(first).size(14).color(c.text_muted));
                    }
                    (None, None) => {}
                }
                if let Some(chips) = chips {
                    body = body.push(chips);
                }
                if !self.tables.is_empty() {
                    body = body.push(
                        row(self.tables.iter().map(|table| -> Element<'a, M> {
                            button(text(format!("Inspire me: {table}")).size(13))
                                .padding([4, 10])
                                .style(style::secondary(c))
                                .on_press(on(Message::Inspire(table.clone())))
                                .into()
                        }))
                        .spacing(6),
                    );
                }
                if let Some(error) = &self.error {
                    body = body.push(text(error).size(13).color(c.danger));
                }
                if let Some(name) = &self.subject_name {
                    let label = self
                        .form
                        .as_ref()
                        .zip(self.field)
                        .and_then(|(form, index)| form.label(index))
                        .unwrap_or_default();
                    // A note not made yet is named; one that is can be opened.
                    let goes_into: Element<'a, M> = match &self.subject {
                        Some(path) => row![
                            text("Goes into").size(12).color(c.text_muted),
                            button(text(name).size(12))
                                .padding([2, 6])
                                .style(style::flat(c, false))
                                .on_press(on(Message::Open(path.clone()))),
                        ]
                        .spacing(4)
                        .align_y(Center)
                        .into(),
                        None => text(format!("Makes the note {name}"))
                            .size(12)
                            .color(c.text_muted)
                            .into(),
                    };
                    body = body.push(
                        row![
                            goes_into,
                            text(format!("· {label}")).size(12).color(c.text_muted),
                        ]
                        .spacing(4)
                        .align_y(Center),
                    );
                }
                body = body.push(space().height(8)).push(
                    row![
                        back,
                        button(text("Skip").size(14))
                            .padding([6, 14])
                            .style(style::secondary(c))
                            .on_press(on(Message::Skip)),
                        button(text("Skip Chapter").size(14))
                            .padding([6, 14])
                            .style(style::flat(c, false))
                            .on_press(on(Message::SkipChapter)),
                        space::horizontal(),
                        button(text("Next").size(14))
                            .padding([6, 18])
                            .style(style::primary(c))
                            .on_press(on(Message::Next)),
                    ]
                    .spacing(8)
                    .align_y(Center),
                );
                body.into()
            }
            Step::Summary(chapter) => {
                let title = &self.questions.chapters[*chapter].title;
                let last = *chapter + 1 == self.questions.chapters.len();
                column![
                    text(format!("{title}: done")).size(22).color(c.text),
                    text(if self.made.is_empty() {
                        "No notes made in this chapter yet."
                    } else {
                        "Notes made in this chapter:"
                    })
                    .size(14)
                    .color(c.text_muted),
                    self.note_list(c, on),
                    space().height(8),
                    row![
                        back,
                        space::horizontal(),
                        button(text(if last { "Finish" } else { "Next Chapter" }).size(14))
                            .padding([6, 18])
                            .style(style::primary(c))
                            .on_press(on(Message::Next)),
                    ]
                    .spacing(8),
                ]
                .spacing(12)
                .max_width(760)
                .into()
            }
            Step::Done => column![
                text("That's the whole walkthrough").size(22).color(c.text),
                text(
                    "Every answer is in its note. Open a chapter on the left to change answers or fill in ones you skipped."
                )
                .size(14)
                .color(c.text_muted),
                self.note_list(c, on),
                space().height(8),
                row![back],
            ]
            .spacing(12)
            .max_width(760)
            .into(),
        };

        row![
            sidebar,
            rule::vertical(c.border_width.max(1.0)).style(style::separator(c)),
            container(scrollable(container(body).padding([32, 40]).width(Fill)))
                .width(Fill)
                .height(Fill)
                .style(style::panel(c.background)),
        ]
        .into()
    }

    /// A box for each name of a list, with a button to take it out, and Add Another.
    fn names_view<'a, M: Clone + 'a>(
        &'a self,
        c: Colors,
        names: &'a [String],
        on: fn(Message) -> M,
    ) -> Element<'a, M> {
        let boxes = names
            .iter()
            .enumerate()
            .map(|(index, name)| -> Element<'a, M> {
                let input = text_input("Type a name, or paste a list", name)
                    .id(name_id(index))
                    .on_input(move |name| on(Message::Name(index, name)))
                    .on_paste(move |name| on(Message::PasteNames(index, name)))
                    .on_submit(on(Message::NameEntered(index)))
                    .padding([6, 8])
                    .size(14)
                    .style(style::input(c));
                let remove = button(text("✕").size(13))
                    .padding([4, 10])
                    .style(style::flat(c, false))
                    .on_press_maybe((names.len() > 1).then(|| on(Message::RemoveName(index))));
                row![input, remove].spacing(6).align_y(Center).into()
            });
        column![
            text(&self.names_label).size(14).color(c.text),
            column(boxes).spacing(6),
            button(text("Add Another").size(13))
                .padding([4, 10])
                .style(style::secondary(c))
                .on_press(on(Message::AddName)),
        ]
        .spacing(8)
        .into()
    }

    /// The notes of a summary, each a button that opens it.
    fn note_list<'a, M: Clone + 'a>(&'a self, c: Colors, on: fn(Message) -> M) -> Element<'a, M> {
        column(self.made.iter().map(|(name, path)| -> Element<'a, M> {
            button(text(name).size(14))
                .padding([3, 8])
                .style(style::flat(c, false))
                .on_press(on(Message::Open(path.clone())))
                .into()
        }))
        .spacing(2)
        .into()
    }
}

/// The chapter a step is in, if it is in one.
fn chapter_of(step: &Step) -> Option<usize> {
    match step {
        Step::Question { chapter, .. } | Step::Summary(chapter) => Some(*chapter),
        Step::Done => None,
    }
}

/// A short name for the note at `path`, for messages.
fn campaign_name(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A form with one line for the name a question asks for.
fn names_form(ty: &ContentType, names: &[String]) -> Form {
    let mut field = Field::for_note(NAMES_KEY, Kind::Text);
    field.label = Some(format!("{} name", ty.label()));
    field.width = Width::Full;
    let names_type = ContentType {
        id: "walkthrough".into(),
        label: Some(ty.label()),
        icon: None,
        fields: vec![field],
        removed_fields: Vec::new(),
        parents: Vec::new(),
    };
    let mut form = Form::blank(names_type);
    form.lock();
    if let Some(name) = names.first() {
        form.set_value(NAMES_KEY, Some(&Value::String(name.clone())));
    }
    form
}

/// The names in a pasted list: one on each line, or else between commas, semicolons or
/// tabs, without list marks or numbers. Names copied all in lower case, as some name
/// generators give them, get capitals.
fn split_names(pasted: &str) -> Vec<String> {
    let lines: Vec<&str> = pasted
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let parts: Vec<&str> = if lines.len() > 1 {
        lines
    } else {
        pasted.split([',', ';', '\t']).collect()
    };
    parts
        .into_iter()
        .map(|part| {
            let part = part.trim();
            let part = part.trim_start_matches(['-', '*', '•', '+']).trim_start();
            // "1. Name" or "2) Name"
            let digits = part.len() - part.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            let part = match part[digits..].strip_prefix(['.', ')']) {
                Some(rest) if digits > 0 => rest.trim_start(),
                _ => part,
            };
            if part.chars().any(char::is_uppercase) {
                part.to_string()
            } else {
                capitalized(part)
            }
        })
        .filter(|name| !name.is_empty())
        .collect()
}

/// `name` with each word's first letter a capital.
fn capitalized(name: &str) -> String {
    name.split(' ')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pasted_list_is_split_into_names() {
        let generator = "swanburn\ncavehill\nheartham\n\nbleakstar\n";
        assert_eq!(
            split_names(generator),
            ["Swanburn", "Cavehill", "Heartham", "Bleakstar"]
        );
        assert_eq!(
            split_names("Eldmoor, the Iron Coast; Vael"),
            ["Eldmoor", "the Iron Coast", "Vael"]
        );
        assert_eq!(
            split_names("1. Old Tomas\n2) Brann\n- McAllister"),
            ["Old Tomas", "Brann", "McAllister"]
        );
        assert_eq!(split_names("Eldmoor"), ["Eldmoor"]);
    }
}
