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

/// Whether front matter `key` is one the form manages itself, rather than a field.
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
            removed,
            inputs,
            other: text_editor::Content::with_text(&note.preamble),
            show_other: false,
            focus: None,
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
            Message::FollowLink(_) => return false,
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

        self.fields.push(Field::for_note(label, kind));
        self.inputs.push(Input::empty(kind));
        Ok(())
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
            _ => None,
        }
    }

    /// Link field `index` and the note name in it.
    pub fn link_field(&self, index: usize) -> Option<(&Field, &str)> {
        match (self.fields.get(index), self.inputs.get(index)) {
            (Some(field), Some(Input::Text(value))) if field.kind == Kind::Link => {
                Some((field, note::unlink(value)))
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
                *value = link.to_string();
                // The link is done, so stop suggesting notes for it.
                self.focus = None;
                true
            }
            _ => false,
        }
    }

    /// The target of the link the cursor is in, in the long text field last typed in.
    pub fn link_under_cursor(&self) -> Option<String> {
        match &self.inputs[self.focus?] {
            Input::Long(content) => links::under_cursor(content),
            _ => None,
        }
    }

    /// Wraps the selection in the long text field last typed in as a `[[link]]`.
    /// Returns whether there was such a field.
    pub fn insert_link(&mut self) -> bool {
        match self.focus.map(|index| &mut self.inputs[index]) {
            Some(Input::Long(content)) => {
                links::insert(content);
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

    /// Shows the form. Long text fields color `[[links]]` with `link_colors`, and Tab
    /// (or Enter in a link field) sends `accept` to finish the link being typed.
    pub fn view<'a, M: Clone + 'a>(
        &'a self,
        c: Colors,
        map: fn(Message) -> M,
        shortcut: fn(&Key, Modifiers) -> Option<M>,
        link_colors: links::Colors,
        accept: Option<M>,
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

                            let input: Element<'a, M> =
                                if field.kind == Kind::Link && !value.trim().is_empty() {
                                    row![
                                        input,
                                        button(text("Open").size(13))
                                            .padding([8, 10])
                                            .style(style::secondary(c))
                                            .on_press(map(Message::FollowLink(index))),
                                    ]
                                    .spacing(6)
                                    .into()
                                } else {
                                    input.into()
                                };

                            column![label(index, field.label()), input]
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
                            let header = label(index, String::new());
                            column![header, check].spacing(6).into()
                        }
                        Input::Long(content) => column![
                            label(index, field.label()),
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
