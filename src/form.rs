//! The form a note in a typed folder is edited in: one input per field of its content type.

use iced::keyboard::{Key, Modifiers};
use iced::widget::{
    checkbox, column, container, pick_list, row, scrollable, space, text, text_editor, text_input,
};
use iced::{Element, Fill};
use serde_yaml_ng::{Mapping, Value};

use crate::note::{self, Note, Section};
use crate::style;
use crate::theme::Colors;
use crate::types::{ContentType, Kind, Width};

#[derive(Debug, Clone)]
pub enum Message {
    Text(usize, String),
    Long(usize, text_editor::Action),
    Check(usize, bool),
    Choice(usize, String),
    Other(text_editor::Action),
}

/// What the user typed into one field.
enum Input {
    Text(String),
    Long(text_editor::Content),
    Check(bool),
    Choice(Option<String>),
}

pub struct Form {
    pub ty: ContentType,
    /// One input per field of `ty`, in the same order.
    inputs: Vec<Input>,
    /// The note's front matter as it was read, so keys the type doesn't know are kept in place.
    front: Mapping,
    /// Body text that isn't one of the type's sections, kept as it is.
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

        let inputs = ty
            .fields
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

        let mut other = vec![note.preamble];
        other.extend(
            sections
                .into_iter()
                .flatten()
                .map(|section| format!("## {}\n\n{}", section.heading, section.body)),
        );
        let other = other
            .into_iter()
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        let form = Form {
            inputs,
            other: text_editor::Content::with_text(&other),
            show_other: false,
            front: note.front,
            ty,
        };

        Form {
            show_other: !other.is_empty() || !form.extra_keys().is_empty(),
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
        }
        true
    }

    /// The note's name, from its `name` field, if the type has one and it is filled in.
    pub fn name(&self) -> Option<String> {
        match self.inputs.get(self.ty.name_field()?) {
            Some(Input::Text(name)) if !name.trim().is_empty() => Some(name.trim().to_string()),
            _ => None,
        }
    }

    /// Front matter keys the type has no field for, other than `type`.
    fn extra_keys(&self) -> Vec<(String, String)> {
        self.front
            .iter()
            .filter_map(|(key, value)| {
                let key = note::value_to_text(key);
                let known = key == "type" || self.ty.fields.iter().any(|field| field.key == key);
                (!known).then(|| (key, note::value_to_text(value)))
            })
            .collect()
    }

    /// The value to store in the front matter for field `index`, or `None` to leave it out.
    fn front_value(&self, index: usize) -> Result<Option<Value>, String> {
        let field = &self.ty.fields[index];

        let value = match &self.inputs[index] {
            Input::Long(_) => None,
            Input::Check(checked) => (*checked || self.front.contains_key(field.key.as_str()))
                .then_some(Value::Bool(*checked)),
            Input::Choice(choice) => choice.clone().map(Value::String),
            Input::Text(text) => {
                let text = text.trim();
                if text.is_empty() {
                    None
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
        let field_index = |key: &str| self.ty.fields.iter().position(|field| field.key == key);

        let mut front = Mapping::new();
        front.insert("type".into(), self.ty.id.clone().into());

        // Keep the keys that were already there in their order, updating the ones the form edits.
        for (key, value) in &self.front {
            let name = note::value_to_text(key);
            if name == "type" {
                continue;
            }
            match field_index(&name) {
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

        for (index, field) in self.ty.fields.iter().enumerate() {
            if !self.front.contains_key(field.key.as_str())
                && let Some(value) = self.front_value(index)?
            {
                front.insert(field.key.clone().into(), value);
            }
        }

        let sections: Vec<Section> = self
            .ty
            .fields
            .iter()
            .zip(&self.inputs)
            .filter_map(|(field, input)| match input {
                Input::Long(content) => Some(Section {
                    heading: field.label(),
                    body: content.text(),
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

        let label = |label: String| text(label).size(14).color(c.text);

        let fields =
            self.ty
                .fields
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
                                label(field.label()),
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
                            label(field.label()),
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
                        Input::Check(checked) => column![
                            space().height(26),
                            checkbox(*checked)
                                .label(field.label())
                                .on_toggle(move |value| map(Message::Check(index, value)))
                                .style(style::check(c)),
                        ]
                        .into(),
                        Input::Long(content) => column![
                            label(field.label()),
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
            let extras = self.extra_keys();
            let mut other = column![
                label("Other".into()),
                text("Content this type has no field for. It is kept as it is.")
                    .size(12)
                    .color(c.text_muted),
            ]
            .spacing(6);

            for (key, value) in extras {
                other = other.push(text(format!("{key}: {value}")).size(13).color(c.text_muted));
            }

            other = other.push(
                text_editor(&self.other)
                    .on_action(move |action| map(Message::Other(action)))
                    .key_binding(key_binding)
                    .min_height(100)
                    .padding(8)
                    .style(style::field_editor(c)),
            );

            content = content.push(space().height(8)).push(other);
        }

        scrollable(container(content).padding([24, 32]).width(Fill))
            .height(Fill)
            .into()
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
        let text = "---\nname: The One Ring\nweight: 0.1\nowner: '[[BBEG]]'\n---\n\nIntro.\n\n## History\n\nForged.\n\n## Secrets\n\nIt whispers.\n";
        let mut form = Form::from_note(item_type(), note::parse(text).unwrap());

        assert_eq!(form.name().as_deref(), Some("The One Ring"));
        assert!(form.show_other);
        assert_eq!(form.extra_keys(), [("weight".into(), "0.1".into())]);
        assert_eq!(
            form.other.text().trim_end(),
            "Intro.\n\n## Secrets\n\nIt whispers."
        );

        // Value is the second field.
        form.update(Message::Text(1, "Priceless".into()));
        let written = note::parse(&form.to_markdown().unwrap()).unwrap();

        let keys: Vec<_> = written.front.keys().map(note::value_to_text).collect();
        assert_eq!(keys, ["type", "name", "weight", "owner", "value"]);
        assert_eq!(written.front.get("owner"), Some(&Value::from("[[BBEG]]")));
        assert_eq!(written.front.get("value"), Some(&Value::from("Priceless")));
        assert_eq!(written.preamble, "Intro.");
        let headings: Vec<_> = written
            .sections
            .iter()
            .map(|s| s.heading.as_str())
            .collect();
        assert_eq!(headings, ["Secrets", "History"]);
    }

    #[test]
    fn blank_form_writes_only_the_type() {
        let form = Form::blank(item_type());
        assert_eq!(form.to_markdown().unwrap(), "---\ntype: item\n---\n");
        assert_eq!(form.name(), None);
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
