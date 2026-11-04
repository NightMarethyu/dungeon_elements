//! Roll buttons: a name for two or more roll tables that aren't linked to each other,
//! rolled together with one click, like "Tavern": Inn and shop names, Feel of a place and
//! What's here.
//!
//! They are kept in a hidden file in the campaign, [`FILE`], as ordinary roll tables: a
//! d1 table for each button, under its name, with one column for each table and a link to
//! it in the only row. Rolling that table rolls each link, so a button needs nothing new to
//! roll, and fixing links after a rename covers it like any note. Nobody needs to open the
//! file: the New Roll Button dialog changes it.

use crate::tables::{self, Table};

/// Where a campaign keeps its roll buttons.
pub const FILE: &str = ".dungeon_elements/roll buttons.md";

/// What the file starts with, for anyone who opens it.
const INTRO: &str = "These are the roll buttons made in the Roll Tables list. Each rolls the tables in its row together. Dungeon Elements keeps this file up to date, so there's no need to change it here.\n";

/// One roll button: its name and the tables it rolls, in order, by the links that name
/// them, like `Inn and shop names` or `English names#Female`.
#[derive(Debug, Clone, PartialEq)]
pub struct Button {
    pub name: String,
    pub tables: Vec<String>,
}

impl Button {
    /// The d1 table that rolls every table of the button: one column each, headed by the
    /// table's name, so the result shows which table gave what.
    pub fn table(&self) -> Option<Table> {
        tables::find(&self.markdown()).into_iter().next()
    }

    /// The button's section of [`FILE`].
    fn markdown(&self) -> String {
        let mut header = vec!["d1".to_string()];
        let mut row = vec!["1".to_string()];
        for table in &self.tables {
            header.push(tables::label(table));
            row.push(format!("[[{table}]]"));
        }
        format!("## {}\n\n{}", self.name, tables::write(&[header, row]))
    }

    /// The tables it rolls, by name, like `Inn and shop names, Feel of a place`.
    pub fn summary(&self) -> String {
        self.tables
            .iter()
            .map(|table| tables::label(table))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The roll buttons in [`FILE`]'s `text`, in order.
pub fn read(text: &str) -> Vec<Button> {
    tables::find(text)
        .into_iter()
        .filter_map(|table| {
            let name = table.heading.clone()?;
            let row = table.rows.first()?;
            let tables: Vec<String> = row
                .cells
                .iter()
                .filter_map(|cell| tables::link_targets(cell).into_iter().next())
                .collect();
            (!tables.is_empty()).then_some(Button { name, tables })
        })
        .collect()
}

/// The text of [`FILE`] with these `buttons`.
pub fn write(buttons: &[Button]) -> String {
    let mut text = String::from(INTRO);
    for button in buttons.iter().filter(|button| !button.tables.is_empty()) {
        text.push('\n');
        text.push_str(&button.markdown());
    }
    text
}

/// Why `name` can't name a new roll button (or the one called `old`, when it is being
/// changed), among the `buttons` there are, or `None` if it can.
pub fn name_problem(name: &str, old: Option<&str>, buttons: &[Button]) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return Some("Give the button a name, like Tavern.".into());
    }
    if name.contains(['[', ']', '#', '|']) {
        return Some("A button's name can't have [ ] # or | in it.".into());
    }
    let taken = buttons.iter().any(|button| {
        button.name.eq_ignore_ascii_case(name)
            && old.is_none_or(|old| !old.eq_ignore_ascii_case(&button.name))
    });
    taken.then(|| format!("There is a roll button called {name} already."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tavern() -> Button {
        Button {
            name: "Tavern".into(),
            tables: vec![
                "Inn and shop names".into(),
                "Feel of a place".into(),
                "English names#Female".into(),
            ],
        }
    }

    #[test]
    fn buttons_are_kept_as_d1_tables() {
        let buttons = vec![
            tavern(),
            Button {
                name: "Ruin".into(),
                tables: vec!["Dangerous places".into(), "Secrets".into()],
            },
        ];
        let text = write(&buttons);
        assert!(text.contains("## Tavern"));
        assert!(text.contains("[[English names#Female]]"));
        assert_eq!(read(&text), buttons);

        // Each table is a column, headed by its name, so the result says which gave what.
        let table = tavern().table().unwrap();
        assert_eq!(table.die.as_ref().unwrap().to_string(), "1d1");
        assert_eq!(
            table.columns,
            ["Inn and shop names", "Feel of a place", "English - Female"]
        );
        assert_eq!(
            tavern().summary(),
            "Inn and shop names, Feel of a place, English - Female"
        );
    }

    #[test]
    fn names_must_be_new_and_plain() {
        let buttons = vec![tavern()];
        assert!(name_problem(" ", None, &buttons).is_some());
        assert!(name_problem("Inn #2", None, &buttons).is_some());
        assert!(name_problem("tavern", None, &buttons).is_some());
        // The button being changed keeps its own name.
        assert_eq!(name_problem("Tavern", Some("Tavern"), &buttons), None);
        assert_eq!(name_problem("Market", None, &buttons), None);
    }
}
