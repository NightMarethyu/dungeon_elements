//! Roll tables: a Markdown table in a note whose first column is a die, like
//!
//! ```markdown
//! | d6  | Weather          |
//! | --- | ---------------- |
//! | 1   | Clear and cold   |
//! | 2-3 | Grey drizzle     |
//! | 4-6 | 1d4 days of rain |
//! ```
//!
//! Rolling the die picks the row. A result that links to another table, like `[[Storms]]`,
//! rolls that table too, and dice in a result, like `1d4`, are rolled. A table with more
//! than one result column gives a whole row, or rolls each column on its own.
//!
//! The Tables list in the sidebar shows every roll table in the campaign. Which tables
//! roll each column on its own is kept in `.dungeon_elements/tables.yaml` in the campaign,
//! so the notes stay plain tables.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use iced::widget::{button, checkbox, column, container, row, space, text, tooltip};
use iced::{Center, Element, Fill, Point};

use crate::dice::Expression;
use crate::links;
use crate::note;
use crate::style;
use crate::theme::Colors;

/// How many tables deep a roll goes, so a table that rolls on itself stops.
const MAX_DEPTH: usize = 10;
/// Dice with more results than this aren't checked for rows that are missing.
const MAX_CHECKED: i64 = 1000;
/// Where the tables set to roll each column are listed, in the campaign folder.
const SETTINGS_FILE: &str = ".dungeon_elements/tables.yaml";

/// A table found in a note: a roll table, or an information table (prices, a calendar, a
/// party list) that isn't rolled.
///
/// A table whose first header is a die is a roll table, unless the campaign's settings
/// make it an information table (see [`Settings::information`]). Any other table is an
/// information table.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// The heading the table is under, if any.
    pub heading: Option<String>,
    /// The die in its first header, like `d6` or `2d6`, if it has one.
    pub die: Option<Expression>,
    /// The headers of its other columns, or of every column without a die.
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    /// Rows whose first cell isn't a number or a range, as written.
    pub unread: Vec<String>,
    /// Where the table is in the note's text.
    pub range: Range<usize>,
    /// Every row's cells as written, the header first, for the table editor.
    pub written: Vec<Vec<String>>,
    /// The tables its results link to that aren't rolled, as [`link_key`]s. This is kept
    /// in the campaign's settings, not the note.
    pub not_rolled: BTreeSet<String>,
    /// How each result column joins the one before it when the columns are rolled on
    /// their own, from the campaign's settings. A column with none set takes a space.
    pub joins: Vec<Join>,
    /// The tables rolled with it, from the table editor's Also roll and Rolled after.
    pub links: Links,
    /// What is rolled in its place when it is rolled: the d1 table that rolls it and its
    /// linked tables, kept in [`LINKED_FILE`].
    pub linked: Option<Box<Table>>,
}

/// One row of a roll table: the numbers it covers and its results.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub low: i64,
    pub high: i64,
    pub cells: Vec<String>,
}

impl Table {
    /// The table's name: its heading, or the name of the note it is in.
    pub fn name(&self, note: &str) -> String {
        self.heading.clone().unwrap_or_else(|| note.to_string())
    }

    /// The row the die roll `value` picks.
    fn row(&self, value: i64) -> Option<&Row> {
        self.rows
            .iter()
            .find(|row| (row.low..=row.high).contains(&value))
    }

    /// Whether it has more than one result column, so it can roll each on its own.
    pub fn has_columns(&self) -> bool {
        self.columns.len() > 1
    }

    /// Whether its first column is a die, so it can be rolled.
    pub fn has_die(&self) -> bool {
        self.die.is_some()
    }
}

/// The tables in a note's `text` whose first column is a die, in order: its roll tables,
/// unless the campaign's settings make some information tables.
pub fn find(text: &str) -> Vec<Table> {
    find_all(text).into_iter().filter(Table::has_die).collect()
}

/// Every table in a note's `text`, roll tables and information tables, in order.
pub fn find_all(text: &str) -> Vec<Table> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    let offset = match note::split_front_matter(text) {
        Ok((_, body)) => text.len() - body.len(),
        Err(_) => 0,
    };
    let body = &text[offset..];

    let mut tables = Vec::new();
    let mut heading: Option<String> = None;
    let mut in_heading = false;
    let mut heading_text = String::new();
    // The table being read: where it starts, and its rows of cells so far.
    let mut table: Option<(usize, Vec<Vec<String>>)> = None;

    let parser = Parser::new_ext(body, Options::ENABLE_TABLES).into_offset_iter();
    for (event, range) in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                let name = heading_text.trim();
                heading = (!name.is_empty()).then(|| name.to_string());
            }
            Event::Text(part) | Event::Code(part) if in_heading => heading_text.push_str(&part),
            Event::Start(Tag::Table(_)) => table = Some((range.start, Vec::new())),
            Event::Start(Tag::TableHead | Tag::TableRow) => {
                if let Some((_, rows)) = &mut table {
                    rows.push(Vec::new());
                }
            }
            Event::Start(Tag::TableCell) => {
                if let Some((_, rows)) = &mut table
                    && let Some(cells) = rows.last_mut()
                {
                    cells.push(cell_text(&body[range]));
                }
            }
            Event::End(TagEnd::Table) => {
                if let Some((start, rows)) = table.take()
                    && let Some(found) =
                        read(heading.clone(), rows, offset + start..offset + range.end)
                {
                    tables.push(found);
                }
            }
            _ => {}
        }
    }
    tables
}

/// A cell's text as written, without the pipes around it, with `\|` as `|` and a `<br>`
/// as a new line.
fn cell_text(raw: &str) -> String {
    let text = raw
        .trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .trim()
        .replace("\\|", "|");
    BREAKS.iter().fold(text, |text, br| text.replace(br, "\n"))
}

/// How a line break in a cell can be written. [`write`] writes the first.
const BREAKS: [&str; 6] = ["<br>", "<br/>", "<br />", "<BR>", "<BR/>", "<BR />"];

/// A cell's text as Markdown: a `|` as `\|` so it doesn't end the cell, and each line
/// break as `<br>`, which Obsidian shows as one.
pub fn cell_markdown(cell: &str) -> String {
    cell.trim()
        .replace("\r\n", "\n")
        .replace('|', "\\|")
        .replace('\n', BREAKS[0])
}

/// A Markdown table of `rows` of cells, the header first, with the columns padded so it
/// reads well as text too. Each cell is written with [`cell_markdown`].
pub fn write(rows: &[Vec<String>]) -> String {
    let count = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|cells| {
            (0..count)
                .map(|index| {
                    cells
                        .get(index)
                        .map_or(String::new(), |cell| cell_markdown(cell))
                })
                .collect()
        })
        .collect();
    let widths: Vec<usize> = (0..count)
        .map(|index| {
            rows.iter()
                .map(|cells| cells[index].chars().count())
                .max()
                .unwrap_or(0)
                .max(3)
        })
        .collect();
    let line = |cells: Vec<String>| format!("| {} |\n", cells.join(" | "));
    let pad = |cells: &[String]| {
        cells
            .iter()
            .zip(&widths)
            .map(|(cell, width)| {
                let extra = width.saturating_sub(cell.chars().count());
                format!("{cell}{}", " ".repeat(extra))
            })
            .collect::<Vec<_>>()
    };
    let mut out = String::new();
    let mut rows = rows.iter();
    if let Some(header) = rows.next() {
        out.push_str(&line(pad(header)));
    }
    out.push_str(&line(
        widths.iter().map(|width| "-".repeat(*width)).collect(),
    ));
    for cells in rows {
        out.push_str(&line(pad(cells)));
    }
    out
}

/// The table in `rows` of cells (the header first): a roll table if its first header is
/// a die.
fn read(heading: Option<String>, rows: Vec<Vec<String>>, range: Range<usize>) -> Option<Table> {
    let written = rows.clone();
    let mut rows = rows.into_iter();
    let header = rows.next()?;
    let Some(die) = die(header.first()?) else {
        return Some(Table {
            heading,
            die: None,
            columns: header,
            rows: Vec::new(),
            unread: Vec::new(),
            range,
            written,
            not_rolled: BTreeSet::new(),
            joins: Vec::new(),
            links: Links::default(),
            linked: None,
        });
    };
    let (_, highest) = die.bounds();
    let mut table = Table {
        heading,
        die: Some(die),
        columns: header[1..].to_vec(),
        rows: Vec::new(),
        unread: Vec::new(),
        range,
        written,
        not_rolled: BTreeSet::new(),
        joins: Vec::new(),
        links: Links::default(),
        linked: None,
    };
    for cells in rows {
        let Some(first) = cells.first() else {
            continue;
        };
        match numbers(first, highest) {
            Some((low, high)) => table.rows.push(Row {
                low,
                high,
                cells: cells[1..].to_vec(),
            }),
            None if first.is_empty() && cells.iter().all(String::is_empty) => {}
            None => table.unread.push(first.clone()),
        }
    }
    Some(table)
}

/// The die a header names, like `d6`, `2d6` or `d%`. A modifier like `+2` isn't one.
pub fn die(header: &str) -> Option<Expression> {
    let header = header.trim();
    if header.starts_with(['+', '-']) || !header.contains(['d', 'D']) {
        return None;
    }
    Expression::parse(header).ok()
}

/// The numbers a row's first cell covers: `4`, `2-3`, `2–3`, `01-05`, `5+` (5 and up),
/// with `00` as 100 on a d100. `highest` is the most the die rolls.
pub fn numbers(cell: &str, highest: i64) -> Option<(i64, i64)> {
    let cell = cell.trim().trim_end_matches('.').trim();
    let number = |part: &str| -> Option<i64> {
        let part = part.trim();
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let value: i64 = part.parse().ok()?;
        Some(if value == 0 && highest == 100 {
            100
        } else {
            value
        })
    };
    if let Some(low) = cell.strip_suffix('+') {
        let low = number(low)?;
        return Some((low, highest.max(low)));
    }
    match cell.split_once(['-', '–', '—']) {
        Some((low, high)) => {
            let (low, high) = (number(low)?, number(high)?);
            Some((low.min(high), low.max(high)))
        }
        None => number(cell).map(|value| (value, value)),
    }
}

