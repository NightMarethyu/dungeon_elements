//! The form a note in a typed folder is edited in: one input per field.
//!
//! The fields are the content type's, then any the note has on its own: front
//! matter keys and `## ` sections the type doesn't list (added with the main
//! pane's + button, or in another app like Obsidian).

use iced::keyboard::{Key, Modifiers};
use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, space, text, text_editor,
    text_input,
};
use iced::{Center, Element, Fill};
use serde_yaml_ng::{Mapping, Value};

use crate::note::{self, Note, Section};
use crate::style;
use crate::theme::Colors;
use crate::types::{ContentType, Field, Kind, Width};

#[derive(Debug, Clone)]
pub enum Message {
    Text(usize, String),
    Long(usize, text_editor::Action),
    Check(usize, bool),
    Choice(usize, String),
    Other(text_editor::Action),
    /// Removes a field this note has on its own.
    Remove(usize),
}

/// What the user typed into one field.
enum Input {
    Text(String),
    Long(text_editor::Content),
    Check(bool),
    Choice(Option<String>),
}

impl Input {
    fn empty(kind: Kind) -> Self {
        match kind {
            Kind::Longtext => Input::Long(text_editor::Content::new()),
            Kind::Checkbox => Input::Check(false),
            Kind::Choice => Input::Choice(None),
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
    /// One input per field, in the same order.
    inputs: Vec<Input>,
    /// The note's front matter as it was read, so keys stay in place when saved.
    front: Mapping,
    /// The body text before the first `## ` section, kept as it is.
    other: text_editor::Content,
    /// Whether the "Other" area shows: only when the note had something to put there.
    show_other: bool,
}

impl Form {
    /// An empty form for a new note.
    pub fn blank(ty: ContentType) -> Self {
        Self::from_note(ty, Note::default())
    }

    pub fn from_note(ty: ContentType, note: Note) -> Self {
        let mut sections: Vec<Option<Section>> = note.sections.into_iter().map(Some).collect();
        let mut fields = ty.fields.clone();
        let own = fields.len();

        // Front matter keys the type doesn't list become fields of this note, if they can be edited as one.
        for (key, value) in &note.front {
            let key = note::value_to_text(key);
            let listed = key == "type" || fields.iter().any(|field| field.key == key);
            if let (false, Some(kind)) = (listed, kind_of(value)) {
                fields.push(Field::for_note(&key, kind));
            }
        }

        let mut inputs: Vec<Input> = fields
            .iter()
            .map(|field| {
                let value = note.front.get(field.key.as_str());
                let text = value.map(note::value_to_text).unwrap_or_default();

                match field.kind {
                    Kind::Longtext => {
                        let label = field.label();
                        let section = sections.iter_mut().find(|section| {
                            section.as_ref().is_some_and(|section| {
                                section.heading.eq_ignore_ascii_case(&label)
                                    || section.heading.eq_ignore_ascii_case(&field.key)
                            })
                        });
                        let body = section
                            .and_then(Option::take)
                            .map(|section| section.body)
                            .unwrap_or_default();
                        Input::Long(text_editor::Content::with_text(&body))
                    }
                    Kind::Checkbox => Input::Check(match value {
                        Some(Value::Bool(checked)) => *checked,
                        _ => matches!(text.to_lowercase().as_str(), "true" | "yes"),
                    }),
                    Kind::Choice => Input::Choice((!text.is_empty()).then_some(text)),
                    Kind::Link => Input::Text(note::unlink(&text).to_string()),
                    Kind::Text | Kind::Number | Kind::Tags => Input::Text(text),
                }
            })
            .collect();

        // Sections the type doesn't list become long text fields of this note.
        for section in sections.into_iter().flatten() {
            fields.push(Field::for_note(&section.heading, Kind::Longtext));
            inputs.push(Input::Long(text_editor::Content::with_text(&section.body)));
        }

        let form = Form {
            ty,
            fields,
            own,
            inputs,
            other: text_editor::Content::with_text(&note.preamble),
            show_other: false,
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
                }
            }
            Message::Long(index, action) => {
                let edited = action.is_edit();
                if let Some(Input::Long(content)) = self.inputs.get_mut(index) {
                    content.perform(action);
                }
                return edited;
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
                if index < self.own || index >= self.fields.len() {
                    return false;
                }
                let field = self.fields.remove(index);
                self.inputs.remove(index);
                self.front.shift_remove(field.key.as_str());
            }
        }
        true
    }

