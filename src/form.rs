//! The form a note in a typed folder is edited in: one input per field.
//!
//! The fields are the content type's, then any the note has on its own: front
//! matter keys and `## ` sections the type doesn't list (added with the main
//! pane's + button, or in another app like Obsidian).

use iced::advanced::widget::Id;
use iced::advanced::widget::operation::scrollable::AbsoluteOffset;
use iced::advanced::widget::operation::{Focusable, Operation, Outcome, Scrollable};
use iced::keyboard::{Key, Modifiers};
use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, space, svg, text, text_editor,
    text_input,
};
use iced::{Center, Element, Fill, Point, Rectangle, Vector};
use serde_yaml_ng::{Mapping, Value};

use crate::context_menu;
use crate::dice::{self, Dice, Expression, Special};
use crate::links;
use crate::note::{self, Note, Section};
use crate::style;
use crate::theme::Colors;
use crate::types::{ContentType, Field, Kind, Width};

/// The front matter key listing the type's fields this note doesn't use.
pub const REMOVED_KEY: &str = "removed_fields";

/// The widget id of the input for field `index`, so it can be focused again.
pub fn field_id(index: usize) -> String {
    format!("form-field-{index}")
}

/// The widget id of step `step` of steps field `index`. The one after the last step is
/// the box a new step is typed in.
pub fn step_id(index: usize, step: usize) -> String {
    format!("form-field-{index}-step-{step}")
}

/// The widget id of part `part` of dice field `index`: 0 is the number of dice (the
/// field's own id), 1 the modifier and 2 how many dice to keep.
pub fn dice_id(index: usize, part: usize) -> String {
    match part {
        0 => field_id(index),
        1 => format!("form-field-{index}-modifier"),
        _ => format!("form-field-{index}-keep"),
    }
}

/// The widget id of the form's scrollable, so Tab can scroll to the field it moves to.
pub const SCROLL: &str = "form-scroll";

/// Whether front matter `key` is one the form manages itself, rather than a field.
/// The front matter key for a note's other names, which links can use too, as in Obsidian.
pub const ALIASES_KEY: &str = "aliases";

pub fn is_reserved(key: &str) -> bool {
    key == "type" || key == REMOVED_KEY
}

#[derive(Debug, Clone)]
pub enum Message {
    Text(usize, String),
    Long(usize, text_editor::Action),
    Check(usize, bool),
    Choice(usize, String),
    Other(text_editor::Action),
    /// Removes a field from this note.
    Remove(usize),
    /// Opens the note link field `index` points to, making it if there is none.
    FollowLink(usize),
    /// Rolls the dice in field `index`, like `1d8+3`.
    Roll(usize, String),
    /// The text of step `.1` of steps field `.0`; the step after the last is a new one
    /// being typed.
    StepText(usize, usize, String),
    /// Ticks or unticks a step.
    StepCheck(usize, usize, bool),
    /// Enter in a step: adds the new step being typed, or a blank one after this one.
    StepSubmit(usize, usize),
    /// Moves a step up (true) or down one place.
    StepMove(usize, usize, bool),
    StepRemove(usize, usize),
    /// Opens the note a step links to, making it if there is none.
    FollowStep(usize, usize),
    /// Changes one part of dice field `.0`.
    Dice(usize, DicePart),
}

/// A part of a dice field, changed in the form.
#[derive(Debug, Clone)]
pub enum DicePart {
    /// What is typed in the box for how many dice to roll.
    Count(String),
    Die(Sides),
    /// What is typed in the modifier box, like `+2`.
    Modifier(String),
    Special(Special),
    /// What is typed in the box for how many dice to keep.
    Keep(String),
    /// The text of a dice field that can't be split into parts.
    Text(String),
}

/// A die in a dice field's list of dice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sides(pub u32);

impl Sides {
    /// The dice offered, from the systems people play most.
    const COMMON: [u32; 9] = [2, 3, 4, 6, 8, 10, 12, 20, 100];
}

impl std::fmt::Display for Sides {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "d{}", self.0)
    }
}

/// What a dice field holds, as the parts its inputs show.
#[derive(Debug, Clone, Default, PartialEq)]
struct DiceInput {
    count: String,
    die: Option<u32>,
    modifier: String,
    special: Special,
    keep: String,
    /// The field's text, when it can't be split into parts (like `1d8+1d6`), so it is
    /// edited as text and kept as written.
    raw: Option<String>,
}

impl DiceInput {
    fn parse(text: &str) -> Self {
        let text = text.trim();
        if text.is_empty() {
            return Self::default();
        }
        let Some(dice) = Dice::parse(text) else {
            return DiceInput {
                raw: Some(text.to_string()),
                ..Self::default()
            };
        };
        DiceInput {
            count: dice.count.to_string(),
            die: Some(dice.sides),
            modifier: match dice.modifier {
                0 => String::new(),
                modifier if modifier > 0 => format!("+{modifier}"),
                modifier => modifier.to_string(),
            },
            special: dice.special,
            keep: if dice.special.keeps() {
                dice.keep.to_string()
            } else {
                String::new()
            },
            raw: None,
        }
    }

    /// The dice the parts make, `None` when nothing is filled in, or why they can't be rolled.
    fn dice(&self) -> Result<Option<Dice>, String> {
        let number = |text: &str| text.trim_start_matches('+').parse::<i64>().ok();
        let modifier = match self.modifier.as_str() {
            "" | "+" | "-" => 0,
            text => number(text).ok_or("The modifier must be a number, like +2")?,
        };
        let Some(sides) = self.die else {
            if self.count.is_empty() && modifier == 0 && self.special == Special::None {
                return Ok(None);
            }
            return Err("Pick a die".into());
        };
        let count = self.count.parse().unwrap_or(1);
        let dice = Dice {
            count,
            sides,
            modifier,
            special: self.special,
            keep: if self.special.keeps() {
                self.keep.parse().unwrap_or(1)
            } else {
                0
            },
        };
        dice.check()?;
        Ok(Some(dice))
    }

    /// The field's text, like `2d6+3`, or why it can't be saved.
    fn text(&self) -> Result<String, String> {
        if let Some(raw) = &self.raw {
            return Ok(raw.trim().to_string());
        }
        Ok(self
            .dice()?
            .map(|dice| dice.to_string())
            .unwrap_or_default())
    }

    fn update(&mut self, part: DicePart) {
        // Only digits fit in the boxes for numbers of dice; anything else typed is dropped.
        let digits = |text: String| -> String {
            text.chars().filter(char::is_ascii_digit).take(3).collect()
        };
        match part {
            DicePart::Count(count) => self.count = digits(count),
            DicePart::Die(sides) => self.die = Some(sides.0),
            DicePart::Modifier(modifier) => {
                let sign = modifier
                    .trim_start()
                    .chars()
                    .next()
                    .filter(|c| matches!(c, '+' | '-'));
                let digits: String = modifier
                    .chars()
                    .filter(char::is_ascii_digit)
                    .take(6)
                    .collect();
                self.modifier = sign.map(String::from).unwrap_or_default() + &digits;
            }
            DicePart::Special(special) => {
                // Keeping starts at one die fewer than are rolled, like 4d6 keep 3.
                if special.keeps() && self.keep.is_empty() {
                    let count = self.count.parse::<u32>().unwrap_or(1);
                    self.keep = count.saturating_sub(1).max(1).to_string();
                }
                self.special = special;
            }
            DicePart::Keep(keep) => self.keep = digits(keep),
            DicePart::Text(text) => {
                // Emptied, the field can be filled in part by part.
                *self = if text.trim().is_empty() {
                    Self::default()
                } else {
                    DiceInput {
                        raw: Some(text),
                        ..Self::default()
                    }
                };
            }
        }
    }

    /// The dice offered: the common ones, and the field's own if it is another.
    fn dice_list(&self) -> Vec<Sides> {
        let mut list: Vec<u32> = Sides::COMMON.to_vec();
        if let Some(die) = self.die.filter(|die| !list.contains(die)) {
            list.push(die);
            list.sort();
        }
        list.into_iter().map(Sides).collect()
    }
}

impl Message {
    /// Whether the focus moves to another input after this message, so it must be focused
    /// again with [`Form::focused_id`].
    pub fn moves_focus(&self) -> bool {
        matches!(
            self,
            Message::StepSubmit(..) | Message::StepMove(..) | Message::StepRemove(..)
        )
    }
}

/// One step of a steps field: a line of text, which can be a `[[link]]` to another note.
#[derive(Debug, Clone, Default, PartialEq)]
struct Step {
    done: bool,
    text: String,
}