/// What is wrong with the roll tables in a note's `text`, one line each: rows the die
/// can't pick, and numbers it can roll that no row has. `information` says which tables,
/// by the heading they are under, were made information tables, which aren't rolled.
pub fn problems(text: &str, note: &str, information: &dyn Fn(Option<&str>) -> bool) -> Vec<String> {
    let mut problems = Vec::new();
    for table in find(text) {
        if information(table.heading.as_deref()) {
            continue;
        }
        let name = table.name(note);
        for cell in &table.unread {
            problems.push(format!(
                "A row of the {name} table starts with \"{cell}\", not a number or a range like 2-3, so it can't be rolled"
            ));
        }
        let Some(die) = &table.die else {
            continue;
        };
        let (low, high) = die.bounds();
        if high - low > MAX_CHECKED || table.rows.is_empty() {
            continue;
        }
        let missing = ranges((low..=high).filter(|value| table.row(*value).is_none()));
        if !missing.is_empty() {
            problems.push(format!(
                "The {name} table has no row for {}, which its {} can roll",
                list(&missing),
                die
            ));
        }
    }
    problems
}

/// `items` as a list to read: `1`, `1 or 3`, `1, 3 or 4-6`.
fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

/// `values`, in order, as runs: `1`, `4-6`.
fn ranges(values: impl Iterator<Item = i64>) -> Vec<String> {
    let mut runs: Vec<(i64, i64)> = Vec::new();
    for value in values {
        match runs.last_mut() {
            Some((_, end)) if *end + 1 == value => *end = value,
            _ => runs.push((value, value)),
        }
    }
    runs.into_iter()
        .map(|(start, end)| {
            if start == end {
                start.to_string()
            } else {
                format!("{start}-{end}")
            }
        })
        .collect()
}

/// What one roll on a table gave.
#[derive(Debug, Clone, PartialEq)]
pub struct Rolled {
    pub name: String,
    /// The die, like `1d6`.
    pub die: String,
    /// What the die rolled, one for each column when each column is rolled.
    pub values: Vec<i64>,
    /// Each result column's header and what it gave, with links and dice rolled.
    pub results: Vec<(String, String)>,
    pub each_column: bool,
    /// How each result joins the one before it, when they are put in a row.
    pub joins: Vec<Join>,
    /// The tables its results rolled on, in order.
    pub inner: Vec<Rolled>,
}

impl Rolled {
    /// The result as one line: the result, the columns' results in a row, or each
    /// column's header and result.
    pub fn text(&self) -> String {
        if self.results.len() == 1 || self.each_column {
            joined(self.results.iter().enumerate().map(|(index, (_, value))| {
                (
                    self.joins.get(index).copied().unwrap_or_default(),
                    value.as_str(),
                )
            }))
        } else {
            let filled = self.results.iter().filter(|(_, value)| !value.is_empty());
            filled
                .map(|(column, value)| format!("{column}: {value}"))
                .collect::<Vec<_>>()
                .join("; ")
        }
    }

    /// A name for a note made from the result: the result, or with a whole row, its
    /// first column's.
    pub fn note_name(&self) -> String {
        let name = if self.results.len() > 1 && !self.each_column {
            self.results
                .first()
                .map(|(_, value)| value.clone())
                .unwrap_or_default()
        } else {
            self.text()
        };
        name.trim().trim_end_matches('.').trim().to_string()
    }

    /// The rest of a whole row, for a note made from it: each column after the first that
    /// gave something, with its header, like `("Wants", "Revenge on her brother")`. A roll
    /// button's columns are its tables, so a note gets every table's result.
    pub fn note_details(&self) -> Vec<(String, String)> {
        if self.results.len() < 2 || self.each_column {
            return Vec::new();
        }
        self.results[1..]
            .iter()
            .filter(|(_, value)| !value.trim().is_empty())
            .map(|(column, value)| (column.trim().to_string(), value.trim().to_string()))
            .collect()
    }

    /// For the list of earlier rolls: `Weather (6): Grey drizzle`.
    pub fn line(&self) -> String {
        if self.values.is_empty() {
            return format!("{}: {}", self.name, self.text());
        }
        let values: Vec<String> = self.values.iter().map(i64::to_string).collect();
        format!("{} ({}): {}", self.name, values.join(", "), self.text())
    }
}

/// Results in a row, each joined to the one before it as its [`Join`] says: with a space,
/// with nothing, or with a comma. A result starting with a hyphen joins the one before it
/// with nothing whatever its join (Ash and -ford make Ashford), and one starting with a
/// comma follows it straight after. Empty results are left out.
fn joined<'a>(values: impl Iterator<Item = (Join, &'a str)>) -> String {
    let mut text = String::new();
    for (join, value) in values {
        let (join, value) = if let Some(rest) = value
            .strip_prefix('-')
            .map(|rest| rest.trim_start_matches('-'))
            .filter(|rest| !rest.trim().is_empty())
        {
            (Join::NoSpace, rest)
        } else if let Some(rest) = value.strip_prefix(',') {
            (Join::Comma, rest.trim_start())
        } else {
            (join, value)
        };
        if value.trim().is_empty() || value.trim() == "-" {
            continue;
        }
        if !text.is_empty() {
            text.push_str(join.separator());
        }
        text.push_str(value);
    }
    text
}

/// How a result joins the one before it when results are put in a row.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Join {
    /// With a space: Bjorn Ashby.
    #[default]
    Space,
    /// With nothing between them: Bjorn and sson make Bjornsson.
    NoSpace,
    /// With a comma and a space, for a list: rope, lantern, chalk.
    Comma,
}

impl Join {
    pub const ALL: [Join; 3] = [Join::Space, Join::NoSpace, Join::Comma];

    /// What goes between it and the result before it.
    pub fn separator(self) -> &'static str {
        match self {
            Join::Space => " ",
            Join::NoSpace => "",
            Join::Comma => ", ",
        }
    }

    /// What starts a cell of the linked tables' file to say how it joins.
    fn prefix(self) -> &'static str {
        match self {
            Join::Space => "",
            Join::NoSpace => "-",
            Join::Comma => ", ",
        }
    }
}

impl std::fmt::Display for Join {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Join::Space => "a space",
            Join::NoSpace => "no space",
            Join::Comma => "a comma",
        })
    }
}

/// Where the tables each table rolls with are kept, in the campaign folder: one d1 table
/// for each table that has some, with one link a column, so rolling it rolls them all.
/// Nobody needs to open it: the table editor's Also roll and Rolled after change it.
pub const LINKED_FILE: &str = ".dungeon_elements/linked tables.md";
/// The headers of the linked tables' columns: the tables rolled first, the table itself,
/// and the tables rolled after it.
const FIRST: &str = "Rolled first";
const THIS: &str = "This table";
const ALSO: &str = "Also rolled";

/// The tables a table is rolled with, by the links that name them, like `Norse names#Male`,
/// each with how its result joins the one before it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Links {
    /// Rolled first, in order: Norse surnames need a father's name.
    pub before: Vec<(String, Join)>,
    /// How the table's own result joins the ones rolled first.
    pub join: Join,
    /// Rolled after it, in order: a Norse given name, then a surname.
    pub after: Vec<(String, Join)>,
}

impl Links {
    pub fn is_empty(&self) -> bool {
        self.before.is_empty() && self.after.is_empty()
    }

    /// The d1 table that rolls the table, linked to as `own`, with these tables.
    pub fn table(&self, own: &str) -> Option<Table> {
        let section = self.markdown(own, own)?;
        find(&section).into_iter().next()
    }

    /// The section of [`LINKED_FILE`] for the table linked to as `own`, under `heading`:
    /// none if it has no links.
    pub fn markdown(&self, heading: &str, own: &str) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let cell = |target: &str, join: Join| format!("{}[[{target}]]", join.prefix());
        let mut header = vec!["d1".to_string()];
        let mut row = vec!["1".to_string()];
        for (target, join) in &self.before {
            header.push(FIRST.into());
            row.push(cell(target, *join));
        }
        header.push(THIS.into());
        row.push(cell(own, self.join));
        for (target, join) in &self.after {
            header.push(ALSO.into());
            row.push(cell(target, *join));
        }
        // A heading can't hold a link's brackets or a `#`.
        let heading = heading.replace(['[', ']', '#'], " ");
        Some(format!(
            "## {}\n\n{}",
            heading.trim(),
            write(&[header, row])
        ))
    }

    /// A short line for the Roll Tables list, like `After Norse names - Male` and
    /// `Also rolls Norse surnames`. None without links.
    pub fn summary(&self) -> Option<String> {
        let names = |links: &[(String, Join)]| {
            links
                .iter()
                .map(|(target, _)| label(target))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut parts = Vec::new();
        if !self.before.is_empty() {
            parts.push(format!("After {}", names(&self.before)));
        }
        if !self.after.is_empty() {
            parts.push(format!("Also rolls {}", names(&self.after)));
        }
        (!parts.is_empty()).then(|| parts.join(". "))
    }
}

/// The tables in [`LINKED_FILE`]'s `text`: for each, the link to the table it is for, as
/// written (like `Norse surnames`), its links, and the d1 table that rolls them.
pub fn read_linked(text: &str) -> Vec<(String, Links, Table)> {
    let mut found = Vec::new();
    for table in find(text) {
        let Some(row) = table.rows.first() else {
            continue;
        };
        let mut links = Links::default();
        let mut own = None;
        for (header, cell) in table.columns.iter().zip(&row.cells) {
            let cell = cell.trim();
            let (join, rest) = if let Some(rest) = cell.strip_prefix('-') {
                (Join::NoSpace, rest)
            } else if let Some(rest) = cell.strip_prefix(',') {
                (Join::Comma, rest)
            } else {
                (Join::Space, cell)
            };
            let Some(target) = link_targets(rest).into_iter().next() else {
                continue;
            };
            match header.trim() {
                header if header.eq_ignore_ascii_case(THIS) && own.is_none() => {
                    own = Some(target);
                    links.join = join;
                }
                _ if own.is_none() => links.before.push((target, join)),
                _ => links.after.push((target, join)),
            }
        }
        if let Some(own) = own
            && !links.is_empty()
        {
            found.push((own, links, table));
        }
    }
    found
}

/// The text of [`LINKED_FILE`] with these `sections`, each the table's name, the link to
/// it and its links.
pub fn write_linked(sections: &[(String, String, Links)]) -> String {
    let mut text = String::from(
        "These are the tables rolled with other roll tables, set with Also roll and Rolled after in the table editor. Dungeon Elements keeps this file up to date, so there's no need to change it here.\n",
    );
    for (heading, own, links) in sections {
        if let Some(section) = links.markdown(heading, own) {
            text.push('\n');
            text.push_str(&section);
        }
    }
    text
}

