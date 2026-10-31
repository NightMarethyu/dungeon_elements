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

use std::collections::BTreeSet;
use std::ops::Range;
use std::path::{Path, PathBuf};

use iced::widget::{button, checkbox, column, container, row, space, text, tooltip};
use iced::{Center, Element, Fill};

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

/// A roll table found in a note.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// The heading the table is under, if any.
    pub heading: Option<String>,
    /// The die in its first header, like `d6` or `2d6`.
    pub die: Expression,
    /// The headers of its other columns.
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    /// Rows whose first cell isn't a number or a range, as written.
    pub unread: Vec<String>,
    /// Where the table is in the note's text.
    pub range: Range<usize>,
    /// Every row's cells as written, the header first, for the table editor.
    pub written: Vec<Vec<String>>,
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
}

/// The roll tables in a note's `text`, in order.
pub fn find(text: &str) -> Vec<Table> {
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

/// The roll table in `rows` of cells (the header first), if its first header is a die.
fn read(heading: Option<String>, rows: Vec<Vec<String>>, range: Range<usize>) -> Option<Table> {
    let written = rows.clone();
    let mut rows = rows.into_iter();
    let header = rows.next()?;
    let die = die(header.first()?)?;
    let (_, highest) = die.bounds();
    let mut table = Table {
        heading,
        die,
        columns: header[1..].to_vec(),
        rows: Vec::new(),
        unread: Vec::new(),
        range,
        written,
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
/// can't pick, and numbers it can roll that no row has.
pub fn problems(text: &str, note: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for table in find(text) {
        let name = table.name(note);
        for cell in &table.unread {
            problems.push(format!(
                "A row of the {name} table starts with \"{cell}\", not a number or a range like 2-3, so it can't be rolled"
            ));
        }
        let (low, high) = table.die.bounds();
        if high - low > MAX_CHECKED || table.rows.is_empty() {
            continue;
        }
        let missing = ranges((low..=high).filter(|value| table.row(*value).is_none()));
        if !missing.is_empty() {
            problems.push(format!(
                "The {name} table has no row for {}, which its {} can roll",
                list(&missing),
                table.die
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
    /// The tables its results rolled on, in order.
    pub inner: Vec<Rolled>,
}

impl Rolled {
    /// The result as one line: the result, the columns' results in a row, or each
    /// column's header and result.
    pub fn text(&self) -> String {
        let filled = self.results.iter().filter(|(_, value)| !value.is_empty());
        if self.results.len() == 1 || self.each_column {
            // A result starting with a hyphen joins the one before it: Ash and -ford
            // make Ashford.
            let mut text = String::new();
            for (_, value) in filled {
                match value.strip_prefix('-').filter(|rest| !rest.is_empty()) {
                    Some(rest) => text.push_str(rest),
                    None => {
                        if !text.is_empty() {
                            text.push(' ');
                        }
                        text.push_str(value);
                    }
                }
            }
            text
        } else {
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

    /// For the list of earlier rolls: `Weather (6): Grey drizzle`.
    pub fn line(&self) -> String {
        let values: Vec<String> = self.values.iter().map(i64::to_string).collect();
        format!("{} ({}): {}", self.name, values.join(", "), self.text())
    }
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
    roll_at(name, table, each_column, rng, lookup, 0)
}

fn roll_at(
    name: &str,
    table: &Table,
    each_column: bool,
    rng: &mut fastrand::Rng,
    lookup: &mut Lookup<'_>,
    depth: usize,
) -> Rolled {
    let each_column = each_column && table.has_columns();
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
                let value = table.die.total(rng);
                values.push(value);
                let found = (value, table.row(value));
                picked = Some(found);
                found
            }
        };
        let result = match row {
            Some(row) => {
                let cell = row.cells.get(index).map_or("", String::as_str);
                expand(cell, rng, lookup, depth, &mut inner)
            }
            None => format!("(no row for {value})"),
        };
        results.push((column.clone(), result));
    }
    Rolled {
        name: name.to_string(),
        die: table.die.to_string(),
        values,
        results,
        each_column,
        inner,
    }
}

/// A result's text with its links to tables rolled, its other links shown as their
/// names, and its dice rolled.
fn expand(
    cell: &str,
    rng: &mut fastrand::Rng,
    lookup: &mut Lookup<'_>,
    depth: usize,
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
        let table = (depth < MAX_DEPTH).then(|| lookup(target)).flatten();
        match table {
            Some((name, table, each_column)) => {
                let rolled = roll_at(&name, &table, each_column, rng, lookup, depth + 1);
                out.push_str(&rolled.text());
                inner.push(rolled);
            }
            None => out.push_str(shown),
        }
    }
    out.push_str(&roll_dice(&cell[last..], rng));
    out
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

    /// Sets whether the table `key` rolls each column on its own.
    pub fn set(&mut self, key: &str, each_column: bool) {
        if each_column {
            self.each_column.insert(key.to_string());
        } else {
            self.each_column.remove(key);
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
}

macro_rules! built_in {
    ($name:literal, $each_column:literal) => {
        BuiltIn {
            name: $name,
            text: include_str!(concat!("../assets/tables/", $name, ".md")),
            each_column: $each_column,
        }
    };
}

/// The tables built into the app, in the order the Roll Tables list shows them.
pub const BUILT_IN: [BuiltIn; 40] = [
    built_in!("Names", false),
    built_in!("Female names", false),
    built_in!("Male names", false),
    built_in!("English names", true),
    built_in!("Norse names", true),
    built_in!("Celtic names", true),
    built_in!("Latin names", true),
    built_in!("Greek names", true),
    built_in!("Arabic names", true),
    built_in!("Slavic names", true),
    built_in!("Japanese names", true),
    built_in!("Fantasy names", true),
    built_in!("Place names", true),
    built_in!("Inn and shop names", true),
    built_in!("Group names", true),
    built_in!("Wants", false),
    built_in!("Trouble", false),
    built_in!("Twists", false),
    built_in!("What's here", false),
    built_in!("Feel of a place", false),
    built_in!("Action and Theme", true),
    built_in!("Campaign names", true),
    built_in!("Threats", false),
    built_in!("Tone", false),
    built_in!("World names", true),
    built_in!("Peoples", true),
    built_in!("Historic events", true),
    built_in!("Aftermath", false),
    built_in!("Faiths", true),
    built_in!("Deity names", true),
    built_in!("Domains", false),
    built_in!("Wonders", true),
    built_in!("Sources of power", false),
    built_in!("Costs and limits", false),
    built_in!("Dangerous places", true),
    built_in!("Quest names", true),
    built_in!("Rumors", false),
    built_in!("Plot hooks", false),
    built_in!("Opening scenes", false),
    built_in!("Secrets", false),
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
    New,
    Close,
}

/// The Tables list in the sidebar.
#[derive(Debug, Default)]
pub struct List {
    pub open: bool,
    entries: Vec<Entry>,
    /// The built-in tables shown, with their dice: those the campaign has no note for.
    built_in: Vec<(String, String)>,
}

impl List {
    /// Shows `entries`, sorted by name, then the built-in tables in `built_in`.
    pub fn set(&mut self, mut entries: Vec<Entry>, built_in: Vec<BuiltIn>) {
        entries.sort_by_cached_key(|entry| (entry.name().to_lowercase(), entry.path.clone()));
        self.entries = entries;
        self.built_in = built_in
            .into_iter()
            .filter_map(|table| Some((table.name.to_string(), table.table()?.die.to_string())))
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
        if self.entries.is_empty() {
            rows.push(
                container(
                    text("No roll tables yet. A table whose first column is a die, like d6, shows here. Make one with the + above.")
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
                button(die_label(entry.table.die.to_string()))
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
            if entry.table.has_columns() {
                line = line.push(
                    container(each_column(c, entry.each_column, {
                        let path = entry.path.clone();
                        move |on| Message::EachColumn(path.clone(), start, on)
                    }))
                    .padding(iced::padding::left(46)),
                );
            }
            rows.push(container(line).padding([2, 8]).into());
        }

        if !self.built_in.is_empty() {
            rows.push(
                container(text("BUILT IN").size(11).color(c.text_muted))
                    .padding(iced::padding::top(12).left(8).right(8).bottom(2))
                    .into(),
            );
        }
        for (name, die) in &self.built_in {
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
            rows.push(
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
                .padding([2, 8])
                .into(),
            );
        }
        column(rows).spacing(1).into()
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
fn pasted_lines(pasted: &str) -> Vec<(Option<String>, String)> {
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
        assert_eq!(table.die.to_string(), "1d6");
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
            problems(text, "Treasure"),
            [
                "A row of the Loot table starts with \"six\", not a number or a range like 2-3, so it can't be rolled",
                "The Loot table has no row for 4-6 or 8, which its 1d8 can roll",
            ]
        );
        assert!(problems(WEATHER, "Weather").is_empty());
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
        assert!(problems(&made, "x").is_empty());

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
        assert!(problems(&made, "x").is_empty());

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
                problems(table.text, table.name).is_empty(),
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
    }
}