/// What a steps field holds.
#[derive(Debug, Default)]
struct Steps {
    steps: Vec<Step>,
    /// The new step being typed in the box under the last one.
    draft: String,
}

impl Steps {
    /// The steps of a `## ` section: one per line, without list marks and task boxes.
    fn parse(body: &str) -> Self {
        let steps = body
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let item = list_item(line);
                Step {
                    done: item.ticked == Some(true),
                    text: item.text.to_string(),
                }
            })
            .collect();
        Steps {
            steps,
            draft: String::new(),
        }
    }

    /// The steps as a Markdown task list, with the step being typed as the last one.
    fn to_markdown(&self) -> String {
        let draft = Step {
            done: false,
            text: self.draft.clone(),
        };
        self.steps
            .iter()
            .chain([&draft])
            .filter(|step| !step.text.trim().is_empty())
            .map(|step| {
                let tick = if step.done { 'x' } else { ' ' };
                format!("- [{tick}] {}", step.text.trim())
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The text of step `step`, or of the new step being typed.
    fn text(&self, step: usize) -> &str {
        self.steps
            .get(step)
            .map_or(self.draft.as_str(), |step| &step.text)
    }

    fn text_mut(&mut self, step: usize) -> &mut String {
        match self.steps.get_mut(step) {
            Some(step) => &mut step.text,
            None => &mut self.draft,
        }
    }
}

/// A line of a list, as [`list_item`] reads it.
struct ListItem<'a> {
    /// The text after the list mark and task box.
    text: &'a str,
    /// Whether the line starts with a list mark (`- `, `* `, `+ ` or `1. `), unindented.
    listed: bool,
    /// Whether its task box is ticked, if it has one.
    ticked: Option<bool>,
}

fn list_item(line: &str) -> ListItem<'_> {
    let trimmed = line.trim();
    let marked = ["- ", "* ", "+ "]
        .iter()
        .find_map(|mark| trimmed.strip_prefix(mark))
        .or_else(|| {
            let digits = trimmed.find(|c: char| !c.is_ascii_digit())?;
            let rest = &trimmed[digits..];
            (digits > 0)
                .then(|| rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")))
                .flatten()
        });
    let Some(rest) = marked else {
        return ListItem {
            text: trimmed,
            listed: false,
            ticked: None,
        };
    };
    let listed = !line.starts_with(char::is_whitespace);
    let rest = rest.trim_start();
    for (tick, ticked) in [("[ ]", false), ("[x]", true), ("[X]", true)] {
        if let Some(text) = rest.strip_prefix(tick) {
            return ListItem {
                text: text.trim(),
                listed,
                ticked: Some(ticked),
            };
        }
    }
    ListItem {
        text: rest.trim(),
        listed,
        ticked: None,
    }
}

/// Whether a steps field's section holds only a list, so the form shows it as it is.
pub fn is_step_list(body: &str) -> bool {
    body.lines()
        .filter(|line| !line.trim().is_empty())
        .all(|line| list_item(line).listed)
}

/// Whether a section the type doesn't list is a task list, so the form shows it as steps.
fn is_task_list(body: &str) -> bool {
    let mut lines = body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .peekable();
    lines.peek().is_some()
        && lines.all(|line| {
            let item = list_item(line);
            item.listed && item.ticked.is_some()
        })
}

/// Where the link being typed at the end of `text` starts: its last `[[` with no `]]` after.
fn open_link(text: &str) -> Option<usize> {
    let start = text.rfind("[[")?;
    (!text[start..].contains("]]")).then_some(start)
}

/// What a short field holds, from [`Form::short_field`].
pub enum FieldValue<'a> {
    Text(&'a str),
    Check(bool),
    Choice(Option<&'a str>),
    /// A dice field, edited with [`Form::dice_view`].
    Dice,
}

/// What the user typed into one field.
enum Input {
    Text(String),
    Long(text_editor::Content),
    Check(bool),
    Choice(Option<String>),
    Steps(Steps),
    Dice(DiceInput),
}

impl Input {
    fn empty(kind: Kind) -> Self {
        match kind {
            Kind::Longtext => Input::Long(text_editor::Content::new()),
            Kind::Steps => Input::Steps(Steps::default()),
            Kind::Checkbox => Input::Check(false),
            Kind::Choice => Input::Choice(None),
            Kind::Dice => Input::Dice(DiceInput::default()),
            Kind::Text | Kind::Number | Kind::Tags | Kind::Link => Input::Text(String::new()),
        }
    }
}

pub struct Form {
    pub ty: ContentType,
    /// The type's fields, then the ones only this note has.
    fields: Vec<Field>,
    /// The index in `fields` where this note's own fields start.
    own: usize,
    /// The keys of the type's fields this note doesn't use, in the type's order.
    removed: Vec<String>,
    /// One input per field, in the same order.
    inputs: Vec<Input>,
    /// The note's front matter as it was read, so keys stay in place when saved.
    front: Mapping,
    /// The body text before the first `## ` section, kept as it is.
    other: text_editor::Content,
    /// Whether the "Other" area shows: only when the note had something to put there.
    show_other: bool,
    /// The field last typed or clicked in, which link suggestions are for.
    focus: Option<usize>,
    /// In a steps field, the step last typed in.
    step: usize,
}

impl Form {
    /// An empty form for a new note.
    pub fn blank(ty: ContentType) -> Self {
        Self::from_note(ty, Note::default())
    }

    pub fn from_note(ty: ContentType, note: Note) -> Self {
        let mut sections: Vec<Option<Section>> = note.sections.into_iter().map(Some).collect();

        let removed_keys: Vec<String> = match note.front.get(REMOVED_KEY) {
            Some(Value::Sequence(keys)) => keys.iter().map(note::value_to_text).collect(),
            Some(Value::String(key)) => vec![key.clone()],
            _ => Vec::new(),
        };
        // The name can't be removed: it is the note's filename.
        let removed: Vec<String> = ty
            .fields
            .iter()
            .filter(|field| field.key != "name" && removed_keys.contains(&field.key))
            .map(|field| field.key.clone())
            .collect();

        let mut fields: Vec<Field> = ty
            .fields
            .iter()
            .filter(|field| !removed.contains(&field.key))
            .cloned()
            .collect();
        let own = fields.len();

        // Front matter keys the type doesn't list become fields of this note, if they can be edited as one.
        for (key, value) in &note.front {
            let key = note::value_to_text(key);
            let listed = is_reserved(&key)
                || ty.fields.iter().any(|field| field.key == key)
                || fields.iter().any(|field| field.key == key);
            if let (false, Some(kind)) = (listed, kind_of(value)) {
                // Shown like the type's fields: `category` as "Category".
                fields.push(Field {
                    label: None,
                    ..Field::for_note(&key, kind)
                });
            }
        }

        let mut inputs: Vec<Input> = fields
            .iter()
            .map(|field| {
                let value = note.front.get(field.key.as_str());
                let text = value.map(note::value_to_text).unwrap_or_default();

                // Long text and steps fields are the `## ` section with their name.
                let mut section = || {
                    let label = field.label();
                    let section = sections.iter_mut().find(|section| {
                        section.as_ref().is_some_and(|section| {
                            section.heading.eq_ignore_ascii_case(&label)
                                || section.heading.eq_ignore_ascii_case(&field.key)
                        })
                    });
                    section
                        .and_then(Option::take)
                        .map(|section| section.body)
                        .unwrap_or_default()
                };
                match field.kind {
                    Kind::Longtext => Input::Long(text_editor::Content::with_text(&section())),
                    Kind::Steps => Input::Steps(Steps::parse(&section())),
                    Kind::Checkbox => Input::Check(match value {
                        Some(Value::Bool(checked)) => *checked,
                        _ => matches!(text.to_lowercase().as_str(), "true" | "yes"),
                    }),
                    Kind::Choice => Input::Choice((!text.is_empty()).then_some(text)),
                    Kind::Link => Input::Text(note::unlink(&text).to_string()),
                    Kind::Dice => Input::Dice(DiceInput::parse(&text)),
                    Kind::Text | Kind::Number | Kind::Tags => Input::Text(text),
                }
            })
            .collect();

        // Sections the type doesn't list become long text fields of this note, or steps
        // if they are a task list.
        for section in sections.into_iter().flatten() {
            if is_task_list(&section.body) {
                fields.push(Field::for_note(&section.heading, Kind::Steps));
                inputs.push(Input::Steps(Steps::parse(&section.body)));
            } else {
                fields.push(Field::for_note(&section.heading, Kind::Longtext));
                inputs.push(Input::Long(text_editor::Content::with_text(&section.body)));
            }
        }

        let form = Form {
            ty,
            fields,
            own,
            removed,
            inputs,
            other: text_editor::Content::with_text(&note.preamble),
            show_other: false,
            focus: None,
            step: 0,
            front: note.front,
        };

        Form {
            show_other: !note.preamble.trim().is_empty() || !form.uneditable_keys().is_empty(),
            ..form
        }
    }

    /// Applies an edit. Returns whether the note changed.
    pub fn update(&mut self, message: Message) -> bool {
        match message {
            Message::Text(index, value) => {
                if let Some(Input::Text(text)) = self.inputs.get_mut(index) {
                    *text = value;
                    self.focus = Some(index);
                }
            }
            Message::Long(index, action) => {
                let edited = action.is_edit();
                if let Some(Input::Long(content)) = self.inputs.get_mut(index) {
                    if !matches!(action, text_editor::Action::Scroll { .. }) {
                        self.focus = Some(index);
                    }
                    links::perform(content, action);
                }
                return edited;
            }
            Message::FollowLink(_) | Message::Roll(..) | Message::FollowStep(..) => return false,
            Message::StepText(index, step, value) => {
                let Some(Input::Steps(steps)) = self.inputs.get_mut(index) else {
                    return false;
                };
                *steps.text_mut(step) = value;
                self.focus = Some(index);
                self.step = step.min(steps.steps.len());
            }
            Message::StepCheck(index, step, done) => {
                let Some(Input::Steps(steps)) = self.inputs.get_mut(index) else {
                    return false;
                };
                let Some(step) = steps.steps.get_mut(step) else {
                    return false;
                };
                step.done = done;
            }
            Message::StepSubmit(index, step) => {
                let Some(Input::Steps(steps)) = self.inputs.get_mut(index) else {
                    return false;
                };
                self.focus = Some(index);
                if step < steps.steps.len() {
                    // Enter in a step makes a blank one after it, like in a list.
                    steps.steps.insert(step + 1, Step::default());
                    self.step = step + 1;
                } else {
                    // The new step goes on the list, and the box is ready for the next.
                    let text = std::mem::take(&mut steps.draft);
                    self.step = steps.steps.len();
                    if text.trim().is_empty() {
                        return false;
                    }
                    steps.steps.push(Step {
                        done: false,
                        text: text.trim().to_string(),
                    });
                    self.step += 1;
                }
            }
            Message::StepMove(index, step, up) => {
                let Some(Input::Steps(steps)) = self.inputs.get_mut(index) else {
                    return false;
                };
                let to = if up {
                    step.checked_sub(1)
                } else {
                    Some(step + 1)
                };
                let Some(to) = to.filter(|&to| step < steps.steps.len() && to < steps.steps.len())
                else {
                    return false;
                };
                steps.steps.swap(step, to);
                self.focus = Some(index);
                self.step = to;
            }
            Message::StepRemove(index, step) => {
                let Some(Input::Steps(steps)) = self.inputs.get_mut(index) else {
                    return false;
                };
                if step >= steps.steps.len() {
                    return false;
                }
                steps.steps.remove(step);
                // The focus goes to the step that took its place.
                self.focus = Some(index);
                self.step = step;
            }
            Message::Dice(index, part) => {
                let Some(Input::Dice(dice)) = self.inputs.get_mut(index) else {
                    return false;
                };
                self.focus = Some(index);
                self.step = match part {
                    DicePart::Modifier(_) => 1,
                    DicePart::Keep(_) => 2,
                    DicePart::Count(_) | DicePart::Text(_) => 0,
                    DicePart::Die(_) | DicePart::Special(_) => self.step,
                };
                let before = dice.clone();
                dice.update(part);
                return *dice != before;
            }
            Message::Check(index, value) => {
                if let Some(Input::Check(checked)) = self.inputs.get_mut(index) {
                    *checked = value;
                }
            }
            Message::Choice(index, value) => {
                if let Some(Input::Choice(choice)) = self.inputs.get_mut(index) {
                    *choice = Some(value);
                }
            }
            Message::Other(action) => {
                let edited = action.is_edit();
                self.other.perform(action);
                return edited;
            }
            Message::Remove(index) => {
                if !self.can_remove(index) {
                    return false;
                }
                let field = self.fields.remove(index);
                self.inputs.remove(index);
                self.focus = None;
                self.front.shift_remove(field.key.as_str());

                if index < self.own {
                    self.own -= 1;
                    self.removed.push(field.key);
                    // Keep the type's order, so the list in the file doesn't depend on removal order.
                    let order = |key: &String| self.ty.fields.iter().position(|f| f.key == *key);
                    self.removed.sort_by_key(order);
                }
            }
        }
        true
    }

    /// Whether field `index` can be removed from this note: any but the name.
    fn can_remove(&self, index: usize) -> bool {
        index < self.fields.len() && !(index < self.own && self.fields[index].key == "name")
    }

    /// The labels of the type's fields this note doesn't use.
    pub fn removed_labels(&self) -> Vec<String> {
        self.removed
            .iter()
            .filter_map(|key| self.ty.fields.iter().find(|field| field.key == *key))
            .map(Field::label)
            .collect()
    }

    /// Adds a field to this note only.
    ///
    /// If one of the type's fields that was removed from this note has that name, it comes back instead.
    pub fn add_field(&mut self, label: &str, kind: Kind) -> Result<(), String> {
        let label = label.trim();
        if label.is_empty() {
            return Err("Give the field a name".into());
        }

        if let Some(position) = self.removed.iter().position(|key| {
            self.ty.fields.iter().any(|field| {
                field.key == *key
                    && (field.key.eq_ignore_ascii_case(label)
                        || field.label().eq_ignore_ascii_case(label))
            })
        }) {
            let key = self.removed.remove(position);
            let field = self
                .ty
                .fields
                .iter()
                .find(|field| field.key == key)
                .cloned()
                .expect("removed keys are the type's");

            // Put it back where the type lists it, among the type's fields still here.
            let type_order = |key: &str| self.ty.fields.iter().position(|f| f.key == key);
            let target = type_order(&key);
            let index = self.fields[..self.own]
                .iter()
                .position(|f| type_order(&f.key) > target)
                .unwrap_or(self.own);

            self.inputs.insert(index, Input::empty(field.kind));
            self.fields.insert(index, field);
            self.own += 1;
            self.focus = None;
            return Ok(());
        }

        let taken = is_reserved(label)
            || self.fields.iter().any(|field| {
                field.key.eq_ignore_ascii_case(label) || field.label().eq_ignore_ascii_case(label)
            })
            || self
                .front
                .keys()
                .any(|key| note::value_to_text(key).eq_ignore_ascii_case(label));
        if taken {
            return Err(format!("This note already has a field called {label:?}"));
        }

        // Obsidian's other names for a note are a list under `aliases`.
        if label.eq_ignore_ascii_case(ALIASES_KEY) {
            self.fields.push(Field {
                label: None,
                ..Field::for_note(ALIASES_KEY, Kind::Tags)
            });
            self.inputs.push(Input::empty(Kind::Tags));
            return Ok(());
        }

        self.fields.push(Field::for_note(label, kind));
        self.inputs.push(Input::empty(kind));
        Ok(())
    }

    /// The label of field `index`, to name what was rolled from it.
    pub fn label(&self, index: usize) -> Option<String> {
        self.fields.get(index).map(Field::label)
    }

    /// The field last typed or clicked in.
    pub fn focus(&self) -> Option<usize> {
        self.focus
    }

    /// The link being typed in the field last typed in: the text after an unclosed `[[`
    /// in a long text field, or what a link field holds. The flag is true for a link field.
    pub fn link_query(&self) -> Option<(String, bool)> {
        let index = self.focus?;
        match (&self.inputs[index], self.fields[index].kind) {
            (Input::Long(content), _) => links::query(content).map(|query| (query, false)),
            (Input::Text(value), Kind::Link) if !value.trim().is_empty() => {
                Some((value.clone(), true))
            }
            (Input::Steps(steps), _) => {
                let text = steps.text(self.step);
                open_link(text).map(|start| (text[start + 2..].to_string(), false))
            }
            _ => None,
        }
    }

    /// The field last typed in, if it can link to notes: a link or steps field.
    pub fn focused_linking_field(&self) -> Option<&Field> {
        self.focus
            .map(|index| &self.fields[index])
            .filter(|field| field.kind.links())
    }

    /// Steps field `index` and the note step `step` of it links to.
    pub fn step_link(&self, index: usize, step: usize) -> Option<(&Field, String)> {
        let (Some(field), Some(Input::Steps(steps))) =
            (self.fields.get(index), self.inputs.get(index))
        else {
            return None;
        };
        let text = steps.text(step);
        let link = links::find(text).into_iter().next()?;
        Some((field, text[link.target].to_string()))
    }

    /// The widget id of the input last typed in, to focus it again.
    pub fn focused_id(&self) -> Option<String> {
        let index = self.focus?;
        Some(match self.inputs[index] {
            Input::Steps(_) => step_id(index, self.step),
            Input::Dice(_) => dice_id(index, self.step),
            _ => field_id(index),
        })
    }

    /// The inputs Tab moves between, in order: each field's and step's, with its field
    /// and step. Checkboxes and choices can't take the keyboard's focus, so they aren't.
    fn tab_stops(&self) -> Vec<(usize, usize, String)> {
        let mut stops = Vec::new();
        for (index, input) in self.inputs.iter().enumerate() {
            match input {
                Input::Text(_) | Input::Long(_) => stops.push((index, 0, field_id(index))),
                Input::Steps(steps) => stops.extend(
                    (0..=steps.steps.len()).map(|step| (index, step, step_id(index, step))),
                ),
                Input::Dice(dice) => {
                    let parts = match (&dice.raw, dice.special.keeps()) {
                        (Some(_), _) => 1,
                        (None, false) => 2,
                        (None, true) => 3,
                    };
                    stops.extend((0..parts).map(|part| (index, part, dice_id(index, part))));
                }
                Input::Check(_) | Input::Choice(_) => {}
            }
        }
        stops
    }

    /// The widget ids Tab moves between, in order, for [`tab`].
    pub fn tab_ids(&self) -> Vec<String> {
        self.tab_stops().into_iter().map(|(.., id)| id).collect()
    }

    /// Notes that Tab moved to input `position` of [`Self::tab_ids`].
    pub fn tabbed_to(&mut self, position: usize) {
        if let Some((index, step, _)) = self.tab_stops().into_iter().nth(position) {
            self.focus = Some(index);
            self.step = step;
        }
    }

    /// Link field `index` and the note name in it.
    pub fn link_field(&self, index: usize) -> Option<(&Field, &str)> {
        match (self.fields.get(index), self.inputs.get(index)) {
            (Some(field), Some(Input::Text(value))) if field.kind == Kind::Link => {
                // `BBEG|Jerry` (a link to an alias, made in Obsidian) is a link to BBEG.
                let target = note::unlink(value);
                Some((field, target.split(['|', '#']).next().unwrap_or(target)))
            }
            _ => None,
        }
    }

    /// The link field last typed in, if the focus is on one.
    pub fn focused_link_field(&self) -> Option<usize> {
        self.focus.filter(|&index| self.link_field(index).is_some())
    }

    /// Finishes the link being typed with `link`. Returns whether the note changed.
    pub fn complete_link(&mut self, link: &str) -> bool {
        let Some(index) = self.focus else {
            return false;
        };
        match &mut self.inputs[index] {
            Input::Long(content) => {
                links::complete(content, link);
                true
            }
            Input::Text(value) if self.fields[index].kind == Kind::Link => {
                // A link field holds just the note: `BBEG`, not `BBEG|Jerry`.
                *value = link.split('|').next().unwrap_or(link).to_string();
                // The link is done, so stop suggesting notes for it.
                self.focus = None;
                true
            }
            Input::Steps(steps) => {
                let text = steps.text_mut(self.step);
                let Some(start) = open_link(text) else {
                    return false;
                };
                text.truncate(start);
                text.push_str(&format!("[[{link}]]"));
                true
            }
            _ => false,
        }
    }

    /// The long text field last typed in, for cutting, copying and pasting.
    pub fn focused_text(&self) -> Option<&text_editor::Content> {
        match &self.inputs[self.focus?] {
            Input::Long(content) => Some(content),
            _ => None,
        }
    }

    /// Like [`Self::focused_text`], to change it, with the field's index.
    pub fn focused_text_mut(&mut self) -> Option<(usize, &mut text_editor::Content)> {
        let index = self.focus?;
        match &mut self.inputs[index] {
            Input::Long(content) => Some((index, content)),
            _ => None,
        }
    }

    /// The target of the link the cursor is in, in the long text field last typed in.
    pub fn link_under_cursor(&self) -> Option<String> {
        match &self.inputs[self.focus?] {
            Input::Long(content) => links::under_cursor(content),
            _ => None,
        }
    }

    /// Types `text` at the cursor in the long text field last typed in. Returns whether
    /// there was such a field.
    pub fn insert_text(&mut self, text: &str) -> bool {
        match self.focus.map(|index| &mut self.inputs[index]) {
            Some(Input::Long(content)) => {
                content.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                    std::sync::Arc::new(text.to_string()),
                )));
                true
            }
            _ => false,
        }
    }

    /// The note's name, from its `name` field, if the type has one and it is filled in.
    pub fn name(&self) -> Option<String> {
        let index = self.fields[..self.own]
            .iter()
            .position(|field| field.key == "name")?;
        match self.inputs.get(index) {
            Some(Input::Text(name)) if !name.trim().is_empty() => Some(name.trim().to_string()),
            _ => None,
        }
    }

    /// The field stored under front matter `key` and what it holds, for editing it outside
    /// the form, like in the reading view. Long text fields are body sections, so they
    /// have none.
    pub fn short_field(&self, key: &str) -> Option<(usize, &Field, FieldValue<'_>)> {
        let index = self.front_field(key)?;
        let value = match &self.inputs[index] {
            Input::Text(text) => FieldValue::Text(text),
            Input::Check(checked) => FieldValue::Check(*checked),
            Input::Choice(choice) => FieldValue::Choice(choice.as_deref()),
            Input::Dice(_) => FieldValue::Dice,
            Input::Long(_) | Input::Steps(_) => return None,
        };
        Some((index, &self.fields[index], value))
    }

    /// The index of the field stored under front matter `key`, if there is one.
    fn front_field(&self, key: &str) -> Option<usize> {
        self.fields
            .iter()
            .position(|field| !field.kind.is_section() && field.key == key)
    }

    /// Front matter keys no field covers, other than `type`, such as nested lists.
    fn uneditable_keys(&self) -> Vec<(String, String)> {
        self.front
            .iter()
            .filter_map(|(key, value)| {
                let key = note::value_to_text(key);
                let covered = is_reserved(&key) || self.front_field(&key).is_some();
                (!covered).then(|| (key, note::value_to_text(value)))
            })
            .collect()
    }

    /// The value to store in the front matter for field `index`, or `None` to leave it out.
    ///
    /// The note's own fields are kept even when empty, so they don't vanish on save.
    fn front_value(&self, index: usize) -> Result<Option<Value>, String> {
        let field = &self.fields[index];
        let keep_empty = index >= self.own;

        let value = match &self.inputs[index] {
            Input::Long(_) | Input::Steps(_) => None,
            Input::Check(checked) => {
                (*checked || keep_empty || self.front.contains_key(field.key.as_str()))
                    .then_some(Value::Bool(*checked))
            }
            Input::Choice(choice) => choice.clone().map(Value::String),
            Input::Dice(dice) => {
                let text = dice
                    .text()
                    .map_err(|error| format!("{}: {error}", field.label()))?;
                if text.is_empty() {
                    keep_empty.then_some(Value::Null)
                } else {
                    Some(Value::String(text))
                }
            }
            Input::Text(text) => {
                let text = text.trim();
                if text.is_empty() {
                    keep_empty.then_some(Value::Null)
                } else {
                    Some(match field.kind {
                        Kind::Number => parse_number(text)
                            .ok_or_else(|| format!("{} must be a number", field.label()))?,
                        Kind::Tags => Value::Sequence(
                            text.split(',')
                                .map(str::trim)
                                .filter(|tag| !tag.is_empty())
                                .map(Value::from)
                                .collect(),
                        ),
                        Kind::Link => Value::String(format!("[[{}]]", note::unlink(text))),
                        _ => Value::String(text.to_string()),
                    })
                }
            }
        };

        Ok(value)
    }

    /// Writes the note as Markdown: front matter for short fields, `## ` sections for long ones.
    pub fn to_markdown(&self) -> Result<String, String> {
        let mut front = Mapping::new();
        front.insert("type".into(), self.ty.id.clone().into());
        if !self.removed.is_empty() {
            front.insert(
                REMOVED_KEY.into(),
                Value::Sequence(self.removed.iter().map(|key| key.as_str().into()).collect()),
            );
        }

        // Keep the keys that were already there in their order, updating the ones the form edits.
        for (key, value) in &self.front {
            let name = note::value_to_text(key);
            if is_reserved(&name) {
                continue;
            }
            match self.front_field(&name) {
                Some(index) => {
                    if let Some(value) = self.front_value(index)? {
                        front.insert(key.clone(), value);
                    }
                }
                None => {
                    front.insert(key.clone(), value.clone());
                }
            }
        }

        for (index, field) in self.fields.iter().enumerate() {
            if !field.kind.is_section()
                && !self.front.contains_key(field.key.as_str())
                && let Some(value) = self.front_value(index)?
            {
                front.insert(field.key.clone().into(), value);
            }
        }

        let sections: Vec<Section> = self
            .fields
            .iter()
            .zip(&self.inputs)
            .enumerate()
            .filter_map(|(index, (field, input))| {
                let body = match input {
                    Input::Long(content) => content.text(),
                    Input::Steps(steps) => steps.to_markdown(),
                    _ => return None,
                };
                Some(Section {
                    heading: field.label(),
                    body,
                    keep_empty: index >= self.own,
                })
            })
            .collect();

        note::to_markdown(&front, &sections, &self.other.text())
    }

    /// Shows the form. Long text fields color `[[links]]` with `link_colors`, and Tab
    /// (or Enter in a link field) sends `accept` to finish the link being typed.
    #[allow(clippy::too_many_arguments)]
    pub fn view<'a, M: Clone + 'a>(
        &'a self,
        c: Colors,
        map: fn(Message) -> M,
        shortcut: fn(&Key, Modifiers) -> Option<M>,
        link_colors: links::Colors,
        accept: Option<M>,
        on_menu: fn(Point) -> M,
        roll_icon: &'a svg::Handle,
    ) -> Element<'a, M> {
        // Every editor sees every key press, so only the focused one handles shortcuts.
        let accept_tab = accept.clone();
        let key_binding = move |key_press: text_editor::KeyPress| {
            if let Some(binding) = links::tab_binding(&key_press, &accept_tab) {
                return Some(binding);
            }
            if key_press.modifiers.command()
                && matches!(key_press.status, text_editor::Status::Focused { .. })
                && let Some(message) = shortcut(&key_press.key, key_press.modifiers)
            {
                return Some(text_editor::Binding::Custom(message));
            }
            text_editor::Binding::from_key_press(key_press)
        };

        // A field's label, with a button to remove the field from this note.
        let label = move |index: usize, label: String| -> Element<'a, M> {
            let label = text(label).size(14).color(c.text);
            if !self.can_remove(index) {
                return label.into();
            }
            row![
                label,
                space::horizontal(),
                button(text("Remove").size(12))
                    .padding([0, 6])
                    .style(style::flat(c, false))
                    .on_press(map(Message::Remove(index))),
            ]
            .align_y(Center)
            .into()
        };

        let fields =
            self.fields
                .iter()
                .zip(&self.inputs)
                .enumerate()
                .map(|(index, (field, input))| {
                    let widget: Element<'a, M> = match input {
                        Input::Text(value) => {
                            let placeholder = match field.kind {
                                Kind::Number => "0",
                                Kind::Tags => "comma, separated, tags",
                                Kind::Link => "Name of another note",
                                _ => "",
                            };
                            let mut input = text_input(placeholder, value)
                                .id(field_id(index))
                                .on_input(move |value| map(Message::Text(index, value)))
                                .size(14)
                                .padding(8)
                                .style(style::input(c));
                            if field.kind == Kind::Link
                                && self.focus == Some(index)
                                && let Some(accept) = &accept
                            {
                                input = input.on_submit(accept.clone());
                            }

                            // The input is always first in a row, so the Open button
                            // showing up as the first letter is typed doesn't make iced
                            // build a new input, which would lose the focus.
                            let open =
                                (field.kind == Kind::Link && !value.trim().is_empty()).then(|| {
                                    button(text("Open").size(13))
                                        .padding([8, 10])
                                        .style(style::secondary(c))
                                        .on_press(map(Message::FollowLink(index)))
                                });
                            // A button for each roll in the field, like `1d8+3` or `+5`.
                            let rolls = (field.kind != Kind::Link).then(|| {
                                row(Expression::find(value).iter().map(|expression| {
                                    dice::field_button(c, roll_icon, expression, move |dice| {
                                        map(Message::Roll(index, dice))
                                    })
                                }))
                                .spacing(6)
                            });
                            let input = row![input]
                                .push(open)
                                .push(rolls)
                                .spacing(6)
                                .align_y(Center);

                            column![label(index, field.label()), input]
                                .spacing(6)
                                .into()
                        }
                        Input::Choice(choice) => {
                            let pick = pick_list(
                                field.options.as_slice(),
                                choice.as_ref(),
                                move |value| map(Message::Choice(index, value)),
                            )
                            .placeholder("Choose...")
                            .text_size(14)
                            .padding(8)
                            .style(style::choice(c))
                            .menu_style(style::choice_menu(c))
                            .width(Fill);
                            // A choice of dice, like a hit die, can be rolled too.
                            let rolls = choice.as_deref().map(Expression::find).map(|found| {
                                row(found.iter().map(|expression| {
                                    dice::field_button(c, roll_icon, expression, move |dice| {
                                        map(Message::Roll(index, dice))
                                    })
                                }))
                                .spacing(6)
                            });
                            column![
                                label(index, field.label()),
                                row![pick].push(rolls).spacing(6).align_y(Center),
                            ]
                            .spacing(6)
                            .into()
                        }
                        Input::Check(checked) => {
                            let check = checkbox(*checked)
                                .label(field.label())
                                .on_toggle(move |value| map(Message::Check(index, value)))
                                .style(style::check(c));
                            let header = label(index, String::new());
                            column![header, check].spacing(6).into()
                        }
                        Input::Dice(_) => column![
                            label(index, field.label()),
                            self.dice_view(c, map, index, Some(roll_icon)),
                        ]
                        .spacing(6)
                        .into(),
                        Input::Steps(steps) => column![
                            label(index, field.label()),
                            self.steps_view(c, map, index, steps, &accept),
                        ]
                        .spacing(6)
                        .into(),
                        Input::Long(content) => column![
                            label(index, field.label()),
                            context_menu::area(
                                text_editor(content)
                                    .id(field_id(index))
                                    .on_action(move |action| map(Message::Long(index, action)))
                                    .key_binding(key_binding.clone())
                                    .highlight_with::<links::Highlighter>(
                                        link_colors.clone(),
                                        links::format,
                                    )
                                    .min_height(140)
                                    .size(14)
                                    .padding(8)
                                    .style(style::field_editor(c)),
                                on_menu,
                            )
                            .click_first(content.selection().is_none()),
                        ]
                        .spacing(6)
                        .into(),
                    };

                    (field.width, widget)
                });

        // Half-width fields pair up side by side; full-width fields take a row each.
        let mut rows: Vec<Element<'a, M>> = Vec::new();
        let mut pending: Option<Element<'a, M>> = None;
        for (width, widget) in fields {
            match width {
                Width::Half => match pending.take() {
                    Some(left) => rows.push(pair(left, widget)),
                    None => pending = Some(widget),
                },
                Width::Full => {
                    if let Some(left) = pending.take() {
                        rows.push(pair(left, space().width(Fill).into()));
                    }
                    rows.push(widget);
                }
            }
        }
        if let Some(left) = pending {
            rows.push(pair(left, space().width(Fill).into()));
        }

        let mut content = column![text(self.ty.label()).size(12).color(c.text_muted)]
            .push(column(rows).spacing(20))
            .spacing(16)
            .max_width(1100);

        if self.show_other {
            let mut other = column![
                text("Other").size(14).color(c.text),
                text("Text before the first section, and properties that can't be shown as a field. Kept as they are.")
                    .size(12)
                    .color(c.text_muted),
            ]
            .spacing(6);

            for (key, value) in self.uneditable_keys() {
                other = other.push(text(format!("{key}: {value}")).size(13).color(c.text_muted));
            }

            other = other.push(
                text_editor(&self.other)
                    .on_action(move |action| map(Message::Other(action)))
                    .key_binding(key_binding)
                    .min_height(100)
                    .size(14)
                    .padding(8)
                    .style(style::field_editor(c)),
            );

            content = content.push(space().height(8)).push(other);
        }

        // Room at the bottom so the last field can scroll clear of the + button.
        content = content.push(space().height(64));

        scrollable(container(content).padding([24, 32]).width(Fill))
            .id(SCROLL)
            .height(Fill)
            .into()
    }

    /// The inputs of dice field `index`: how many dice, which die, the modifier, and a
    /// special rule (with how many dice to keep), then a button to roll them if
    /// `roll_icon` is given. Shows why the dice can't be rolled, if they can't.
    pub fn dice_view<'a, M: Clone + 'a>(
        &'a self,
        c: Colors,
        map: fn(Message) -> M,
        index: usize,
        roll_icon: Option<&'a svg::Handle>,
    ) -> Element<'a, M> {
        let Some(Input::Dice(dice)) = self.inputs.get(index) else {
            return space().into();
        };
        let part =
            move |part: fn(String) -> DicePart| move |value| map(Message::Dice(index, part(value)));
        let number = |id: String, placeholder: &'a str, value: &'a str| {
            text_input(placeholder, value)
                .id(id)
                .size(14)
                .padding(8)
                .width(52)
                .style(style::input(c))
        };
        let roll = |expression: &dyn std::fmt::Display| {
            roll_icon.map(|icon| {
                dice::field_button(c, icon, &expression.to_string(), move |dice| {
                    map(Message::Roll(index, dice))
                })
            })
        };

        if let Some(raw) = &dice.raw {
            let input = text_input("Dice, like 2d6+3", raw)
                .id(field_id(index))
                .on_input(part(DicePart::Text))
                .size(14)
                .padding(8)
                .style(style::input(c));
            let rolls = row(Expression::find(raw)
                .into_iter()
                .filter_map(|expression| roll(&expression)))
            .spacing(6);
            return column![
                row![input, rolls].spacing(6).align_y(Center),
                text("These dice can't be split into parts, so they're kept as written. Empty the box to pick the dice instead.")
                    .size(12)
                    .color(c.text_muted),
            ]
            .spacing(4)
            .into();
        }

        // The inputs keep their places in the row, so the keep box and roll button
        // showing up don't make iced build new inputs, which would lose the focus.
        let mut parts = row![
            number(dice_id(index, 0), "1", &dice.count).on_input(part(DicePart::Count)),
            pick_list(dice.dice_list(), dice.die.map(Sides), move |sides| {
                map(Message::Dice(index, DicePart::Die(sides)))
            })
            .placeholder("Die")
            .text_size(14)
            .padding(8)
            .style(style::choice(c))
            .menu_style(style::choice_menu(c)),
            number(dice_id(index, 1), "+0", &dice.modifier).on_input(part(DicePart::Modifier)),
            pick_list(Special::ALL, Some(dice.special), move |special| {
                map(Message::Dice(index, DicePart::Special(special)))
            })
            .text_size(14)
            .padding(8)
            .style(style::choice(c))
            .menu_style(style::choice_menu(c)),
        ]
        .spacing(6)
        .align_y(Center);
        if dice.special.keeps() {
            parts = parts
                .push(number(dice_id(index, 2), "1", &dice.keep).on_input(part(DicePart::Keep)));
        }

        match dice.dice() {
            Ok(Some(dice)) => parts.push(roll(&dice)).wrap().vertical_spacing(6).into(),
            Ok(None) => parts.wrap().vertical_spacing(6).into(),
            Err(error) => column![
                parts.wrap().vertical_spacing(6),
                text(error).size(12).color(c.danger),
            ]
            .spacing(4)
            .into(),
        }
    }

    /// The steps of steps field `index`, each with a box to tick it, then a box to type a
    /// new one in. Enter in a step that links to a note being typed sends `accept`.
    fn steps_view<'a, M: Clone + 'a>(
        &'a self,
        c: Colors,
        map: fn(Message) -> M,
        index: usize,
        steps: &'a Steps,
        accept: &Option<M>,
    ) -> Element<'a, M> {
        let typing = |step: usize| self.focus == Some(index) && self.step == step;
        // Enter finishes the link being typed, if there are notes it could be.
        let submit = move |step: usize| match accept {
            Some(accept) if typing(step) && open_link(steps.text(step)).is_some() => accept.clone(),
            _ => map(Message::StepSubmit(index, step)),
        };
        let small = |label: &'a str, message: Option<Message>| {
            button(text(label).size(13))
                .padding([4, 8])
                .style(style::flat(c, false))
                .on_press_maybe(message.map(map))
        };

        let last = steps.steps.len();
        let mut list = column![].spacing(6);
        for (step, Step { done, text: value }) in steps.steps.iter().enumerate() {
            let input = text_input("A step, or a [[link]] to another note", value)
                .id(step_id(index, step))
                .on_input(move |value| map(Message::StepText(index, step, value)))
                .on_submit(submit(step))
                .size(14)
                .padding(8)
                .style(style::input(c));
            // The input stays in the same place in the row, so the Open button showing up
            // as a link is typed doesn't lose the focus.
            let open = (!links::find(value).is_empty()).then(|| {
                button(text("Open").size(13))
                    .padding([8, 10])
                    .style(style::secondary(c))
                    .on_press(map(Message::FollowStep(index, step)))
            });
            list = list.push(
                row![
                    checkbox(*done)
                        .on_toggle(move |done| map(Message::StepCheck(index, step, done)))
                        .style(style::check(c)),
                    input,
                ]
                .push(open)
                .push(small(
                    "↑",
                    (step > 0).then_some(Message::StepMove(index, step, true)),
                ))
                .push(small(
                    "↓",
                    (step + 1 < last).then_some(Message::StepMove(index, step, false)),
                ))
                .push(small("Remove", Some(Message::StepRemove(index, step))))
                .spacing(6)
                .align_y(Center),
            );
        }

        let draft = text_input("Add a step, or a [[link]] to another note", &steps.draft)
            .id(step_id(index, last))
            .on_input(move |value| map(Message::StepText(index, last, value)))
            .on_submit(submit(last))
            .size(14)
            .padding(8)
            .style(style::input(c));
        list.push(
            row![
                draft,
                button(text("Add").size(13))
                    .padding([8, 10])
                    .style(style::secondary(c))
                    .on_press_maybe(
                        (!steps.draft.trim().is_empty())
                            .then(|| map(Message::StepSubmit(index, last)))
                    ),
            ]
            .spacing(6)
            .align_y(Center),
        )
        .into()
    }
}