/// Finds the table a link names, for results that roll on other tables: its name, the
/// table, and whether it rolls each column.
pub type Lookup<'a> = dyn FnMut(&str) -> Option<(String, Table, bool)> + 'a;

/// Rolls `table`, called `name`, with `rng`. `lookup` finds the tables its results link to.
pub fn roll(
    name: &str,
    table: &Table,
    each_column: bool,
    rng: &mut fastrand::Rng,
    lookup: &mut Lookup<'_>,
) -> Rolled {
    roll_at(name, table, each_column, rng, lookup, 0, &[])
}

/// Rolls `table`, or with tables linked to it (Also roll and Rolled after), its
/// [`Table::linked`] table in its place. `linking` names the tables whose linked tables
/// are being rolled already: those are rolled on their own, so the link from a linked
/// table back to its own table rolls it, and two tables linked to each other stop.
fn roll_at(
    name: &str,
    table: &Table,
    each_column: bool,
    rng: &mut fastrand::Rng,
    lookup: &mut Lookup<'_>,
    depth: usize,
    linking: &[String],
) -> Rolled {
    let key = name.trim().to_lowercase();
    if depth < MAX_DEPTH
        && !linking.contains(&key)
        && let Some(linked) = &table.linked
    {
        let mut linking = linking.to_vec();
        linking.push(key);
        // Each table it rolled shows its own die and number, so the d1's aren't shown.
        let mut rolled = roll_at(name, linked, true, rng, lookup, depth + 1, &linking);
        rolled.die = String::new();
        rolled.values = Vec::new();
        return rolled;
    }
    let each_column = each_column && table.has_columns();
    // An information table has no die, so nothing is picked from it.
    let die = table
        .die
        .clone()
        .unwrap_or_else(|| Expression::parse("d1").expect("d1"));
    let columns = if table.columns.is_empty() {
        vec![String::new()]
    } else {
        table.columns.clone()
    };
    let mut values = Vec::new();
    let mut results = Vec::new();
    let mut inner = Vec::new();
    let mut picked: Option<(i64, Option<&Row>)> = None;
    for (index, column) in columns.iter().enumerate() {
        let (value, row) = match picked {
            Some(picked) if !each_column => picked,
            _ => {
                let value = die.total(rng);
                values.push(value);
                let found = (value, table.row(value));
                picked = Some(found);
                found
            }
        };
        let result = match row {
            Some(row) => {
                let cell = row.cells.get(index).map_or("", String::as_str);
                expand(
                    cell,
                    column,
                    &table.not_rolled,
                    rng,
                    lookup,
                    depth,
                    linking,
                    &mut inner,
                )
            }
            None => format!("(no row for {value})"),
        };
        results.push((column.clone(), result));
    }
    Rolled {
        name: name.to_string(),
        die: die.to_string(),
        values,
        results,
        each_column,
        joins: table.joins.clone(),
        inner,
    }
}

/// A result's text with its links to tables rolled (unless they are in `not_rolled`), its
/// other links shown as their names, and its dice rolled.
///
/// A link in a column rolls only the column of the same name in the table it links to,
/// when that table has one, so a name's family name can come from another table than its
/// first name. The tables named in `linking` are rolled without their linked tables.
#[allow(clippy::too_many_arguments)]
fn expand(
    cell: &str,
    column: &str,
    not_rolled: &BTreeSet<String>,
    rng: &mut fastrand::Rng,
    lookup: &mut Lookup<'_>,
    depth: usize,
    linking: &[String],
    inner: &mut Vec<Rolled>,
) -> String {
    let mut out = String::new();
    let mut last = 0;
    for link in links::find(cell) {
        out.push_str(&roll_dice(&cell[last..link.range.start], rng));
        last = link.range.end;
        let written = cell[link.range.clone()]
            .trim_start_matches('!')
            .trim_start_matches("[[")
            .trim_end_matches("]]");
        let (target, shown) = match written.split_once('|') {
            Some((target, shown)) => (target.trim(), shown.trim()),
            None => (written.trim(), written.trim()),
        };
        let rolls = depth < MAX_DEPTH && !not_rolled.contains(&link_key(target));
        let table = rolls.then(|| lookup(target)).flatten();
        match table {
            Some((name, table, each_column)) => match columns_named(&table, column) {
                Some(columns) => {
                    let table = Table {
                        columns: table.columns[columns.clone()].to_vec(),
                        joins: table
                            .joins
                            .get(columns.clone())
                            .unwrap_or_default()
                            .to_vec(),
                        linked: None,
                        rows: table
                            .rows
                            .iter()
                            .map(|row| Row {
                                cells: row.cells.get(columns.clone()).unwrap_or_default().to_vec(),
                                ..row.clone()
                            })
                            .collect(),
                        ..table
                    };
                    let mut rolled = roll_at(&name, &table, false, rng, lookup, depth + 1, linking);
                    rolled.each_column = true;
                    out.push_str(&rolled.text());
                    inner.push(rolled);
                }
                None => {
                    let rolled =
                        roll_at(&name, &table, each_column, rng, lookup, depth + 1, linking);
                    out.push_str(&rolled.text());
                    inner.push(rolled);
                }
            },
            None => out.push_str(shown),
        }
    }
    out.push_str(&roll_dice(&cell[last..], rng));
    out
}

/// The columns of `table` that a link from the column headed `header` rolls: the one with
/// the same header, and the ones after it that join it (each result starts with a hyphen,
/// like a Norse name's -sson). None when the table has no column of that name.
fn columns_named(table: &Table, header: &str) -> Option<Range<usize>> {
    if header.trim().is_empty() || !table.has_columns() {
        return None;
    }
    let start = table
        .columns
        .iter()
        .position(|column| column.trim().eq_ignore_ascii_case(header.trim()))?;
    let mut end = start + 1;
    while end < table.columns.len()
        && (table.joins.get(end) == Some(&Join::NoSpace)
            || table.rows.iter().all(|row| {
                row.cells
                    .get(end)
                    .is_some_and(|cell| cell.trim_start().starts_with('-'))
            }))
    {
        end += 1;
    }
    Some(start..end)
}

/// A link's target as it is compared to say whether the link rolls its table:
/// `Norse names#Female` as `norse names#female`.
pub fn link_key(target: &str) -> String {
    let target = target.split('|').next().unwrap_or(target);
    target
        .split('#')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("#")
        .to_lowercase()
}

/// The targets of the links in `text`, as written, like `Storms` or `Names#Female`.
pub fn link_targets(text: &str) -> Vec<String> {
    links::find(text)
        .into_iter()
        .map(|link| {
            let written = text[link.range.clone()]
                .trim_start_matches('!')
                .trim_start_matches("[[")
                .trim_end_matches("]]");
            written
                .split('|')
                .next()
                .unwrap_or(written)
                .trim()
                .to_string()
        })
        .collect()
}

/// `text` with each dice expression in it, like `1d4`, replaced by what it rolled.
fn roll_dice(text: &str, rng: &mut fastrand::Rng) -> String {
    let mut out = String::new();
    let mut last = 0;
    for (range, expression) in Expression::spans(text) {
        out.push_str(&text[last..range.start]);
        out.push_str(&expression.total(rng).to_string());
        last = range.end;
    }
    out.push_str(&text[last..]);
    out
}

/// The key a table is known by in the settings file: the note's path in the campaign,
/// and the table's heading.
pub fn key(root: &Path, path: &Path, heading: Option<&str>) -> String {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    match heading {
        Some(heading) => format!("{relative}#{heading}"),
        None => relative,
    }
}

/// Which of a campaign's tables roll each column on their own, by [`key`].
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub each_column: BTreeSet<String>,
    /// The table last picked for a field or question that rolls on several, by where it
    /// is, like `npcs/name`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub picked: BTreeMap<String, String>,
    /// For each table, the tables its results link to that it doesn't roll.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub not_rolled: BTreeMap<String, BTreeSet<String>>,
    /// The groups of built-in tables open in the Roll Tables list, by [`Group::key`].
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub open_groups: BTreeSet<String>,
    /// For each table, how its result columns join the one before them, by the column's
    /// header, when not with a space.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub joins: BTreeMap<String, BTreeMap<String, Join>>,
    /// The tables, by [`key`], whose first column is a die but that are information
    /// tables: not rolled or listed. Any other table with a die is a roll table.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub information: BTreeSet<String>,
}

impl Settings {
    /// The settings of the campaign at `root`, or none set if it has no settings file.
    pub fn load(root: &Path) -> Self {
        std::fs::read_to_string(root.join(SETTINGS_FILE))
            .ok()
            .and_then(|yaml| serde_yaml_ng::from_str(&yaml).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, root: &Path) -> Result<(), String> {
        let path = root.join(SETTINGS_FILE);
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
        }
        let yaml = serde_yaml_ng::to_string(self).map_err(|error| error.to_string())?;
        std::fs::write(&path, yaml).map_err(|error| error.to_string())
    }

    /// Sets the tables that the results of table `key` link to without rolling them.
    pub fn set_not_rolled(&mut self, key: &str, not_rolled: BTreeSet<String>) {
        if not_rolled.is_empty() {
            self.not_rolled.remove(key);
        } else {
            self.not_rolled.insert(key.to_string(), not_rolled);
        }
    }

    /// Sets how the result columns of table `key`, headed `columns`, join the one before
    /// them.
    pub fn set_joins(&mut self, key: &str, columns: &[String], joins: &[Join]) {
        let set: BTreeMap<String, Join> = columns
            .iter()
            .zip(joins)
            .skip(1)
            .filter(|(_, join)| **join != Join::Space)
            .map(|(column, join)| (column.trim().to_string(), *join))
            .collect();
        if set.is_empty() {
            self.joins.remove(key);
        } else {
            self.joins.insert(key.to_string(), set);
        }
    }