    /// Adds a field to this note only.
    pub fn add_field(&mut self, label: &str, kind: Kind) -> Result<(), String> {
        let label = label.trim();
        if label.is_empty() {
            return Err("Give the field a name".into());
        }

        let taken = label.eq_ignore_ascii_case("type")
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

        self.fields.push(Field::for_note(label, kind));
        self.inputs.push(Input::empty(kind));
        Ok(())
    }

    /// The note's name, from its `name` field, if the type has one and it is filled in.
    pub fn name(&self) -> Option<String> {
        match self.inputs.get(self.ty.name_field()?) {
            Some(Input::Text(name)) if !name.trim().is_empty() => Some(name.trim().to_string()),
            _ => None,
        }
    }

    /// The index of the field stored under front matter `key`, if there is one.
    fn front_field(&self, key: &str) -> Option<usize> {
        self.fields
            .iter()
            .position(|field| field.kind != Kind::Longtext && field.key == key)
    }

    /// Front matter keys no field covers, other than `type`, such as nested lists.
    fn uneditable_keys(&self) -> Vec<(String, String)> {
        self.front
            .iter()
            .filter_map(|(key, value)| {
                let key = note::value_to_text(key);
                let covered = key == "type" || self.front_field(&key).is_some();
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
            Input::Long(_) => None,
            Input::Check(checked) => {
                (*checked || keep_empty || self.front.contains_key(field.key.as_str()))
                    .then_some(Value::Bool(*checked))
            }
            Input::Choice(choice) => choice.clone().map(Value::String),
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

        // Keep the keys that were already there in their order, updating the ones the form edits.
        for (key, value) in &self.front {
            let name = note::value_to_text(key);
            if name == "type" {
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
            if field.kind != Kind::Longtext
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
            .filter_map(|(index, (field, input))| match input {
                Input::Long(content) => Some(Section {
                    heading: field.label(),
                    body: content.text(),
                    keep_empty: index >= self.own,
                }),
                _ => None,
            })
            .collect();

        note::to_markdown(&front, &sections, &self.other.text())
    }

    pub fn view<'a, M: Clone + 'a>(
        &'a self,
        c: Colors,
        map: fn(Message) -> M,
        shortcut: fn(&Key, Modifiers) -> Option<M>,
    ) -> Element<'a, M> {
        // Every editor sees every key press, so only the focused one handles shortcuts.
        let key_binding = move |key_press: text_editor::KeyPress| {
            if key_press.modifiers.command()
                && matches!(key_press.status, text_editor::Status::Focused { .. })
                && let Some(message) = shortcut(&key_press.key, key_press.modifiers)
            {
                return Some(text_editor::Binding::Custom(message));
            }
            text_editor::Binding::from_key_press(key_press)
        };

        let own = self.own;
        // A field's label, with a button to remove it if only this note has it.
        let label = move |index: usize, label: String| -> Element<'a, M> {
            let label = text(label).size(14).color(c.text);
            if index < own {
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
                            column![
                                label(index, field.label()),
                                text_input(placeholder, value)
                                    .on_input(move |value| map(Message::Text(index, value)))
                                    .size(14)
                                    .padding(8)
                                    .style(style::input(c)),
                            ]
                            .spacing(6)
                            .into()
                        }
                        Input::Choice(choice) => column![
                            label(index, field.label()),
                            pick_list(field.options.as_slice(), choice.as_ref(), move |value| {
                                map(Message::Choice(index, value))
                            })
                            .placeholder("Choose...")
                            .text_size(14)
                            .padding(8)
                            .style(style::choice(c))
                            .menu_style(style::choice_menu(c))
                            .width(Fill),
                        ]
                        .spacing(6)
                        .into(),
                        Input::Check(checked) => {
                            let check = checkbox(*checked)
                                .label(field.label())
                                .on_toggle(move |value| map(Message::Check(index, value)))
                                .style(style::check(c));
                            let header: Element<'a, M> = if index < own {
                                space().height(20).into()
                            } else {
                                label(index, String::new())
                            };
                            column![header, check].spacing(6).into()
                        }
                        Input::Long(content) => column![
                            label(index, field.label()),
                            text_editor(content)
                                .on_action(move |action| map(Message::Long(index, action)))
                                .key_binding(key_binding)
                                .min_height(140)
                                .size(14)
                                .padding(8)
                                .style(style::field_editor(c)),
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
            .height(Fill)
            .into()
    }
}

/// The kind of field that can edit a front matter value, or `None` for nested values.
fn kind_of(value: &Value) -> Option<Kind> {
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

fn parse_number(text: &str) -> Option<Value> {
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

        // The type's own fields can't be removed.
        assert!(!again.update(Message::Remove(0)));
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