/// An operation that moves the keyboard's focus from the input in `ids` that has it to the
/// next (or, with `back`, the one before), going round at the ends, and scrolls the form
/// to show it. With none focused, the first (or last) gets it. Its output is the position
/// in `ids` of the input it focused. With one id, it focuses that input and shows it.
pub fn tab(ids: Vec<String>, back: bool) -> impl Operation<usize> {
    struct Find {
        ids: Vec<Id>,
        back: bool,
        focused: Option<usize>,
        bounds: Vec<Option<Rectangle>>,
    }

    impl Operation<usize> for Find {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<usize>)) {
            operate(self);
        }

        fn focusable(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Focusable) {
            if let Some(position) = self.ids.iter().position(|target| Some(target) == id) {
                self.bounds[position] = Some(bounds);
                if state.is_focused() {
                    self.focused = Some(position);
                }
            }
        }

        fn finish(&self) -> Outcome<usize> {
            let count = self.ids.len();
            if count == 0 {
                return Outcome::None;
            }
            let position = match (self.focused, self.back) {
                (Some(position), false) => (position + 1) % count,
                (Some(position), true) => (position + count - 1) % count,
                (None, false) => 0,
                (None, true) => count - 1,
            };
            Outcome::Chain(Box::new(Focus {
                target: self.ids[position].clone(),
                position,
                bounds: self.bounds[position],
            }))
        }
    }

    struct Focus {
        target: Id,
        position: usize,
        bounds: Option<Rectangle>,
    }

    impl Operation<usize> for Focus {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<usize>)) {
            operate(self);
        }

        fn scrollable(
            &mut self,
            id: Option<&Id>,
            bounds: Rectangle,
            _content_bounds: Rectangle,
            translation: Vector,
            state: &mut dyn Scrollable,
        ) {
            if id != Some(&Id::new(SCROLL)) {
                return;
            }
            let Some(target) = self.bounds else {
                return;
            };
            // Room for the field's label above it.
            let margin = 40.0;
            let top = target.y - bounds.y - margin;
            let bottom = target.y + target.height - bounds.y + margin;
            let y = if top < translation.y {
                Some(top.max(0.0))
            } else if bottom > translation.y + bounds.height {
                // A field taller than the form shows from its top.
                Some((bottom - bounds.height).min(top))
            } else {
                None
            };
            if y.is_some() {
                state.scroll_to(AbsoluteOffset { x: None, y });
            }
        }

        fn focusable(&mut self, id: Option<&Id>, _bounds: Rectangle, state: &mut dyn Focusable) {
            if id == Some(&self.target) {
                state.focus();
            } else {
                state.unfocus();
            }
        }

        fn finish(&self) -> Outcome<usize> {
            Outcome::Some(self.position)
        }
    }

    Find {
        bounds: vec![None; ids.len()],
        ids: ids.into_iter().map(Id::from).collect(),
        back,
        focused: None,
    }
}