    /// How each of the result columns of table `key`, headed `columns`, joins the one
    /// before it.
    pub fn joins(&self, key: &str, columns: &[String]) -> Vec<Join> {
        let set = self.joins.get(key);
        columns
            .iter()
            .map(|column| {
                set.and_then(|set| set.get(column.trim()))
                    .copied()
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Sets whether the table `key`, whose first column is a die, is an information table.
    pub fn set_information(&mut self, key: &str, information: bool) {
        if information {
            self.information.insert(key.to_string());
        } else {
            self.information.remove(key);
        }
    }

    /// Sets whether the table `key` rolls each column on its own.
    pub fn set(&mut self, key: &str, each_column: bool) {
        if each_column {
            self.each_column.insert(key.to_string());
        } else {
            self.each_column.remove(key);
        }
    }

    /// Which of `tables` the field or question `key` rolls on: the one picked for it
    /// last, or the first.
    pub fn picked<'a>(&self, key: &str, tables: &'a [String]) -> Option<&'a String> {
        self.picked
            .get(key)
            .and_then(|picked| {
                tables
                    .iter()
                    .find(|table| table.eq_ignore_ascii_case(picked))
            })
            .or_else(|| tables.first())
    }
}

/// How a table that a field or question rolls on shows in its dropdown: `English
/// names#Female` as English - Female, `Female names` as Any - Female, `Mixed male names` as
/// Mixed - Male, and any other table by its name, with a heading after a dash.
pub fn label(table: &str) -> String {
    let table = table.trim();
    let without_names = |name: &str| -> String {
        let lower = name.to_lowercase();
        match lower.strip_suffix(" names") {
            Some(_) if lower.len() > " names".len() => {
                name[..name.len() - " names".len()].to_string()
            }
            _ => name.to_string(),
        }
    };
    let capital = |word: &str| -> String {
        let mut chars = word.chars();
        chars
            .next()
            .map(|first| first.to_uppercase().chain(chars).collect())
            .unwrap_or_default()
    };
    if let Some((note, heading)) = table.split_once('#') {
        return format!("{} - {}", without_names(note.trim()), heading.trim());
    }
    let short = without_names(table);
    if short.len() == table.len() {
        return table.to_string();
    }
    let lower = short.to_lowercase();
    if lower == "female" || lower == "male" {
        format!("Any - {}", capital(&short))
    } else if let Some(rest) = lower.strip_prefix("mixed ") {
        format!("Mixed - {}", capital(rest))
    } else {
        table.to_string()
    }
}

/// A group of built-in tables, in the Roll Tables list and the Tables menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    Names,
    StoryPrompts,
    World,
    Adventure,
}

impl Group {
    pub const ALL: [Group; 4] = [
        Group::Names,
        Group::StoryPrompts,
        Group::World,
        Group::Adventure,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Group::Names => "Names",
            Group::StoryPrompts => "Story Prompts",
            Group::World => "World",
            Group::Adventure => "Adventure",
        }
    }

    /// How the group is named in the campaign's settings file.
    pub fn key(self) -> &'static str {
        match self {
            Group::Names => "names",
            Group::StoryPrompts => "story prompts",
            Group::World => "world",
            Group::Adventure => "adventure",
        }
    }
}

/// A roll table built into the app, used where a campaign has no note of its name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuiltIn {
    pub name: &'static str,
    pub text: &'static str,
    /// Whether it rolls each column on its own, like a name's first and family names.
    pub each_column: bool,
    pub group: Group,
}

macro_rules! built_in {
    ($name:literal, $each_column:literal, $group:ident) => {
        BuiltIn {
            name: $name,
            text: include_str!(concat!("../assets/tables/", $name, ".md")),
            each_column: $each_column,
            group: Group::$group,
        }
    };
}

/// The tables built into the app, in the order the Roll Tables list shows them.
pub const BUILT_IN: [BuiltIn; 45] = [
    built_in!("Names", false, Names),
    built_in!("Female names", false, Names),
    built_in!("Male names", false, Names),
    built_in!("Mixed female names", true, Names),
    built_in!("Mixed male names", true, Names),
    built_in!("English names", true, Names),
    built_in!("Norse names", true, Names),
    built_in!("Celtic names", true, Names),
    built_in!("Latin names", true, Names),
    built_in!("Greek names", true, Names),
    built_in!("Arabic names", true, Names),
    built_in!("Slavic names", true, Names),
    built_in!("Japanese names", true, Names),
    built_in!("Fantasy names", true, Names),
    built_in!("Place names", true, Names),
    built_in!("Inn and shop names", true, Names),
    built_in!("Group names", true, Names),
    built_in!("Wants", false, StoryPrompts),
    built_in!("Trouble", false, StoryPrompts),
    built_in!("Twists", false, StoryPrompts),
    built_in!("What's here", false, StoryPrompts),
    built_in!("Feel of a place", false, StoryPrompts),
    built_in!("Tropes and Verbs", false, StoryPrompts),
    built_in!("Tropes", false, StoryPrompts),
    built_in!("Verbs", false, StoryPrompts),
    built_in!("Action and Theme", true, StoryPrompts),
    built_in!("Campaign names", true, World),
    built_in!("Threats", false, World),
    built_in!("Tone", false, World),
    built_in!("World names", true, World),
    built_in!("Peoples", true, World),
    built_in!("Historic events", true, World),
    built_in!("Aftermath", false, World),
    built_in!("Faiths", true, World),
    built_in!("Deity names", true, World),
    built_in!("Domains", false, World),
    built_in!("Wonders", true, World),
    built_in!("Sources of power", false, World),
    built_in!("Costs and limits", false, World),
    built_in!("Dangerous places", true, Adventure),
    built_in!("Quest names", true, Adventure),
    built_in!("Rumors", false, Adventure),
    built_in!("Plot hooks", false, Adventure),
    built_in!("Opening scenes", false, Adventure),
    built_in!("Secrets", false, Adventure),
];

/// The built-in table called `name`, ignoring case.
pub fn built_in(name: &str) -> Option<BuiltIn> {
    BUILT_IN
        .into_iter()
        .find(|table| table.name.eq_ignore_ascii_case(name.trim()))
}

impl BuiltIn {
    pub fn table(&self) -> Option<Table> {
        self.table_at(None)
    }

    /// The table under `heading`, or the first one when there is no heading.
    pub fn table_at(&self, heading: Option<&str>) -> Option<Table> {
        find(self.text).into_iter().find(|table| {
            heading.is_none_or(|heading| {
                table
                    .heading
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(heading.trim()))
            })
        })
    }
}

/// The built-in table a link names, like `Norse names` or `Norse names#Male`: its name,
/// the table, and whether it rolls each column.
pub fn built_in_link(target: &str) -> Option<(String, Table, bool)> {
    let (note, heading) = match target.split_once('#') {
        Some((note, heading)) => (note, Some(heading)),
        None => (target, None),
    };
    let built_in = built_in(note)?;
    let table = built_in.table_at(heading)?;
    Some((table.name(built_in.name), table, built_in.each_column))
}

/// A table in the Tables list.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    /// The name of the note it is in.
    pub note: String,
    pub table: Table,
    pub each_column: bool,
}

impl Entry {
    pub fn name(&self) -> String {
        self.table.name(&self.note)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Roll the table that starts at this place in the note.
    Roll(PathBuf, usize),
    /// Set whether that table rolls each column on its own.
    EachColumn(PathBuf, usize, bool),
    Open(PathBuf),
    /// Roll the built-in table with this name.
    RollBuiltIn(String),
    /// Copy the built-in table with this name into the campaign, to change it.
    CopyBuiltIn(String),
    /// Open or fold a group of built-in tables.
    ToggleGroup(Group),
    /// Open the table editor on the table that starts at this place in the note.
    Edit(PathBuf, usize),
    /// Make the table that starts at this place in the note an information table, so it
    /// isn't rolled or listed.
    Information(PathBuf, usize),
    /// Copy the built-in table with this name into the campaign, and open the table
    /// editor on the copy.
    EditBuiltIn(String),
    /// A table in the list was right-clicked here.
    Menu(Pick, Point),
    New,
    Close,
    /// Roll the roll button with this name.
    RollButton(String),
    /// Make a new roll button, in the New Roll Button dialog.
    NewButton,
    /// Change the roll button with this name, in the same dialog.
    EditButton(String),
    DeleteButton(String),
}

/// A table in the Roll Tables list.
#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
    /// The table that starts at this place in the note.
    Note(PathBuf, usize),
    /// The built-in table with this name.
    BuiltIn(String),
    /// The roll button with this name.
    Button(String),
}

/// A roll button in the Roll Tables list.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonRow {
    pub name: String,
    /// The tables it rolls, by name.
    pub summary: String,
    /// The tables it rolls that can't be found.
    pub missing: Vec<String>,
}

/// The Tables list in the sidebar.
#[derive(Debug, Default)]
pub struct List {
    pub open: bool,
    entries: Vec<Entry>,
    /// The built-in tables shown, with their dice and groups: those the campaign has no
    /// note for.
    built_in: Vec<(String, String, Group)>,
    /// The groups of built-in tables that are open, by [`Group::key`].
    pub open_groups: BTreeSet<String>,
    /// The campaign's roll buttons, in their own group at the top.
    pub buttons: Vec<ButtonRow>,
}

impl List {
    /// Shows `entries`, sorted by name, then the built-in tables in `built_in`.
    pub fn set(&mut self, mut entries: Vec<Entry>, built_in: Vec<BuiltIn>) {
        entries.sort_by_cached_key(|entry| (entry.name().to_lowercase(), entry.path.clone()));
        self.entries = entries;
        self.built_in = built_in
            .into_iter()
            .filter_map(|table| {
                Some((
                    table.name.to_string(),
                    table.table()?.die?.to_string(),
                    table.group,
                ))
            })
            .collect();
    }

