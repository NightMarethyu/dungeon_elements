//! Picking the roll tables a field's or a walkthrough question's Inspire Me button rolls
//! on, in Edit Types and Edit > Walkthrough Questions, so nobody types table names.
//!
//! The tables are kept as they always were: names with commas between them, like
//! `Female names, English names#Male`, where `#Male` is the table under that heading.

use iced::widget::{button, column, pick_list, row, text, text_input};
use iced::{Center, Element};

use crate::style;
use crate::tables;
use crate::theme::Colors;

/// A change made in the picker.
#[derive(Debug, Clone)]
pub enum Edit {
    /// The tables are now these, in this order.
    Tables(Vec<String>),
    /// The word on the button after Inspire Me, like Names.
    Inspire(String),
}

/// Something the Add a Table dropdown adds: one table, or every part of one, like
/// English names' Female and Male tables.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    adds: Vec<String>,
    label: String,
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// What the Add a Table dropdown offers: the built-in tables, then the campaign's, each
/// followed by the tables under its headings, and all of those at once. `campaign` is the
/// name of each of the campaign's table notes with the heading of each table in it.
pub fn choices(campaign: Vec<(String, Option<String>)>) -> Vec<Choice> {
    let mut notes: Vec<(String, Vec<String>)> = Vec::new();
    let mut add = |note: &str, heading: Option<&str>| {
        let at = match notes
            .iter()
            .position(|(name, _)| name.eq_ignore_ascii_case(note))
        {
            Some(at) => at,
            None => {
                notes.push((note.to_string(), Vec::new()));
                notes.len() - 1
            }
        };
        if let Some(heading) = heading
            && !notes[at]
                .1
                .iter()
                .any(|known| known.eq_ignore_ascii_case(heading))
        {
            notes[at].1.push(heading.to_string());
        }
    };
    for built_in in tables::BUILT_IN {
        for table in tables::find(built_in.text) {
            add(built_in.name, table.heading.as_deref());
        }
    }
    for (note, heading) in &campaign {
        add(note, heading.as_deref());
    }

    let mut choices = Vec::new();
    for (note, headings) in notes {
        choices.push(Choice {
            label: tables::label(&note),
            adds: vec![note.clone()],
        });
        let parts: Vec<String> = headings
            .iter()
            .map(|heading| format!("{note}#{heading}"))
            .collect();
        for part in &parts {
            choices.push(Choice {
                label: tables::label(part),
                adds: vec![part.clone()],
            });
        }
        if parts.len() > 1 {
            choices.push(Choice {
                label: tables::label(&format!("{note}#{}", headings.join(" and "))),
                adds: parts,
            });
        }
    }
    choices
}

/// The tables in a `table:` value, like `Female names, Male names`.
pub fn split(tables: &str) -> Vec<String> {
    tables
        .split(',')
        .map(str::trim)
        .filter(|table| !table.is_empty())
        .map(String::from)
        .collect()
}

/// The `table:` value for a list of tables.
pub fn join(tables: &[String]) -> String {
    tables.join(", ")
}

/// The tables picked, each a chip that takes it out when clicked, a dropdown to add more,
/// and the box for the word on the button.
pub fn view<'a, M: Clone + 'a>(
    c: Colors,
    picked: &[String],
    inspire: &str,
    choices: &[Choice],
    on: impl Fn(Edit) -> M + Clone + 'a,
) -> Element<'a, M> {
    let with = |change: &dyn Fn(&mut Vec<String>)| {
        let mut tables = picked.to_vec();
        change(&mut tables);
        on(Edit::Tables(tables))
    };

    // Each table is a chip; clicking it takes it out. The first is picked at the start.
    let mut list = row![].spacing(6);
    for (at, table) in picked.iter().enumerate() {
        let label = if picked.len() > 1 && at == 0 {
            format!("{} (first) ✕", tables::label(table))
        } else {
            format!("{} ✕", tables::label(table))
        };
        list = list.push(
            button(text(label).size(12))
                .padding([2, 8])
                .style(style::secondary(c))
                .on_press(with(&|tables| {
                    tables.remove(at);
                })),
        );
    }
    let list = list.wrap().vertical_spacing(6);

    let offered: Vec<Choice> = choices
        .iter()
        .filter(|choice| {
            choice
                .adds
                .iter()
                .any(|table| !picked.iter().any(|known| known.eq_ignore_ascii_case(table)))
        })
        .cloned()
        .collect();
    let current = picked.to_vec();
    let on_add = on.clone();
    let add = pick_list(offered, None::<Choice>, move |choice: Choice| {
        let mut tables = current.clone();
        for table in choice.adds {
            if !tables
                .iter()
                .any(|known| known.eq_ignore_ascii_case(&table))
            {
                tables.push(table);
            }
        }
        on_add(Edit::Tables(tables))
    })
    .placeholder("Add a table...")
    .text_size(13)
    .padding(6)
    .width(240)
    .style(style::choice(c))
    .menu_style(style::choice_menu(c));

    let heading = text("Inspire Me rolls on").size(12).color(c.text_muted);
    // With no tables there is no button, so only the dropdown to add one shows.
    if picked.is_empty() {
        return row![
            heading,
            add,
            text("No tables, so no button.")
                .size(12)
                .color(c.text_muted)
        ]
        .spacing(8)
        .align_y(Center)
        .into();
    }

    let words = match inspire.trim() {
        "" => "Inspire Me".to_string(),
        word => format!("Inspire Me - {word}"),
    };
    let on_word = on.clone();
    let word = row![
        add,
        text("Button says").size(12).color(c.text_muted),
        text_input("a word, like Names", inspire)
            .on_input(move |word| on_word(Edit::Inspire(word)))
            .size(13)
            .padding(6)
            .width(160)
            .style(style::input(c)),
        text(if picked.len() > 1 {
            format!("\"{words}\", with a dropdown")
        } else {
            format!("\"{words}\"")
        })
        .size(12)
        .color(c.text_muted),
    ]
    .spacing(8)
    .align_y(Center);

    column![heading, list, word].spacing(6).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_tables_their_parts_and_all_parts() {
        let choices = choices(vec![
            ("Weather".into(), Some("Summer".into())),
            ("Weather".into(), Some("Winter".into())),
        ]);
        let labels: Vec<String> = choices.iter().map(ToString::to_string).collect();
        let english = labels
            .iter()
            .position(|label| label == "English names")
            .unwrap();
        assert_eq!(
            labels[english..english + 4],
            [
                "English names",
                "English - Female",
                "English - Male",
                "English - Female and Male"
            ]
        );
        let all = &choices[english + 3];
        assert_eq!(all.adds, ["English names#Female", "English names#Male"]);
        assert!(labels.contains(&"Weather - Winter".to_string()));
        assert!(labels.contains(&"Any - Female".to_string()));
    }

    #[test]
    fn splits_and_joins_table_lists() {
        let tables = split(" Female names,English names#Male ,, ");
        assert_eq!(tables, ["Female names", "English names#Male"]);
        assert_eq!(join(&tables), "Female names, English names#Male");
    }
}