/// The kind of field that can edit a front matter value, or `None` for nested values.
pub fn kind_of(value: &Value) -> Option<Kind> {
    match value {
        Value::Bool(_) => Some(Kind::Checkbox),
        Value::Number(_) => Some(Kind::Number),
        Value::Null => Some(Kind::Text),
        Value::String(text) if text.trim().starts_with("[[") && text.trim().ends_with("]]") => {
            Some(Kind::Link)
        }
        Value::String(_) => Some(Kind::Text),
        Value::Sequence(items)
            if items
                .iter()
                .all(|item| matches!(item, Value::String(_) | Value::Number(_))) =>
        {
            Some(Kind::Tags)
        }
        Value::Sequence(_) | Value::Mapping(_) | Value::Tagged(_) => None,
    }
}

fn pair<'a, M: 'a>(left: Element<'a, M>, right: Element<'a, M>) -> Element<'a, M> {
    row![container(left).width(Fill), container(right).width(Fill)]
        .spacing(40)
        .into()
}

pub fn parse_number(text: &str) -> Option<Value> {
    if let Ok(integer) = text.parse::<i64>() {
        return Some(Value::from(integer));
    }
    text.parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .map(Value::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types;

    fn item_type() -> ContentType {
        types::parse(types::STARTER_TYPES[0].1).unwrap()
    }

    #[test]
    fn fills_the_form_and_writes_it_back() {
        let text = "---\nname: The One Ring\nweight: 0.1\nowner: '[[BBEG]]'\nstats_block: {str: 1}\n---\n\nIntro.\n\n## History\n\nForged.\n\n## Secrets\n\nIt whispers.\n";
        let mut form = Form::from_note(item_type(), note::parse(text).unwrap());

        assert_eq!(form.name().as_deref(), Some("The One Ring"));
        assert!(form.show_other);
        assert_eq!(form.other.text().trim_end(), "Intro.");

        // The note's own fields: weight from the front matter, then the Secrets section.
        let own: Vec<_> = form.fields[form.own..]
            .iter()
            .map(|field| (field.key.as_str(), field.kind))
            .collect();
        assert_eq!(own, [("weight", Kind::Number), ("Secrets", Kind::Longtext)]);
        assert_eq!(
            form.uneditable_keys(),
            [("stats_block".into(), "str: 1".into())]
        );

        // Value is the second field.
        form.update(Message::Text(1, "Priceless".into()));
        let written = note::parse(&form.to_markdown().unwrap()).unwrap();

        let keys: Vec<_> = written.front.keys().map(note::value_to_text).collect();
        assert_eq!(
            keys,
            ["type", "name", "weight", "owner", "stats_block", "value"]
        );
        assert_eq!(written.front.get("weight"), Some(&Value::from(0.1)));
        assert_eq!(written.front.get("owner"), Some(&Value::from("[[BBEG]]")));
        assert_eq!(written.front.get("value"), Some(&Value::from("Priceless")));
        assert_eq!(written.preamble, "Intro.");
        let headings: Vec<_> = written
            .sections
            .iter()
            .map(|s| s.heading.as_str())
            .collect();
        assert_eq!(headings, ["History", "Secrets"]);
    }

    #[test]
    fn blank_form_writes_only_the_type() {
        let form = Form::blank(item_type());
        assert_eq!(form.to_markdown().unwrap(), "---\ntype: item\n---\n");
        assert_eq!(form.name(), None);
    }

    #[test]
    fn adds_and_removes_fields_for_one_note() {
        let mut form = Form::blank(item_type());

        assert!(form.add_field("Durability", Kind::Number).is_ok());
        assert!(form.add_field("Lore", Kind::Longtext).is_ok());
        assert!(form.add_field("durability", Kind::Text).is_err());
        assert!(form.add_field("Name", Kind::Text).is_err());
        assert!(form.add_field("  ", Kind::Text).is_err());

        // Empty fields of the note's own are still written, so they come back next time.
        let text = form.to_markdown().unwrap();
        assert_eq!(text, "---\ntype: item\nDurability: null\n---\n\n## Lore\n");

        let mut again = Form::from_note(item_type(), note::parse(&text).unwrap());
        assert_eq!(again.fields.len(), form.fields.len());

        let durability = again.own;
        again.update(Message::Remove(durability));
        assert_eq!(
            again.to_markdown().unwrap(),
            "---\ntype: item\n---\n\n## Lore\n"
        );

        // The name can't be removed: it is the filename.
        assert!(!again.update(Message::Remove(0)));
    }

    #[test]
    fn removes_and_restores_the_types_fields() {
        let mut form = Form::blank(item_type());
        let keys = |form: &Form| -> Vec<String> {
            form.fields.iter().map(|field| field.key.clone()).collect()
        };

        // Remove Stats, then Value; the file lists them in the type's order.
        form.update(Message::Remove(5));
        form.update(Message::Remove(1));
        assert_eq!(keys(&form), ["name", "rarity", "owner", "history"]);
        assert_eq!(form.removed_labels(), ["Value", "Stats"]);

        let text = form.to_markdown().unwrap();
        assert_eq!(
            text,
            "---\ntype: item\nremoved_fields:\n- value\n- stats\n---\n"
        );

        // Reopened, the note still doesn't have them.
        let mut again = Form::from_note(item_type(), note::parse(&text).unwrap());
        assert_eq!(keys(&again), ["name", "rarity", "owner", "history"]);

        // Adding one by name brings it back in its place, whatever kind is asked for.
        again.add_field("value", Kind::Checkbox).unwrap();
        assert_eq!(
            keys(&again),
            ["name", "value", "rarity", "owner", "history"]
        );
        assert_eq!(again.fields[1].kind, Kind::Text);
        assert_eq!(
            again.to_markdown().unwrap(),
            "---\ntype: item\nremoved_fields:\n- stats\n---\n"
        );
    }

    /// An Item type with a Damage dice field and an Attack one.
    fn weapon_type() -> ContentType {
        let mut ty = item_type();
        for key in ["damage", "attack"] {
            ty.fields.push(Field {
                label: None,
                ..Field::for_note(key, Kind::Dice)
            });
        }
        ty
    }

    fn written(form: &Form, key: &str) -> Option<Value> {
        let written = note::parse(&form.to_markdown().unwrap()).unwrap();
        written.front.get(key).cloned()
    }

    #[test]
    fn picks_dice_part_by_part() {
        let mut form = Form::blank(weapon_type());
        let damage = form.fields.iter().position(|f| f.key == "damage").unwrap();
        let attack = damage + 1;
        assert_eq!(written(&form, "damage"), None);

        let part = |form: &mut Form, part| form.update(Message::Dice(damage, part));
        part(&mut form, DicePart::Count("2".into()));
        // Dice need a die before they can be saved.
        assert!(form.to_markdown().unwrap_err().contains("Pick a die"));
        part(&mut form, DicePart::Die(Sides(6)));
        part(&mut form, DicePart::Modifier("+x3".into()));
        assert_eq!(written(&form, "damage"), Some(Value::from("2d6+3")));

        part(&mut form, DicePart::Modifier("-1".into()));
        part(&mut form, DicePart::Special(Special::Advantage));
        assert_eq!(
            written(&form, "damage"),
            Some(Value::from("2d6-1 advantage"))
        );

        // Keeping starts one die short of the dice rolled.
        part(&mut form, DicePart::Count("4".into()));
        part(&mut form, DicePart::Modifier(String::new()));
        part(&mut form, DicePart::Special(Special::KeepHighest));
        assert_eq!(written(&form, "damage"), Some(Value::from("4d6kh3")));
        part(&mut form, DicePart::Keep("5".into()));
        assert!(
            form.to_markdown()
                .unwrap_err()
                .contains("Can't keep 5 of 4")
        );
        part(&mut form, DicePart::Keep("3".into()));

        // A modifier on its own is a d20 roll, read back into its parts.
        form.update(Message::Dice(attack, DicePart::Die(Sides(20))));
        form.update(Message::Dice(attack, DicePart::Modifier("+5".into())));
        assert_eq!(written(&form, "attack"), Some(Value::from("1d20+5")));
    }

    #[test]
    fn reads_dice_fields_and_keeps_others_as_written() {
        let text = "---
type: item
damage: 1d20 + 2 adv
attack: 1d8+1d6
---
";
        let mut form = Form::from_note(weapon_type(), note::parse(text).unwrap());
        let damage = form.fields.iter().position(|f| f.key == "damage").unwrap();
        let Input::Dice(dice) = &form.inputs[damage] else {
            panic!("damage should be dice");
        };
        assert_eq!(
            (
                dice.count.as_str(),
                dice.die,
                dice.modifier.as_str(),
                dice.special
            ),
            ("1", Some(20), "+2", Special::Advantage)
        );
        let Input::Dice(attack) = &form.inputs[damage + 1] else {
            panic!("attack should be dice");
        };
        assert_eq!(attack.raw.as_deref(), Some("1d8+1d6"));
        assert_eq!(
            written(&form, "damage"),
            Some(Value::from("1d20+2 advantage"))
        );
        assert_eq!(written(&form, "attack"), Some(Value::from("1d8+1d6")));

        // Emptied, it can be picked part by part.
        form.update(Message::Dice(damage + 1, DicePart::Text(String::new())));
        form.update(Message::Dice(damage + 1, DicePart::Die(Sides(8))));
        assert_eq!(written(&form, "attack"), Some(Value::from("1d8")));
    }

    #[test]
    fn tab_goes_through_the_dice_parts() {
        let mut form = Form::blank(weapon_type());
        let damage = form.fields.iter().position(|f| f.key == "damage").unwrap();
        let ids = form.tab_ids();
        let at = ids.iter().position(|id| *id == field_id(damage)).unwrap();
        assert_eq!(ids[at + 1], dice_id(damage, 1));
        assert_eq!(ids[at + 2], field_id(damage + 1));

        form.update(Message::Dice(
            damage,
            DicePart::Special(Special::KeepLowest),
        ));
        assert_eq!(form.tab_ids()[at + 2], dice_id(damage, 2));
        form.update(Message::Dice(damage, DicePart::Keep("1".into())));
        assert_eq!(form.focused_id(), Some(dice_id(damage, 2)));
    }

    fn quest_type() -> ContentType {
        let quests = types::STARTER_TYPES
            .iter()
            .find(|(folder, _)| *folder == "quests")
            .unwrap();
        types::parse(quests.1).unwrap()
    }

    fn steps(form: &Form, index: usize) -> Vec<(bool, &str)> {
        match &form.inputs[index] {
            Input::Steps(steps) => steps
                .steps
                .iter()
                .map(|step| (step.done, step.text.as_str()))
                .collect(),
            _ => panic!("field {index} isn't a steps field"),
        }
    }

    #[test]
    fn reads_and_writes_steps() {
        let text = "---\nname: The Big Quest\n---\n\n## Objectives\n\n- [x] Find the map\n- [ ] [[Cross the Marsh]]\n* Talk to [[BBEG|Jerry]]\n";
        let mut form = Form::from_note(quest_type(), note::parse(text).unwrap());
        let objectives = form
            .fields
            .iter()
            .position(|f| f.key == "objectives")
            .unwrap();
        assert_eq!(form.fields[objectives].kind, Kind::Steps);
        assert_eq!(
            steps(&form, objectives),
            [
                (true, "Find the map"),
                (false, "[[Cross the Marsh]]"),
                (false, "Talk to [[BBEG|Jerry]]")
            ]
        );
        assert_eq!(
            form.step_link(objectives, 1).map(|(_, target)| target),
            Some("Cross the Marsh".into())
        );
        assert_eq!(
            form.step_link(objectives, 2).map(|(_, target)| target),
            Some("BBEG".into())
        );
        assert!(form.step_link(objectives, 0).is_none());

        // Tick the second, move the last up, and type a new one.
        form.update(Message::StepCheck(objectives, 1, true));
        form.update(Message::StepMove(objectives, 2, true));
        form.update(Message::StepText(objectives, 3, "Report back".into()));
        let written = form.to_markdown().unwrap();
        assert!(written.ends_with(
            "## Objectives\n\n- [x] Find the map\n- [ ] Talk to [[BBEG|Jerry]]\n- [x] [[Cross the Marsh]]\n- [ ] Report back\n"
        ));

        // The steps a type doesn't list come back as steps, if they are a task list.
        let mut again = Form::from_note(quest_type(), note::parse(&written).unwrap());
        assert_eq!(steps(&again, objectives).len(), 4);
        again.update(Message::StepRemove(objectives, 0));
        assert_eq!(again.focused_id(), Some(step_id(objectives, 0)));
        assert_eq!(steps(&again, objectives)[0].1, "Talk to [[BBEG|Jerry]]");
    }

    #[test]
    fn enter_adds_steps() {
        let mut form = Form::blank(quest_type());
        let objectives = form
            .fields
            .iter()
            .position(|f| f.key == "objectives")
            .unwrap();

        // Enter in the new step box puts it on the list; an empty one does nothing.
        form.update(Message::StepText(objectives, 0, "Find the map".into()));
        assert!(form.update(Message::StepSubmit(objectives, 0)));
        assert!(!form.update(Message::StepSubmit(objectives, 1)));
        assert_eq!(form.focused_id(), Some(step_id(objectives, 1)));

        // Enter in a step makes a blank one after it, which isn't saved while empty.
        form.update(Message::StepSubmit(objectives, 0));
        assert_eq!(
            steps(&form, objectives),
            [(false, "Find the map"), (false, "")]
        );
        assert_eq!(form.focused_id(), Some(step_id(objectives, 1)));
        assert_eq!(
            form.to_markdown().unwrap(),
            "---\ntype: quest\n---\n\n## Objectives\n\n- [ ] Find the map\n"
        );

        // Typing a link in a step suggests notes, and finishing it closes it.
        form.update(Message::StepText(objectives, 1, "Go to [[Marsh".into()));
        assert_eq!(form.link_query(), Some(("Marsh".into(), false)));
        assert!(form.complete_link("Cross the Marsh"));
        assert_eq!(steps(&form, objectives)[1].1, "Go to [[Cross the Marsh]]");
        assert_eq!(form.link_query(), None);
    }

    #[test]
    fn tab_goes_through_the_inputs_in_order() {
        let mut form = Form::blank(quest_type());
        // Status is a choice, which can't take the focus, so Tab skips it.
        let ids = form.tab_ids();
        let objectives = form
            .fields
            .iter()
            .position(|f| f.key == "objectives")
            .unwrap();
        assert_eq!(ids[0], field_id(0));
        assert!(!ids.contains(&field_id(1)));
        assert_eq!(ids.last(), Some(&step_id(objectives, 0)));

        form.tabbed_to(1);
        assert_eq!(form.focused_id().as_ref(), Some(&ids[1]));
        form.tabbed_to(ids.len() - 1);
        assert_eq!(form.focused_id(), Some(step_id(objectives, 0)));
    }

    #[test]
    fn steps_lines_and_lists() {
        assert!(is_step_list("- [ ] One\n\n1. Two\n* [x] Three"));
        assert!(!is_step_list("Some words\n- [ ] One"));
        assert!(!is_step_list("- One\n  - Nested"));
        assert!(is_task_list("- [ ] One\n- [x] Two"));
        assert!(!is_task_list("- One\n- [x] Two"));
        assert!(!is_task_list(""));
        assert_eq!(open_link("Go to [[Mar"), Some(6));
        assert_eq!(open_link("Go to [[Marsh]]"), None);
    }

    #[test]
    fn checks_numbers() {
        let ty = types::parse(
            "type: pc\nfields:\n  - key: level\n    kind: number\n  - key: tags\n    kind: tags\n",
        )
        .unwrap();
        let mut form = Form::blank(ty);

        form.update(Message::Text(0, "three".into()));
        assert!(form.to_markdown().is_err());

        form.update(Message::Text(0, "3".into()));
        form.update(Message::Text(1, "wizard, elf ,".into()));
        assert_eq!(
            form.to_markdown().unwrap(),
            "---\ntype: pc\nlevel: 3\ntags:\n- wizard\n- elf\n---\n"
        );
    }
}