    pub fn view(&self, c: Colors, selected: Option<&Path>) -> Element<'_, Message> {
        let header = row![
            text("ROLL TABLES").size(11).color(c.text_muted),
            space::horizontal(),
            hint(
                c,
                button(text("+").size(13))
                    .padding([0, 6])
                    .style(style::flat(c, false))
                    .on_press(Message::New),
                "New roll table",
            ),
            button(text("✕").size(11))
                .padding([2, 6])
                .style(style::flat(c, false))
                .on_press(Message::Close),
        ]
        .spacing(2)
        .align_y(Center)
        .padding([2, 8]);
        let mut rows: Vec<Element<'_, Message>> = vec![header.into()];
        rows.extend(self.buttons_view(c));
        rows.push(
            container(text("TABLES").size(11).color(c.text_muted))
                .padding(iced::padding::top(12).left(8).right(8).bottom(2))
                .into(),
        );
        if self.entries.is_empty() {
            rows.push(
                container(
                    text("No roll tables yet. A table whose first column is a die, like d6, shows here. Make one with the + beside Roll Tables.")
                        .size(12)
                        .color(c.text_muted),
                )
                .padding([2, 8])
                .into(),
            );
        }
        for entry in &self.entries {
            let start = entry.table.range.start;
            let roll = hint(
                c,
                button(die_label(
                    entry
                        .table
                        .die
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                ))
                .width(DIE_WIDTH)
                .padding([1, 0])
                .style(style::secondary(c))
                .on_press(Message::Roll(entry.path.clone(), start)),
                "Roll it",
            );
            let name = button(
                text(entry.name())
                    .size(13)
                    .color(c.text)
                    .wrapping(text::Wrapping::None),
            )
            .width(Fill)
            .padding([1, 4])
            .style(style::flat(c, selected == Some(entry.path.as_path())))
            .on_press(Message::Open(entry.path.clone()));
            let mut line = column![
                row![roll, container(name).clip(true).width(Fill)]
                    .spacing(6)
                    .align_y(Center)
            ]
            .spacing(2);
            if entry.table.heading.is_some() && entry.name() != entry.note {
                line = line.push(
                    container(
                        text(format!("in {}", entry.note))
                            .size(11)
                            .color(c.text_muted),
                    )
                    .padding(iced::padding::left(46)),
                );
            }
            // The tables it is rolled with, from the table editor.
            if let Some(summary) = entry.table.links.summary() {
                line = line.push(
                    container(text(summary).size(11).color(c.text_muted))
                        .padding(iced::padding::left(46)),
                );
            }
            if entry.table.has_columns() {
                line = line.push(
                    container(each_column(c, entry.each_column, {
                        let path = entry.path.clone();
                        move |on| Message::EachColumn(path.clone(), start, on)
                    }))
                    .padding(iced::padding::left(46)),
                );
            }
            let pick = Pick::Note(entry.path.clone(), start);
            rows.push(
                crate::context_menu::area(container(line).padding([2, 8]), move |position| {
                    Message::Menu(pick.clone(), position)
                })
                .into(),
            );
        }

        if !self.built_in.is_empty() {
            rows.push(
                container(text("BUILT IN").size(11).color(c.text_muted))
                    .padding(iced::padding::top(12).left(8).right(8).bottom(2))
                    .into(),
            );
        }
        let mut group = None;
        for (name, die, in_group) in &self.built_in {
            let open = self.open_groups.contains(in_group.key());
            if group != Some(*in_group) {
                group = Some(*in_group);
                let count = self
                    .built_in
                    .iter()
                    .filter(|(_, _, other)| other == in_group)
                    .count();
                rows.push(
                    button(
                        row![
                            text(if open { "▾" } else { "▸" }).size(11).width(12),
                            text(in_group.label()).size(13),
                            text(count.to_string()).size(11).color(c.text_muted),
                        ]
                        .spacing(6)
                        .align_y(Center),
                    )
                    .width(Fill)
                    .padding([2, 8])
                    .style(style::flat(c, false))
                    .on_press(Message::ToggleGroup(*in_group))
                    .into(),
                );
            }
            if !open {
                continue;
            }
            let roll = hint(
                c,
                button(die_label(die.clone()))
                    .width(DIE_WIDTH)
                    .padding([1, 0])
                    .style(style::secondary(c))
                    .on_press(Message::RollBuiltIn(name.clone())),
                "Roll it",
            );
            let copy = hint(
                c,
                button(text("Copy").size(11))
                    .padding([1, 6])
                    .style(style::flat(c, false))
                    .on_press(Message::CopyBuiltIn(name.clone())),
                "Copy it into this campaign, to change it",
            );
            let pick = Pick::BuiltIn(name.clone());
            rows.push(
                crate::context_menu::area(
                    container(
                        row![
                            roll,
                            container(
                                text(name)
                                    .size(13)
                                    .color(c.text)
                                    .wrapping(text::Wrapping::None)
                            )
                            .clip(true)
                            .padding([1, 4])
                            .width(Fill),
                            copy,
                        ]
                        .spacing(6)
                        .align_y(Center),
                    )
                    .padding([2, 8]),
                    move |position| Message::Menu(pick.clone(), position),
                )
                .into(),
            );
        }
        column(rows).spacing(1).into()
    }
}

impl List {
    /// The Roll Buttons group: each button with what it rolls, and a way to make one.
    fn buttons_view(&self, c: Colors) -> Vec<Element<'_, Message>> {
        let new = hint(
            c,
            button(text("+").size(13))
                .padding([0, 6])
                .style(style::flat(c, false))
                .on_press(Message::NewButton),
            "New roll button",
        );
        let mut rows: Vec<Element<'_, Message>> = vec![
            container(
                row![
                    text("ROLL BUTTONS").size(11).color(c.text_muted),
                    space::horizontal(),
                    new
                ]
                .align_y(Center),
            )
            .padding(iced::padding::top(6).left(8).right(8).bottom(2))
            .into(),
        ];
        if self.buttons.is_empty() {
            rows.push(
                container(
                    button(
                        text("Roll two or more tables with one click, like a tavern's name, feel and what's there. Click to make one.")
                            .size(12)
                            .color(c.text_muted),
                    )
                    .padding(0)
                    .style(style::flat(c, false))
                    .on_press(Message::NewButton),
                )
                .padding([2, 8])
                .into(),
            );
        }
        for found in &self.buttons {
            let roll = hint(
                c,
                button(die_label("Roll".into()))
                    .width(DIE_WIDTH)
                    .padding([1, 0])
                    .style(style::secondary(c))
                    .on_press(Message::RollButton(found.name.clone())),
                "Roll every table it has",
            );
            let name = button(
                text(&found.name)
                    .size(13)
                    .color(c.text)
                    .wrapping(text::Wrapping::None),
            )
            .width(Fill)
            .padding([1, 4])
            .style(style::flat(c, false))
            .on_press(Message::EditButton(found.name.clone()));
            let mut line = column![
                row![roll, container(name).clip(true).width(Fill)]
                    .spacing(6)
                    .align_y(Center),
                container(text(&found.summary).size(11).color(c.text_muted))
                    .padding(iced::padding::left(46)),
            ]
            .spacing(2);
            if !found.missing.is_empty() {
                line = line.push(
                    container(
                        text(format!(
                            "Can't find {}. Click the name to change it.",
                            found.missing.join(", ")
                        ))
                        .size(11)
                        .color(c.danger),
                    )
                    .padding(iced::padding::left(46)),
                );
            }
            let pick = Pick::Button(found.name.clone());
            rows.push(
                crate::context_menu::area(container(line).padding([2, 8]), move |position| {
                    Message::Menu(pick.clone(), position)
                })
                .into(),
            );
        }
        rows
    }
}

/// The toggle for rolling each column of a table on its own instead of one whole row.
pub fn each_column<'a, M: Clone + 'a>(
    c: Colors,
    on: bool,
    on_toggle: impl Fn(bool) -> M + 'a,
) -> Element<'a, M> {
    tooltip(
        checkbox(on)
            .label("Roll each column")
            .on_toggle(on_toggle)
            .size(12)
            .text_size(11)
            .style(style::check(c)),
        container(
            text("Roll the die again for every column, to mix results. Off, one roll gives a whole row.")
                .size(12),
        )
        .padding([2, 6])
        .max_width(260)
        .style(style::dropdown(c)),
        tooltip::Position::Bottom,
    )
    .into()
}

fn hint<'a>(
    c: Colors,
    content: impl Into<Element<'a, Message>>,
    label: &'a str,
) -> Element<'a, Message> {
    tooltip(
        content,
        container(text(label).size(12))
            .padding([2, 6])
            .style(style::dropdown(c)),
        tooltip::Position::Bottom,
    )
    .into()
}

/// The dice a new table can use, in the New Roll Table dialog.
pub const DICE: [&str; 8] = ["d4", "d6", "d8", "d10", "d12", "d20", "d100", "2d6"];

/// What the New Roll Table dialog asks for.
#[derive(Debug, Clone, PartialEq)]
pub struct Blank {
    pub die: String,
    pub columns: Vec<String>,
    /// How many rows, or `None` for one per number the die rolls.
    pub rows: Option<i64>,
    /// Results pasted in, one a line.
    pub pasted: String,
}

/// The smallest of [`DICE`] (other than 2d6) with at least `count` results, or d100.
pub fn die_for(count: usize) -> &'static str {
    let count = count as i64;
    ["d4", "d6", "d8", "d10", "d12", "d20", "d100"]
        .into_iter()
        .find(|die| die_bounds(die).1 >= count)
        .unwrap_or("d100")
}

fn die_bounds(die: &str) -> (i64, i64) {
    Expression::parse(die).map_or((1, 6), |die| die.bounds())
}

/// The pasted lines with anything in them, each as its numbers (if it starts with a
/// number or range, like `1.` or `3-4`) and its result.
pub fn pasted_lines(pasted: &str) -> Vec<(Option<String>, String)> {
    pasted
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let line = line.trim_start_matches('|').trim_end_matches('|').trim();
            // A number or range at the start, like `1.`, `3-4`, `01–05`, `6:` or `| 2 |`.
            let end = line
                .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '–' | '—')))
                .unwrap_or(line.len());
            let numbers = line[..end].trim_end_matches(['-', '–', '—']);
            let rest = line[end..].trim_start_matches(['.', ')', ':', '|']).trim();
            if !numbers.is_empty()
                && numbers.starts_with(|c: char| c.is_ascii_digit())
                && !rest.is_empty()
                && (end == line.len() || !line[end..].starts_with(|c: char| c.is_alphanumeric()))
            {
                (Some(numbers.replace(['–', '—'], "-")), rest.to_string())
            } else {
                (None, line.to_string())
            }
        })
        .collect()
}

/// The die a list pasted in fits: one per line, or up to the highest number written.
pub fn die_for_pasted(pasted: &str) -> Option<&'static str> {
    let lines = pasted_lines(pasted);
    if lines.is_empty() {
        return None;
    }
    let written = lines
        .iter()
        .filter_map(|(numbers, _)| numbers.as_deref())
        .filter_map(|numbers| numbers.rsplit('-').next()?.parse::<usize>().ok())
        .max();
    Some(die_for(written.unwrap_or(0).max(lines.len())))
}

/// The Markdown of a new table: a heading-free table with the die's header and one row
/// per number (or `blank.rows` rows splitting the numbers evenly), filled with the lines
/// pasted in, if any.
pub fn markdown(blank: &Blank) -> String {
    let columns: Vec<String> = {
        let named: Vec<String> = blank
            .columns
            .iter()
            .map(|column| column.trim().replace('|', "/"))
            .filter(|column| !column.is_empty())
            .collect();
        if named.is_empty() {
            vec!["Result".into()]
        } else {
            named
        }
    };
    let (low, high) = die_bounds(&blank.die);
    let lines = pasted_lines(&blank.pasted);
    let width = high.to_string().len();
    let number = |value: i64| {
        if high == 100 {
            format!("{value:02}").replace("100", "00")
        } else {
            value.to_string()
        }
    };
    let span = |a: i64, b: i64| {
        if a == b {
            number(a)
        } else {
            format!("{}-{}", number(a), number(b))
        }
    };

    let mut numbers: Vec<String> = Vec::new();
    let mut results: Vec<String> = Vec::new();
    if !lines.is_empty() && lines.iter().all(|(numbers, _)| numbers.is_some()) {
        // The list came with its own numbers.
        for (written, result) in &lines {
            numbers.push(written.clone().unwrap_or_default());
            results.push(result.clone());
        }
    } else {
        let count = blank
            .rows
            .unwrap_or(if lines.is_empty() {
                high - low + 1
            } else {
                lines.len() as i64
            })
            .clamp(1, high - low + 1);
        let total = high - low + 1;
        for index in 0..count {
            let start = low + total * index / count;
            let end = low + total * (index + 1) / count - 1;
            numbers.push(span(start, end));
            results.push(
                lines
                    .get(index as usize)
                    .map(|(_, result)| result.clone())
                    .unwrap_or_default(),
            );
        }
    }

    let die = blank.die.clone();
    let first = die.len().max(width).max(3);
    let mut out = String::new();
    let line = |cells: Vec<String>| format!("| {} |\n", cells.join(" | "));
    let mut header = vec![format!("{die:<first$}")];
    header.extend(columns.iter().cloned());
    out.push_str(&line(header));
    let mut rule = vec!["-".repeat(first)];
    rule.extend(
        columns
            .iter()
            .map(|column| "-".repeat(column.chars().count().max(3))),
    );
    out.push_str(&line(rule));
    for (numbers, result) in numbers.into_iter().zip(results) {
        let mut cells = vec![format!("{numbers:<first$}")];
        for index in 0..columns.len() {
            cells.push(if index == 0 {
                result.replace('|', "\\|")
            } else {
                String::new()
            });
        }
        out.push_str(&line(cells));
    }
    out
}

/// Every die button in the list is this wide, so the names line up.
const DIE_WIDTH: f32 = 40.0;

fn die_label<'a>(die: String) -> iced::widget::Text<'a> {
    text(die).size(11).width(Fill).align_x(Center)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_tables_are_grouped_in_order() {
        let groups: Vec<Group> = BUILT_IN.iter().map(|table| table.group).collect();
        // Each group's tables are together, and the groups come in the menu's order.
        let mut sorted = groups.clone();
        sorted.sort();
        assert_eq!(groups, sorted);
        let count = |group| groups.iter().filter(|other| **other == group).count();
        assert_eq!(count(Group::Names), 17);
        assert_eq!(count(Group::StoryPrompts), 9);
        assert_eq!(count(Group::World), 13);
        assert_eq!(count(Group::Adventure), 6);
    }

    #[test]
    fn open_groups_are_saved_only_when_some_are_open() {
        let mut settings = Settings::default();
        assert!(
            !serde_yaml_ng::to_string(&settings)
                .unwrap()
                .contains("open_groups")
        );
        settings.open_groups.insert(Group::World.key().to_string());
        let yaml = serde_yaml_ng::to_string(&settings).unwrap();
        assert!(yaml.contains("open_groups"), "{yaml}");
        let back: Settings = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(back, settings);
    }

    const WEATHER: &str = "---\nname: Weather\n---\n\n## Weather\n\n| d6  | Weather |\n| --- | ------- |\n| 1   | Clear |\n| 2-3 | Drizzle |\n| 4–5 | 1d4 days of rain |\n| 6   | [[Storms]] |\n";

    fn no_tables(_: &str) -> Option<(String, Table, bool)> {
        None
    }

    #[test]
    fn finds_tables_with_a_die_column() {
        let tables = find(WEATHER);
        assert_eq!(tables.len(), 1);
        let table = &tables[0];
        assert_eq!(table.heading.as_deref(), Some("Weather"));
        assert_eq!(table.die.as_ref().unwrap().to_string(), "1d6");
        assert_eq!(table.columns, ["Weather"]);
        let covered: Vec<_> = table.rows.iter().map(|row| (row.low, row.high)).collect();
        assert_eq!(covered, [(1, 1), (2, 3), (4, 5), (6, 6)]);
        assert_eq!(table.rows[3].cells, ["[[Storms]]"]);
        assert!(WEATHER[table.range.clone()].starts_with("| d6"));

        // A table whose first column isn't a die isn't a roll table.
        assert!(find("| Name | Age |\n| --- | --- |\n| Bo | 3 |\n").is_empty());
        assert!(find("| +2 | x |\n| --- | --- |\n| 1 | a |\n").is_empty());
    }

    #[test]
    fn a_d1_table_is_a_roll_table() {
        let table = find("| d1 | A | B |\n| --- | --- | --- |\n| 1 | x | y |\n").remove(0);
        assert_eq!(table.die.as_ref().unwrap().bounds(), (1, 1));
        assert_eq!(table.rows[0].cells, ["x", "y"]);
    }

    #[test]
    fn joins_results_with_spaces_nothing_or_commas() {
        let row = |pairs: &[(Join, &str)]| joined(pairs.iter().copied());
        assert_eq!(
            row(&[(Join::Space, "Bjorn"), (Join::Space, "Ashby")]),
            "Bjorn Ashby"
        );
        assert_eq!(
            row(&[(Join::Space, "Bjorn"), (Join::NoSpace, "sson")]),
            "Bjornsson"
        );
        assert_eq!(
            row(&[
                (Join::Space, "rope"),
                (Join::Comma, "lantern"),
                (Join::Comma, "chalk")
            ]),
            "rope, lantern, chalk"
        );
        // The hyphen in a result still joins it, whatever its column says, even twice.
        assert_eq!(
            row(&[(Join::Space, "Ash"), (Join::Space, "-ford")]),
            "Ashford"
        );
        assert_eq!(
            row(&[(Join::Space, "Ash"), (Join::NoSpace, "--ford")]),
            "Ashford"
        );
        assert_eq!(
            row(&[(Join::Space, "rope"), (Join::Space, ", chalk")]),
            "rope, chalk"
        );
        // Empty results are left out.
        assert_eq!(
            row(&[(Join::Space, "a"), (Join::Comma, ""), (Join::Comma, "b")]),
            "a, b"
        );
    }

    #[test]
    fn each_column_rolls_join_as_the_settings_say() {
        let mut table =
            find("| d1 | Item | Item |\n| --- | --- | --- |\n| 1 | rope | chalk |\n").remove(0);
        let mut settings = Settings::default();
        let key = "gear.md#Packs";
        settings.set_joins(key, &table.columns, &[Join::Space, Join::Comma]);
        table.joins = settings.joins(key, &table.columns);
        let mut rng = fastrand::Rng::with_seed(1);
        assert_eq!(
            roll("Packs", &table, true, &mut rng, &mut no_tables).text(),
            "rope, chalk"
        );

        // Only joins other than a space are kept, and none for a table with none.
        settings.set_joins(key, &table.columns, &[Join::Space, Join::Space]);
        assert!(settings.joins.is_empty());
        let yaml = serde_yaml_ng::to_string(&settings).unwrap();
        assert!(!yaml.contains("joins"), "{yaml}");
        settings.set_joins(
            key,
            &["A".into(), "B".into()],
            &[Join::Space, Join::NoSpace],
        );
        let yaml = serde_yaml_ng::to_string(&settings).unwrap();
        assert!(yaml.contains("no_space"), "{yaml}");
        assert_eq!(
            serde_yaml_ng::from_str::<Settings>(&yaml).unwrap(),
            settings
        );
    }

    const GIVEN: &str = "| d2 | Given |\n| --- | --- |\n| 1 | Bjorn |\n| 2 | Leif |\n";
    const SURNAME: &str = "| d1 | Ending |\n| --- | --- |\n| 1 | sson |\n";
    const FATHER: &str = "| d1 | Father |\n| --- | --- |\n| 1 | Ulf |\n";

    /// Finds the tables above by name, each with `linked` set from `file`, as the app does.
    fn norse(file: &str) -> impl FnMut(&str) -> Option<(String, Table, bool)> + '_ {
        move |target: &str| {
            let text = match target {
                "Given" => GIVEN,
                "Surname" => SURNAME,
                "Father" => FATHER,
                _ => return None,
            };
            let mut table = find(text).remove(0);
            if let Some((_, links, linked)) = read_linked(file)
                .into_iter()
                .find(|(own, _, _)| own == target)
            {
                table.links = links;
                table.linked = Some(Box::new(linked));
            }
            Some((target.to_string(), table, false))
        }
    }

    #[test]
    fn linked_tables_are_kept_as_d1_tables() {
        let surname = Links {
            before: vec![("Father".into(), Join::Space)],
            join: Join::NoSpace,
            after: Vec::new(),
        };
        let given = Links {
            before: Vec::new(),
            join: Join::Space,
            after: vec![
                ("Father".into(), Join::Space),
                ("Surname".into(), Join::NoSpace),
            ],
        };
        let file = write_linked(&[
            ("Surname".into(), "Surname".into(), surname.clone()),
            ("Given".into(), "Given".into(), given.clone()),
            ("Nothing".into(), "Nothing".into(), Links::default()),
        ]);
        assert!(file.contains("| d1 "), "{file}");
        assert!(file.contains("-[[Surname]]"), "{file}");
        assert!(!file.contains("Nothing"), "{file}");
        let read = read_linked(&file);
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].0, "Surname");
        assert_eq!(read[0].1, surname);
        assert_eq!(read[1].0, "Given");
        assert_eq!(read[1].1, given);
        assert_eq!(read[1].2.columns, [THIS, ALSO, ALSO]);
        assert_eq!(
            given.summary().as_deref(),
            Some("Also rolls Father, Surname")
        );
        assert_eq!(surname.summary().as_deref(), Some("After Father"));
    }

    #[test]
    fn rolling_a_table_rolls_its_linked_tables() {
        let file = write_linked(&[
            (
                "Given".into(),
                "Given".into(),
                Links {
                    after: vec![("Surname".into(), Join::Space)],
                    ..Links::default()
                },
            ),
            (
                "Surname".into(),
                "Surname".into(),
                Links {
                    before: vec![("Father".into(), Join::Space)],
                    join: Join::NoSpace,
                    ..Links::default()
                },
            ),
        ]);
        let mut lookup = norse(&file);
        let mut rng = fastrand::Rng::with_seed(3);
        let (name, given, _) = lookup("Given").unwrap();
        let rolled = roll(&name, &given, false, &mut rng, &mut lookup);
        let text = rolled.text();
        assert!(text == "Bjorn Ulfsson" || text == "Leif Ulfsson", "{text}");
        // The history shows what each table rolled, not the d1.
        assert!(rolled.values.is_empty());
        assert_eq!(rolled.inner.len(), 2);
        assert!(rolled.line().starts_with("Given: "), "{}", rolled.line());

        // Rolled after: a surname gets its father's name first, on its own too.
        let (name, surname, _) = lookup("Surname").unwrap();
        let rolled = roll(&name, &surname, false, &mut rng, &mut lookup);
        assert_eq!(rolled.text(), "Ulfsson");

        // Two tables that roll each other stop.
        let looped = write_linked(&[
            (
                "Given".into(),
                "Given".into(),
                Links {
                    after: vec![("Father".into(), Join::Space)],
                    ..Links::default()
                },
            ),
            (
                "Father".into(),
                "Father".into(),
                Links {
                    after: vec![("Given".into(), Join::Space)],
                    ..Links::default()
                },
            ),
        ]);
        let mut looping = norse(&looped);
        let (name, given, _) = looping("Given").unwrap();
        let text = roll(&name, &given, false, &mut rng, &mut looping).text();
        assert_eq!(text.split(' ').count(), 3, "{text}");

        // A link from another table rolls the linked tables too.
        let names = find("| d1 | Name |\n| --- | --- |\n| 1 | [[Given]] the Bold |\n").remove(0);
        let rolled = roll("Names", &names, false, &mut rng, &mut lookup);
        assert!(
            rolled.text().ends_with("Ulfsson the Bold"),
            "{}",
            rolled.text()
        );
    }

    #[test]
    fn finds_information_tables_too() {
        let text = "# Prices\n\n| Item | Cost |\n| --- | --- |\n| Rope | 1 sp |\n\n## Weather\n\n| d2 | Sky |\n| --- | --- |\n| 1 | Sun |\n| 2 | Rain |\n";
        let all = find_all(text);
        assert_eq!(all.len(), 2);
        assert!(!all[0].has_die());
        assert_eq!(all[0].heading.as_deref(), Some("Prices"));
        assert_eq!(all[0].columns, ["Item", "Cost"]);
        assert_eq!(all[0].written[1], ["Rope", "1 sp"]);
        assert!(all[1].has_die());
        // Only the one with a die is found to roll.
        let rolled = find(text);
        assert_eq!(rolled.len(), 1);
        assert_eq!(rolled[0].heading.as_deref(), Some("Weather"));
    }

    #[test]
    fn information_tables_have_no_problems() {
        let text = "## Gaps\n\n| d6 | Sky |\n| --- | --- |\n| 1 | Sun |\n";
        assert_eq!(problems(text, "x", &|_| false).len(), 1);
        assert!(problems(text, "x", &|heading| heading == Some("Gaps")).is_empty());
    }

    #[test]
    fn reads_row_numbers() {
        assert_eq!(numbers("4", 6), Some((4, 4)));
        assert_eq!(numbers("2-3", 6), Some((2, 3)));
        assert_eq!(numbers("01–05", 100), Some((1, 5)));
        assert_eq!(numbers("96-00", 100), Some((96, 100)));
        assert_eq!(numbers("00", 100), Some((100, 100)));
        assert_eq!(numbers("5+", 6), Some((5, 6)));
        assert_eq!(numbers("3.", 6), Some((3, 3)));
        assert_eq!(numbers("one", 6), None);
    }

    #[test]
    fn rolls_rows_dice_and_other_tables() {
        let table = find(WEATHER).remove(0);
        let storms = find("| d4 | Storm |\n| -- | -- |\n| 1-4 | Hail |\n").remove(0);
        let mut rng = fastrand::Rng::with_seed(7);
        let mut seen = BTreeSet::new();
        for _ in 0..200 {
            let mut lookup = |target: &str| {
                (target == "Storms").then(|| ("Storms".to_string(), storms.clone(), false))
            };
            let rolled = roll("Weather", &table, false, &mut rng, &mut lookup);
            let value = rolled.values[0];
            let text = rolled.text();
            match value {
                1 => assert_eq!(text, "Clear"),
                2 | 3 => assert_eq!(text, "Drizzle"),
                4 | 5 => {
                    let days: i64 = text.split(' ').next().unwrap().parse().unwrap();
                    assert!((1..=4).contains(&days), "{text}");
                    assert!(text.ends_with(" days of rain"));
                }
                6 => {
                    assert_eq!(text, "Hail");
                    assert_eq!(rolled.inner.len(), 1);
                    assert_eq!(
                        rolled.inner[0].line(),
                        format!("Storms ({}): Hail", rolled.inner[0].values[0])
                    );
                }
                _ => panic!("rolled {value}"),
            }
            seen.insert(value);
        }
        assert_eq!(seen.len(), 6);

        // A link to a note that isn't a table is shown as its name.
        let mut rng = fastrand::Rng::with_seed(1);
        let only = find("| d1 | x |\n| - | - |\n| 1 | Ask [[Mira\\|the smith]] |\n").remove(0);
        assert_eq!(
            roll("x", &only, false, &mut rng, &mut no_tables).text(),
            "Ask the smith"
        );
    }

    #[test]
    fn links_that_dont_roll_stay_links() {
        let mut table = find("| d1 | x |\n| - | - |\n| 1 | Beware [[Storms]] |\n").remove(0);
        let storms = find("| d1 | Storm |\n| -- | -- |\n| 1 | Hail |\n").remove(0);
        let mut lookup = |target: &str| {
            (target == "Storms").then(|| ("Storms".to_string(), storms.clone(), false))
        };
        let mut rng = fastrand::Rng::with_seed(1);
        assert_eq!(
            roll("x", &table, false, &mut rng, &mut lookup).text(),
            "Beware Hail"
        );
        table.not_rolled.insert(link_key(" storms "));
        assert_eq!(
            roll("x", &table, false, &mut rng, &mut lookup).text(),
            "Beware Storms"
        );
        assert_eq!(link_key("Weather # Winter|the cold"), "weather#winter");
    }

    #[test]
    fn a_table_that_rolls_on_itself_stops() {
        let table = find("| d1 | Again |\n| - | - |\n| 1 | more [[Loop]] |\n").remove(0);
        let mut rng = fastrand::Rng::with_seed(1);
        let copy = table.clone();
        let mut lookup = move |_: &str| Some(("Loop".to_string(), copy.clone(), false));
        let rolled = roll("Loop", &table, false, &mut rng, &mut lookup);
        assert_eq!(rolled.text().matches("more").count(), MAX_DEPTH + 1);
    }

    #[test]
    fn rolls_a_whole_row_or_each_column() {
        let text =
            "| d2 | First | Last |\n| -- | -- | -- |\n| 1 | Ann | Ash |\n| 2 | Bo | Birch |\n";
        let table = find(text).remove(0);
        assert!(table.has_columns());
        let mut rng = fastrand::Rng::with_seed(3);
        let mut mixed = false;
        for _ in 0..100 {
            let row = roll("Names", &table, false, &mut rng, &mut no_tables);
            assert_eq!(row.values.len(), 1);
            assert!(
                ["First: Ann; Last: Ash", "First: Bo; Last: Birch"].contains(&row.text().as_str())
            );
            assert!(["Ann", "Bo"].contains(&row.note_name().as_str()));

            let each = roll("Names", &table, true, &mut rng, &mut no_tables);
            assert_eq!(each.values.len(), 2);
            mixed |= ["Ann Birch", "Bo Ash"].contains(&each.text().as_str());
            assert_eq!(each.note_name(), each.text());
        }
        assert!(mixed);
    }

    #[test]
    fn points_out_rows_that_cant_be_rolled() {
        let text = "## Loot\n\n| d8 | Loot |\n| -- | -- |\n| 1 | Coins |\n| 2-3 | Gem |\n| six | Sword |\n| 7 | Map |\n";
        assert_eq!(
            problems(text, "Treasure", &|_| false),
            [
                "A row of the Loot table starts with \"six\", not a number or a range like 2-3, so it can't be rolled",
                "The Loot table has no row for 4-6 or 8, which its 1d8 can roll",
            ]
        );
        assert!(problems(WEATHER, "Weather", &|_| false).is_empty());
    }

    #[test]
    fn makes_new_tables() {
        let blank = Blank {
            die: "d6".into(),
            columns: vec!["Result".into()],
            rows: None,
            pasted: String::new(),
        };
        let made = markdown(&blank);
        assert!(
            made.starts_with("| d6  | Result |\n| --- | ------ |\n| 1   |  |\n"),
            "{made}"
        );
        let table = find(&made).remove(0);
        assert_eq!(table.rows.len(), 6);
        assert!(problems(&made, "x", &|_| false).is_empty());

        // Fewer rows split the numbers evenly.
        let made = markdown(&Blank {
            die: "d100".into(),
            rows: Some(20),
            ..blank.clone()
        });
        let table = find(&made).remove(0);
        assert_eq!(table.rows.len(), 20);
        assert_eq!((table.rows[0].low, table.rows[0].high), (1, 5));
        assert_eq!((table.rows[19].low, table.rows[19].high), (96, 100));
        assert!(made.contains("| 96-00"));
        assert!(problems(&made, "x", &|_| false).is_empty());

        // Pasted lines fill the rows, keeping their own numbers if they have them.
        let pasted = "1. A cold wind\n2-3: Fog\n4) Rain\n";
        assert_eq!(die_for_pasted(pasted), Some("d4"));
        let made = markdown(&Blank {
            die: "d4".into(),
            pasted: pasted.into(),
            columns: vec!["Weather".into(), "Note".into()],
            ..blank.clone()
        });
        let table = find(&made).remove(0);
        assert_eq!(table.columns, ["Weather", "Note"]);
        let rows: Vec<_> = table
            .rows
            .iter()
            .map(|row| (row.low, row.high, row.cells[0].as_str()))
            .collect();
        assert_eq!(rows, [(1, 1, "A cold wind"), (2, 3, "Fog"), (4, 4, "Rain")]);

        // Without numbers, each line is a row, numbered for it.
        let pasted = "Wolves\nBandits\nA lost child\nNothing\nA storm\n";
        assert_eq!(die_for_pasted(pasted), Some("d6"));
        let made = markdown(&Blank {
            die: "d6".into(),
            pasted: pasted.into(),
            ..blank
        });
        let table = find(&made).remove(0);
        assert_eq!(table.rows.len(), 5);
        assert_eq!((table.rows[4].low, table.rows[4].high), (5, 6));
        assert_eq!(table.rows[2].cells, ["A lost child"]);
        // Words that start with a number aren't numbered rows.
        assert_eq!(
            pasted_lines("3rd bridge")[0],
            (None, "3rd bridge".to_string())
        );
    }

    #[test]
    fn built_in_tables_read_and_roll() {
        let mut rng = fastrand::Rng::with_seed(5);
        for table in BUILT_IN {
            assert!(table.table().is_some(), "{} isn't a table", table.name);
            assert!(
                problems(table.text, table.name, &|_| false).is_empty(),
                "{}",
                table.name
            );
            // Every table it links to is built in too.
            for link in links::targets(table.text) {
                assert!(
                    built_in_link(&link).is_some(),
                    "{} links to {link}",
                    table.name
                );
            }
            for read in find(table.text) {
                let rolled = roll(
                    table.name,
                    &read,
                    table.each_column,
                    &mut rng,
                    &mut built_in_link,
                );
                assert!(
                    !rolled.text().trim().is_empty() && !rolled.text().contains("[["),
                    "{}",
                    table.name
                );
            }
        }
        assert!(built_in("names").is_some());
        // Names are built from parts: a Norse man is his father's son.
        let (name, male, each_column) = built_in_link("Male names").unwrap();
        for _ in 0..20 {
            let rolled = roll(&name, &male, each_column, &mut rng, &mut built_in_link);
            assert!(rolled.text().contains(' '), "{}", rolled.text());
        }
        let (name, norse, each_column) = built_in_link("Norse names#Male").unwrap();
        let rolled = roll(&name, &norse, each_column, &mut rng, &mut built_in_link);
        assert!(rolled.text().ends_with("sson"), "{}", rolled.text());
    }

    #[test]
    fn every_link_in_a_built_in_table_finds_its_table() {
        // A renamed heading would leave a link that rolls nothing, so a roll shows
        // "[[Norse names#Female]]" in place of a name.
        for table in BUILT_IN {
            for link in links::targets(table.text) {
                assert!(
                    built_in_link(&link).is_some(),
                    "{} links to {link}, which no built-in table has",
                    table.name
                );
            }
        }
    }

    fn rolls(target: &str, times: usize) -> Vec<String> {
        let mut rng = fastrand::Rng::with_seed(7);
        let (name, table, each_column) = built_in_link(target).unwrap();
        (0..times)
            .map(|_| roll(&name, &table, each_column, &mut rng, &mut built_in_link).text())
            .collect()
    }

    #[test]
    fn a_culture_gives_full_names() {
        for culture in [
            "English", "Norse", "Celtic", "Latin", "Greek", "Arabic", "Slavic", "Japanese",
        ] {
            for target in [
                format!("{culture} names"),
                format!("{culture} names#Female"),
                format!("{culture} names#Male"),
            ] {
                for name in rolls(&target, 20) {
                    assert!(
                        name.split_whitespace().count() >= 2 && !name.contains("[["),
                        "{target} gave {name}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_norse_name_picked_has_the_matching_ending() {
        for name in rolls("Norse names#Female", 20) {
            assert!(name.ends_with("sdottir"), "{name}");
        }
        for name in rolls("Norse names#Male", 20) {
            assert!(name.ends_with("sson"), "{name}");
        }
        // Norse names rolls a daughter or a son, each with the right ending.
        let (name, table, each_column) = built_in_link("Norse names").unwrap();
        let mut rng = fastrand::Rng::with_seed(3);
        for _ in 0..40 {
            let rolled = roll(&name, &table, each_column, &mut rng, &mut built_in_link);
            let female = rolled.inner[0].name == "Female";
            let text = rolled.text();
            assert!(
                text.ends_with(if female { "sdottir" } else { "sson" }),
                "{text}"
            );
        }
    }

    #[test]
    fn random_norse_family_names_can_end_either_way() {
        for target in ["Norse names#Family Names", "Norse names#Any"] {
            let names = rolls(target, 60);
            assert!(names.iter().any(|name| name.ends_with("sson")), "{target}");
            assert!(
                names.iter().any(|name| name.ends_with("sdottir")),
                "{target}"
            );
            assert!(names.iter().all(|name| !name.contains('-')), "{target}");
        }
        // Names and the Mixed tables use them.
        let names = built_in("Names").unwrap().text;
        assert!(names.contains("[[Norse names#Any]]"));
        for mixed in ["Mixed female names", "Mixed male names"] {
            assert!(
                built_in(mixed)
                    .unwrap()
                    .text
                    .contains("| [[Norse names#Family Names]] |"),
                "{mixed}"
            );
        }
    }

    #[test]
    fn mixed_names_take_a_family_name_not_a_second_first_name() {
        // The Family column links to family names, not given names.
        for mixed in ["Mixed female names", "Mixed male names"] {
            let table = built_in(mixed).unwrap().table().unwrap();
            for row in &table.rows {
                assert!(!row.cells[1].contains("Given"), "{mixed}: {}", row.cells[1]);
            }
        }
    }

    #[test]
    fn tropes_and_verbs_gives_one_of_each() {
        for text in rolls("Tropes and Verbs", 10) {
            assert!(
                text.starts_with("Trope: ") && text.contains("; Verb: "),
                "{text}"
            );
        }
        assert_eq!(built_in("Tropes").unwrap().table().unwrap().rows.len(), 50);
        assert_eq!(built_in("Verbs").unwrap().table().unwrap().rows.len(), 100);
    }

    #[test]
    fn a_hyphen_joins_the_result_before_it() {
        let text = "| d1 | Start | End |\n| - | - | - |\n| 1 | Ash | -ford |\n";
        let table = find(text).remove(0);
        let mut rng = fastrand::Rng::with_seed(1);
        assert_eq!(
            roll("Place", &table, true, &mut rng, &mut no_tables).text(),
            "Ashford"
        );
    }

    #[test]
    fn keys_tables_by_note_and_heading() {
        let root = Path::new("/c");
        assert_eq!(
            key(root, &root.join("tables/Names.md"), Some("Elves")),
            "tables/Names.md#Elves"
        );
        let mut settings = Settings::default();
        settings.set("a", true);
        assert!(settings.each_column.contains("a"));
        settings.set("a", false);
        assert!(settings.each_column.is_empty());

        // Links that don't roll are only written for tables that have some.
        settings.set_not_rolled("a", ["storms".to_string()].into());
        let yaml = serde_yaml_ng::to_string(&settings).unwrap();
        assert!(yaml.contains("not_rolled"), "{yaml}");
        let read: Settings = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(read, settings);
        settings.set_not_rolled("a", BTreeSet::new());
        let yaml = serde_yaml_ng::to_string(&settings).unwrap();
        assert!(!yaml.contains("not_rolled"), "{yaml}");
    }

    #[test]
    fn a_link_in_a_column_rolls_that_column_of_the_table() {
        let names = find(
            "| d2 | First | Family | Ending |\n| --- | --- | --- | --- |\n| 1 | Astrid | Arne | -sdottir |\n| 2 | Astrid | Arne | -sdottir |\n",
        )
        .remove(0);
        let other = find("| d2 | Given |\n| --- | --- |\n| 1 | Isaac |\n| 2 | Isaac |\n").remove(0);
        let mixed = find(
            "| d2 | First | Family |\n| --- | --- | --- |\n| 1 | [[Norse]] | [[Norse]] |\n| 2 | [[Other]] | [[Norse]] |\n",
        )
        .remove(0);
        let mut lookup = |name: &str| match name {
            "Norse" => Some(("Norse".to_string(), names.clone(), true)),
            "Other" => Some(("Other".to_string(), other.clone(), false)),
            _ => None,
        };
        let mut rng = fastrand::Rng::with_seed(1);
        for _ in 0..20 {
            let rolled = roll("Mixed", &mixed, true, &mut rng, &mut lookup);
            // The family name takes the Family column and the ending that joins it; a
            // table with no First column gives its whole row.
            let first = &rolled.results[0].1;
            assert!(first == "Astrid" || first == "Isaac", "{first}");
            assert_eq!(rolled.results[1].1, "Arnesdottir");
        }
    }

    #[test]
    fn mixed_names_can_come_from_two_cultures() {
        let table = built_in("Mixed male names").unwrap();
        let mut rng = fastrand::Rng::with_seed(3);
        let mut lookup = built_in_link;
        let rolled = roll(
            table.name,
            &table.table().unwrap(),
            true,
            &mut rng,
            &mut lookup,
        );
        let text = rolled.text();
        assert_eq!(rolled.results.len(), 2, "{text}");
        assert!(!text.contains("[[") && !text.contains(':'), "{text}");
    }

    #[test]
    fn labels_tables_for_a_dropdown() {
        assert_eq!(label("English names#Female"), "English - Female");
        assert_eq!(label("Norse names#Male"), "Norse - Male");
        assert_eq!(label("Female names"), "Any - Female");
        assert_eq!(label("Male names"), "Any - Male");
        assert_eq!(label("Mixed female names"), "Mixed - Female");
        assert_eq!(label("Rumors"), "Rumors");
        assert_eq!(label("Place names"), "Place names");
        assert_eq!(label("Weather#Storms"), "Weather - Storms");
    }

    #[test]
    fn remembers_the_table_picked() {
        let tables = vec!["Female names".to_string(), "Norse names#Male".to_string()];
        let mut settings = Settings::default();
        assert_eq!(settings.picked("npcs/name", &tables), Some(&tables[0]));
        settings
            .picked
            .insert("npcs/name".into(), "norse names#male".into());
        assert_eq!(settings.picked("npcs/name", &tables), Some(&tables[1]));
        // A pick no longer in the list falls back to the first.
        settings.picked.insert("npcs/name".into(), "Gone".into());
        assert_eq!(settings.picked("npcs/name", &tables), Some(&tables[0]));
    }
}
