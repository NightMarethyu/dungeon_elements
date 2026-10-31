//! The roll table editor: a roll table's die, columns and rows in boxes, shown in place of
//! the table in the reading view (or of a long text field in the form), so a table can be
//! made and changed without writing its Markdown.
//!
//! With Calculate ranges ticked, each row says how many of the die's numbers it takes, and
//! the numbers each row covers are worked out from the top, so they always fit the die.
//! Unticked, the numbers are typed, like `2-3` or `12+`. Either way the table is written
//! back as an ordinary Markdown table, over the one it was read from.

use std::collections::BTreeMap;
use std::ops::Range;

use iced::keyboard::{Key, Modifiers, key};
use iced::widget::{
    button, checkbox, column, container, mouse_area, pick_list, row, space, text, text_editor,
    text_input, tooltip,
};
use iced::{Center, Element, Fill, Length};

use crate::dice::Expression;
use crate::style;
use crate::tables::{self, Table};
use crate::theme::Colors;

/// The choice in the die list that lets any die be typed.
const OTHER: &str = "Other...";
/// The widget id of the box the die is typed in.
const DIE_ID: &str = "table-editor-die";
/// How wide the Rolls, Takes and Chance columns are, so the rows line up.
const ROLLS_WIDTH: f32 = 72.0;
const TAKES_WIDTH: f32 = 92.0;
const CHANCE_WIDTH: f32 = 52.0;
/// The room the buttons at the end of each row take.
const ACTIONS_WIDTH: f32 = 96.0;

/// The widget id of the box for column `column` of row `row`.
pub fn cell_id(row: usize, column: usize) -> String {
    format!("table-cell-{row}-{column}")
}

/// The widget id of the box row `row`'s numbers are typed in.
fn numbers_id(row: usize) -> String {
    format!("table-numbers-{row}")
}

/// The widget id of the box column `column`'s header is typed in.
fn column_id(column: usize) -> String {
    format!("table-column-{column}")
}

#[derive(Debug, Clone)]
pub enum Message {
    /// A die picked from the list, or [`OTHER`] to type one.
    PickDie(String),
    /// The die typed in the Other box.
    TypeDie(String),
    Calculate(bool),
    SpreadEvenly,
    EachColumn(bool),
    Column(usize, String),
    AddColumn,
    /// Moves a column one place left (true) or right.
    MoveColumn(usize, bool),
    /// Removes a column, asking first if anything is in it.
    RemoveColumn(usize),
    /// The answer to removing a column with something in it.
    ConfirmRemoveColumn(bool),
    /// The numbers typed for a row, with Calculate ranges unticked.
    Numbers(usize, String),
    /// One more (true) or one fewer of the die's numbers for a row.
    Takes(usize, bool),
    Cell(usize, usize, text_editor::Action),
    AddRow,
    /// Enter in a row: a new row after it.
    AddRowAfter(usize),
    /// Moves a row one place up (true) or down.
    MoveRow(usize, bool),
    RemoveRow(usize),
    /// Backspace in an empty row: removes it and goes back to the row before.
    BackspaceRow(usize),
    /// Removes the rows the die has no numbers left for.
    RemoveExtraRows,
    /// A click on the editor, not on anything in it, which keeps it open.
    Click,
    Done,
    Cancel,
}

/// What a message did to the editor.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Editing,
    /// Editing is finished: the table goes back into the text.
    Done,
    /// Editing is finished, and what was changed is thrown away.
    Cancel,
}

/// A roll table being edited.
#[derive(Debug)]
pub struct Editor {
    /// Where the table is in the text it was read from.
    pub range: Range<usize>,
    /// The table's name, for what is wrong with it.
    name: String,
    /// The die as written in the first header, like `d6`.
    die: String,
    /// Whether the die is typed, rather than picked from the list.
    typing_die: bool,
    columns: Vec<String>,
    rows: Vec<Row>,
    calculate: bool,
    /// Whether it rolls each column on its own, if that can be set here.
    each_column: Option<bool>,
    /// A column with something in it that Remove was clicked for.
    removing_column: Option<usize>,
    /// Whether the table's text ended with a new line, so the new one does too.
    newline: bool,
    /// The box to focus after the last message.
    focus: Option<String>,
    changed: bool,
}

#[derive(Debug)]
struct Row {
    /// How many of the die's numbers it takes, with Calculate ranges ticked. A row the die
    /// has no numbers left for takes none.
    takes: i64,
    /// Its numbers as typed, with Calculate ranges unticked.
    numbers: String,
    cells: Vec<text_editor::Content>,
}

impl Row {
    fn new(takes: i64, numbers: String, columns: usize) -> Self {
        Row {
            takes,
            numbers,
            cells: (0..columns).map(|_| text_editor::Content::new()).collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.cells.iter().all(|cell| cell.text().trim().is_empty())
    }
}

impl Editor {
    /// Edits `table`, called `name`, which is in `source`. `each_column` is whether it
    /// rolls each column on its own, if that can be changed here.
    pub fn new(source: &str, table: &Table, name: &str, each_column: Option<bool>) -> Self {
        let mut written = table.written.iter();
        let header = written.next().cloned().unwrap_or_default();
        let die = header.first().cloned().unwrap_or_else(|| "d6".into());
        let mut columns: Vec<String> = header.iter().skip(1).cloned().collect();
        if columns.is_empty() {
            columns.push("Result".into());
        }
        let (low, high) = table.die.bounds();

        let mut rows = Vec::new();
        let mut spans = Vec::new();
        for cells in written {
            if cells.iter().all(|cell| cell.trim().is_empty()) {
                continue;
            }
            let numbers = cells.first().cloned().unwrap_or_default();
            let span = tables::numbers(&numbers, high);
            spans.push(span);
            let mut row = Row::new(
                span.map_or(1, |(start, end)| end - start + 1),
                numbers,
                columns.len(),
            );
            for (index, cell) in row.cells.iter_mut().enumerate() {
                if let Some(value) = cells.get(index + 1) {
                    *cell = text_editor::Content::with_text(value);
                }
            }
            rows.push(row);
        }

        // Numbers that run from the die's lowest to its highest, in order, can be worked
        // out again, so they are; others are kept as written.
        let mut next = low;
        let in_order = !spans.is_empty()
            && spans.iter().all(|span| match span {
                Some((start, end)) if *start == next => {
                    next = end + 1;
                    true
                }
                _ => false,
            })
            && next == high + 1;

        let ends = source.get(table.range.clone()).unwrap_or_default();
        Editor {
            range: table.range.clone(),
            name: name.to_string(),
            typing_die: !tables::DICE.contains(&die.as_str()),
            die,
            columns,
            rows,
            calculate: in_order,
            each_column,
            removing_column: None,
            newline: ends.ends_with('\n'),
            focus: Some(cell_id(0, 0)),
            changed: false,
        }
    }

    /// Whether anything was changed since it was opened.
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// Whether it rolls each column on its own, if that can be set here.
    pub fn each_column(&self) -> Option<bool> {
        self.each_column.map(|on| on && self.columns.len() > 1)
    }

    /// The box to focus, once, after the last message.
    pub fn take_focus(&mut self) -> Option<String> {
        self.focus.take()
    }

    /// The boxes Tab moves between, in order.
    pub fn tab_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        if self.typing_die {
            ids.push(DIE_ID.to_string());
        }
        ids.extend((0..self.columns.len()).map(column_id));
        for row in 0..self.rows.len() {
            if !self.calculate {
                ids.push(numbers_id(row));
            }
            ids.extend((0..self.columns.len()).map(|column| cell_id(row, column)));
        }
        ids
    }

    /// The lowest and highest the die rolls, if it can be read.
    fn bounds(&self) -> Option<(i64, i64)> {
        tables::die(&self.die).map(|die| die.bounds())
    }

    /// How many numbers the die rolls.
    fn total(&self) -> i64 {
        self.bounds().map_or(0, |(low, high)| high - low + 1)
    }

    /// The numbers each row covers, if any.
    fn spans(&self) -> Vec<Option<(i64, i64)>> {
        let Some((low, high)) = self.bounds() else {
            return vec![None; self.rows.len()];
        };
        if self.calculate {
            let mut next = low;
            self.rows
                .iter()
                .map(|row| {
                    (row.takes > 0).then(|| {
                        let span = (next, next + row.takes - 1);
                        next += row.takes;
                        span
                    })
                })
                .collect()
        } else {
            self.rows
                .iter()
                .map(|row| tables::numbers(&row.numbers, high))
                .collect()
        }
    }

    /// The numbers row `index` covers, as written in the table: `4`, `2-3`, or on a d100
    /// `01-05` and `00`.
    fn numbers_text(&self, span: Option<(i64, i64)>) -> String {
        let Some((start, end)) = span else {
            return String::new();
        };
        let high = self.bounds().map_or(0, |(_, high)| high);
        let number = |value: i64| {
            if high == 100 {
                format!("{value:02}").replace("100", "00")
            } else {
                value.to_string()
            }
        };
        if start == end {
            number(start)
        } else {
            format!("{}-{}", number(start), number(end))
        }
    }

    /// The table as Markdown, to put back where it was read from.
    pub fn markdown(&self) -> String {
        let mut written = Vec::new();
        let mut header = vec![self.die.trim().to_string()];
        header.extend(self.columns.iter().map(|column| column.trim().to_string()));
        written.push(header);
        let spans = self.spans();
        for (row, span) in self.rows.iter().zip(spans) {
            let numbers = if self.calculate {
                self.numbers_text(span)
            } else {
                row.numbers.trim().to_string()
            };
            let mut cells = vec![numbers];
            cells.extend(row.cells.iter().map(|cell| cell.text().trim().to_string()));
            written.push(cells);
        }
        let mut markdown = tables::write(&written);
        if !self.newline {
            markdown.truncate(markdown.trim_end().len());
        }
        markdown
    }

    /// What is wrong with the table, one line each.
    pub fn problems(&self) -> Vec<String> {
        let Some((low, high)) = self.bounds() else {
            return vec![format!(
                "\"{}\" isn't a die the table can roll. Try one like d6, 2d6 or d100",
                self.die.trim()
            )];
        };
        let mut problems = Vec::new();
        if self.rows.is_empty() {
            problems.push("The table has no rows yet".into());
            return problems;
        }
        let spans = self.spans();
        if self.calculate {
            let extra = self.rows.iter().filter(|row| row.takes == 0).count();
            if extra > 0 {
                problems.push(format!(
                    "{} can't be rolled: a {} only has {} numbers",
                    if extra == 1 {
                        "The last row".to_string()
                    } else {
                        format!("The last {extra} rows")
                    },
                    self.die.trim(),
                    high - low + 1
                ));
            }
            return problems;
        }

        for (index, (row, span)) in self.rows.iter().zip(&spans).enumerate() {
            if span.is_none() {
                problems.push(if row.numbers.trim().is_empty() {
                    format!("Row {} has no numbers, so it can't be rolled", index + 1)
                } else {
                    format!(
                        "Row {}'s numbers, \"{}\", aren't a number or a range like 2-3",
                        index + 1,
                        row.numbers.trim()
                    )
                });
            }
        }
        if high - low > 1000 {
            return problems;
        }
        let mut missing = Vec::new();
        // Runs of numbers in more than one row, and the rows.
        let mut twice: Vec<(i64, i64, Vec<usize>)> = Vec::new();
        for value in low..=high {
            let rows: Vec<usize> = spans
                .iter()
                .enumerate()
                .filter(|(_, span)| span.is_some_and(|(start, end)| (start..=end).contains(&value)))
                .map(|(index, _)| index + 1)
                .collect();
            match rows.len() {
                0 => missing.push(value),
                1 => {}
                _ => match twice.last_mut() {
                    Some((_, end, last)) if *last == rows && *end + 1 == value => *end = value,
                    _ => twice.push((value, value, rows)),
                },
            }
        }
        if !missing.is_empty() {
            problems.push(format!(
                "No row has {}, which the {} can roll",
                runs(&missing),
                self.die.trim()
            ));
        }
        for (start, end, rows) in twice {
            let first = rows[0];
            let rows: Vec<String> = rows.iter().map(usize::to_string).collect();
            let (numbers, verb, them) = if start == end {
                (start.to_string(), "is", "it")
            } else {
                (format!("{start}-{end}"), "are", "them")
            };
            problems.push(format!(
                "{numbers} {verb} in rows {}, so only row {first} is rolled for {them}",
                list(&rows)
            ));
        }
        let beyond = spans
            .iter()
            .flatten()
            .any(|(start, end)| *start < low || *end > high);
        if beyond {
            problems.push(format!(
                "Some numbers are ones the {} can't roll ({low} to {high})",
                self.die.trim()
            ));
        }
        problems
    }

    pub fn update(&mut self, message: Message) -> Outcome {
        let edits = !matches!(
            message,
            Message::Done
                | Message::Cancel
                | Message::Click
                | Message::Cell(..)
                | Message::RemoveColumn(_)
                | Message::ConfirmRemoveColumn(false)
        );
        self.changed |= edits;
        match message {
            Message::PickDie(die) => {
                if die == OTHER {
                    self.typing_die = true;
                    self.focus = Some(DIE_ID.into());
                } else {
                    self.typing_die = false;
                    self.set_die(die);
                }
            }
            Message::TypeDie(die) => self.set_die(die),
            Message::Calculate(on) => {
                if on && !self.calculate {
                    // Each row keeps its share of the die, worked out again from the top.
                    let weights: Vec<i64> = self
                        .spans()
                        .iter()
                        .map(|span| span.map_or(1, |(start, end)| end - start + 1))
                        .collect();
                    self.calculate = true;
                    self.share(&weights);
                } else if !on && self.calculate {
                    for (row, span) in (0..self.rows.len()).zip(self.spans()) {
                        self.rows[row].numbers = self.numbers_text(span);
                    }
                    self.calculate = false;
                }
            }
            Message::SpreadEvenly => {
                let total = self.total();
                let count = self.rows.len() as i64;
                if count > 0 {
                    for (index, row) in self.rows.iter_mut().enumerate() {
                        let index = index as i64;
                        row.takes = (total * (index + 1) / count - total * index / count).max(0);
                    }
                    if count > total {
                        let weights = vec![1; self.rows.len()];
                        self.share(&weights);
                    }
                }
            }
            Message::EachColumn(on) => {
                if let Some(each_column) = &mut self.each_column {
                    *each_column = on;
                }
            }
            Message::Column(index, value) => {
                if let Some(column) = self.columns.get_mut(index) {
                    *column = value;
                }
            }
            Message::AddColumn => {
                self.columns.push(String::new());
                for row in &mut self.rows {
                    row.cells.push(text_editor::Content::new());
                }
                self.focus = Some(column_id(self.columns.len() - 1));
            }
            Message::MoveColumn(index, left) => {
                let other = if left {
                    index.checked_sub(1)
                } else {
                    Some(index + 1)
                };
                if let Some(other) = other.filter(|other| *other < self.columns.len()) {
                    self.columns.swap(index, other);
                    for row in &mut self.rows {
                        row.cells.swap(index, other);
                    }
                }
            }
            Message::RemoveColumn(index) => {
                let filled = self.rows.iter().any(|row| {
                    row.cells
                        .get(index)
                        .is_some_and(|cell| !cell.text().trim().is_empty())
                });
                if filled {
                    self.removing_column = Some(index);
                } else {
                    self.remove_column(index);
                    self.changed = true;
                }
            }
            Message::ConfirmRemoveColumn(remove) => {
                if let Some(index) = self.removing_column.take()
                    && remove
                {
                    self.remove_column(index);
                }
            }
            Message::Numbers(index, value) => {
                if let Some(row) = self.rows.get_mut(index) {
                    row.numbers = value;
                }
            }
            Message::Takes(index, more) => {
                if more {
                    self.take_more(index);
                } else {
                    self.take_fewer(index);
                }
            }
            Message::Cell(row, column, action) => {
                self.changed |= action.is_edit();
                if let Some(cell) = self
                    .rows
                    .get_mut(row)
                    .and_then(|row| row.cells.get_mut(column))
                {
                    cell.perform(action);
                }
            }
            Message::AddRow => {
                self.add_row(self.rows.len());
                self.focus = Some(cell_id(self.rows.len() - 1, 0));
            }
            Message::AddRowAfter(index) => {
                self.add_row(index + 1);
                self.focus = Some(cell_id(index + 1, 0));
            }
            Message::MoveRow(index, up) => {
                let other = if up {
                    index.checked_sub(1)
                } else {
                    Some(index + 1)
                };
                if let Some(other) = other.filter(|other| *other < self.rows.len()) {
                    self.rows.swap(index, other);
                }
            }
            Message::RemoveRow(index) => self.remove_row(index),
            Message::BackspaceRow(index) => {
                if index > 0 {
                    self.remove_row(index);
                    self.focus = Some(cell_id(index - 1, self.columns.len() - 1));
                }
            }
            Message::RemoveExtraRows => {
                if self.calculate {
                    self.rows.retain(|row| row.takes > 0);
                }
            }
            Message::Click => {}
            Message::Done => return Outcome::Done,
            Message::Cancel => return Outcome::Cancel,
        }
        Outcome::Editing
    }

    /// Changes the die, keeping each row's share of it.
    fn set_die(&mut self, die: String) {
        let weights: Vec<i64> = self.rows.iter().map(|row| row.takes.max(1)).collect();
        self.die = die;
        if self.calculate && self.bounds().is_some() {
            self.share(&weights);
        }
    }

    /// Shares the die's numbers between the rows in proportion to `weights`, each row
    /// getting at least one while there are enough. Rows the die has too few numbers for
    /// get none.
    fn share(&mut self, weights: &[i64]) {
        let total = self.total();
        let count = self.rows.len() as i64;
        if count == 0 || total == 0 {
            return;
        }
        if count >= total {
            for (index, row) in self.rows.iter_mut().enumerate() {
                row.takes = i64::from((index as i64) < total);
            }
            return;
        }
        let sum: i64 = weights.iter().map(|weight| weight.max(&1)).sum();
        let exact: Vec<f64> = weights
            .iter()
            .map(|weight| (*weight.max(&1) as f64) * total as f64 / sum as f64)
            .collect();
        let mut takes: Vec<i64> = exact
            .iter()
            .map(|share| (share.floor() as i64).max(1))
            .collect();
        // Hand out (or take back) what rounding left over, by the biggest remainders.
        let mut order: Vec<usize> = (0..takes.len()).collect();
        order.sort_by(|a, b| {
            let left = |index: usize| exact[index] - exact[index].floor();
            left(*b).total_cmp(&left(*a))
        });
        let mut given: i64 = takes.iter().sum();
        let mut at = 0;
        while given < total {
            takes[order[at % order.len()]] += 1;
            given += 1;
            at += 1;
        }
        while given > total {
            // The biggest row, from the bottom, gives one back.
            let Some(biggest) = (0..takes.len())
                .rev()
                .filter(|index| takes[*index] > 1)
                .max_by_key(|index| takes[*index])
            else {
                break;
            };
            takes[biggest] -= 1;
            given -= 1;
        }
        for (row, takes) in self.rows.iter_mut().zip(takes) {
            row.takes = takes;
        }
    }

    /// The row nearest `index` with numbers to spare: below it first, then above.
    fn donor(&self, index: usize) -> Option<usize> {
        let below = (index + 1..self.rows.len()).find(|other| self.rows[*other].takes > 1);
        below.or_else(|| (0..index).rev().find(|other| self.rows[*other].takes > 1))
    }

    fn can_take_more(&self, index: usize) -> bool {
        self.donor(index).is_some()
    }

    fn can_take_fewer(&self, index: usize) -> bool {
        self.rows.len() > 1 && self.rows.get(index).is_some_and(|row| row.takes > 1)
    }

    fn take_more(&mut self, index: usize) {
        if let Some(donor) = self.donor(index) {
            self.rows[donor].takes -= 1;
            self.rows[index].takes += 1;
        }
    }

    fn take_fewer(&mut self, index: usize) {
        if !self.can_take_fewer(index) {
            return;
        }
        // The number goes to the row below, or for the last row, the one above.
        let receiver = if index + 1 < self.rows.len() {
            index + 1
        } else {
            index - 1
        };
        self.rows[index].takes -= 1;
        self.rows[receiver].takes += 1;
    }

    /// Adds an empty row at `index`, taking a number from the biggest row if the die has
    /// one to spare.
    fn add_row(&mut self, index: usize) {
        let index = index.min(self.rows.len());
        let mut row = Row::new(0, String::new(), self.columns.len());
        if self.rows.is_empty() {
            row.takes = self.total();
        } else if let Some(biggest) = (0..self.rows.len())
            .rev()
            .filter(|other| self.rows[*other].takes > 1)
            .max_by_key(|other| self.rows[*other].takes)
        {
            self.rows[biggest].takes -= 1;
            row.takes = 1;
        }
        if !self.calculate {
            // The number after the highest one written, if the die rolls it.
            let high = self.bounds().map_or(0, |(_, high)| high);
            let next = self.spans().iter().flatten().map(|(_, end)| end + 1).max();
            if let Some(next) = next.filter(|next| *next <= high) {
                row.numbers = self.numbers_text(Some((next, next)));
            }
        }
        self.rows.insert(index, row);
    }

    /// Removes row `index`. Its numbers go to the row above, or the one below if it was
    /// first.
    fn remove_row(&mut self, index: usize) {
        if index >= self.rows.len() {
            return;
        }
        let row = self.rows.remove(index);
        let receiver = index
            .checked_sub(1)
            .or((!self.rows.is_empty()).then_some(0));
        if let Some(receiver) = receiver {
            self.rows[receiver].takes += row.takes;
        }
    }

    fn remove_column(&mut self, index: usize) {
        if self.columns.len() <= 1 || index >= self.columns.len() {
            return;
        }
        self.columns.remove(index);
        for row in &mut self.rows {
            if index < row.cells.len() {
                row.cells.remove(index);
            }
        }
    }

    /// The chance of each row, from 0 to 1, if the die's chances can be worked out.
    fn chances(&self) -> Vec<Option<f64>> {
        let odds: Option<BTreeMap<i64, f64>> = Expression::parse(self.die.trim())
            .ok()
            .and_then(|die| die.odds());
        // A number in more than one row only ever rolls the first.
        let mut taken = std::collections::BTreeSet::new();
        self.spans()
            .into_iter()
            .map(|span| {
                let odds = odds.as_ref()?;
                let (start, end) = span?;
                Some(
                    odds.range(start..=end)
                        .filter(|(value, _)| taken.insert(**value))
                        .map(|(_, chance)| chance)
                        .sum(),
                )
            })
            .collect()
    }

    /// The editor. `shortcut` gives the app's keyboard shortcuts in the boxes.
    pub fn view<'a, M: Clone + 'a>(
        &'a self,
        c: Colors,
        map: impl Fn(Message) -> M + Copy + 'a,
        shortcut: fn(&Key, Modifiers) -> Option<M>,
    ) -> Element<'a, M> {
        let small = |label: &'a str, message: Option<Message>| {
            button(text(label).size(12))
                .padding([2, 6])
                .style(style::flat(c, false))
                .on_press_maybe(message.map(map))
        };
        let hint = |content: Element<'a, M>, tip: &'a str| -> Element<'a, M> {
            tooltip(
                content,
                container(text(tip).size(12))
                    .padding([4, 8])
                    .style(style::dropdown(c)),
                tooltip::Position::Top,
            )
            .into()
        };

        // The die, how ranges are worked out, and Done.
        let mut dice: Vec<String> = tables::DICE.iter().map(|die| die.to_string()).collect();
        dice.push(OTHER.into());
        let picked = if self.typing_die {
            OTHER.to_string()
        } else {
            self.die.clone()
        };
        let title = row![
            text(format!("{} table", self.name))
                .size(14)
                .color(c.text)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..iced::Font::DEFAULT
                }),
            space::horizontal(),
            button(text("Cancel").size(13))
                .padding([4, 10])
                .style(style::secondary(c))
                .on_press(map(Message::Cancel)),
            button(text("Done").size(13))
                .padding([4, 12])
                .style(style::primary(c))
                .on_press(map(Message::Done)),
        ]
        .spacing(8)
        .align_y(Center);
        let mut top = row![
            text("Die").size(13).color(c.text_muted),
            pick_list(dice, Some(picked), move |die| map(Message::PickDie(die)))
                .text_size(13)
                .padding([4, 8])
                .style(style::choice(c))
                .menu_style(style::choice_menu(c)),
        ]
        .spacing(8)
        .align_y(Center);
        if self.typing_die {
            top = top.push(
                text_input("like 3d6 or d30", &self.die)
                    .id(DIE_ID)
                    .on_input(move |die| map(Message::TypeDie(die)))
                    .size(13)
                    .padding([4, 8])
                    .width(110)
                    .style(style::input(c)),
            );
        }
        top = top.push(space().width(8)).push(hint(
            checkbox(self.calculate)
                .label("Calculate ranges")
                .text_size(13)
                .on_toggle(move |on| map(Message::Calculate(on)))
                .style(style::check(c))
                .into(),
            "Ticked, each row takes some of the die's numbers and they are worked out for you. Untick it to type your own, like 2-3 or 12+",
        ));
        if self.calculate {
            top = top.push(
                button(text("Spread Evenly").size(12))
                    .padding([3, 8])
                    .style(style::secondary(c))
                    .on_press(map(Message::SpreadEvenly)),
            );
        }
        if let Some(each_column) = self.each_column
            && self.columns.len() > 1
        {
            top = top.push(space().width(8)).push(
                checkbox(each_column)
                    .label("Roll each column on its own")
                    .text_size(13)
                    .on_toggle(move |on| map(Message::EachColumn(on)))
                    .style(style::check(c)),
            );
        }

        // The headers: Rolls, Takes, Chance, then a box for each column's.
        let label = |label: &'a str, width: f32| {
            text(label)
                .size(12)
                .color(c.text_muted)
                .width(Length::Fixed(width))
        };
        let mut header = row![label("Rolls", ROLLS_WIDTH)].spacing(6).align_y(Center);
        if self.calculate {
            header = header.push(label("Takes", TAKES_WIDTH));
        }
        header = header.push(label("Chance", CHANCE_WIDTH));
        let last_column = self.columns.len() - 1;
        for (index, name) in self.columns.iter().enumerate() {
            let input = text_input("Column name", name)
                .id(column_id(index))
                .on_input(move |value| map(Message::Column(index, value)))
                .size(13)
                .padding([4, 6])
                .style(style::input(c));
            let mut tools = row![].spacing(0);
            if self.columns.len() > 1 {
                tools = tools
                    .push(small(
                        "←",
                        (index > 0).then_some(Message::MoveColumn(index, true)),
                    ))
                    .push(small(
                        "→",
                        (index < last_column).then_some(Message::MoveColumn(index, false)),
                    ))
                    .push(small("×", Some(Message::RemoveColumn(index))));
            }
            header = header.push(row![input, tools].spacing(2).align_y(Center).width(Fill));
        }
        header = header.push(
            container(hint(
                small("+ Column", Some(Message::AddColumn)).into(),
                "Add a column of results",
            ))
            .width(Length::Fixed(ACTIONS_WIDTH)),
        );

        // A line for each row.
        let spans = self.spans();
        let chances = self.chances();
        let last_row = self.rows.len().saturating_sub(1);
        let mut rows = column![].spacing(6);
        for (index, row) in self.rows.iter().enumerate() {
            let numbers: Element<'a, M> = if self.calculate {
                let written = self.numbers_text(spans[index]);
                let shown = if written.is_empty() {
                    text("can't roll").size(12).color(c.warning)
                } else {
                    text(written).size(13).color(c.text)
                };
                container(shown)
                    .width(Length::Fixed(ROLLS_WIDTH))
                    .padding([6, 0])
                    .into()
            } else {
                text_input("2-3", &row.numbers)
                    .id(numbers_id(index))
                    .on_input(move |value| map(Message::Numbers(index, value)))
                    .size(13)
                    .padding([4, 6])
                    .width(Length::Fixed(ROLLS_WIDTH))
                    .style(style::input(c))
                    .into()
            };
            let mut line = row![numbers].spacing(6).align_y(Center);
            if self.calculate {
                line = line.push(
                    row![
                        small(
                            "-",
                            self.can_take_fewer(index)
                                .then_some(Message::Takes(index, false))
                        ),
                        text(row.takes.to_string())
                            .size(13)
                            .width(24)
                            .align_x(Center),
                        small(
                            "+",
                            self.can_take_more(index)
                                .then_some(Message::Takes(index, true))
                        ),
                    ]
                    .spacing(2)
                    .align_y(Center)
                    .width(Length::Fixed(TAKES_WIDTH)),
                );
            }
            let chance = match chances[index] {
                Some(chance) if chance > 0.0 && chance < 0.005 => "<1%".to_string(),
                Some(chance) => format!("{:.0}%", chance * 100.0),
                None => String::new(),
            };
            line = line.push(
                text(chance)
                    .size(12)
                    .color(c.text_muted)
                    .width(Length::Fixed(CHANCE_WIDTH)),
            );
            let empty = row.is_empty();
            for (column, cell) in row.cells.iter().enumerate() {
                line = line.push(
                    text_editor(cell)
                        .id(cell_id(index, column))
                        .placeholder(if column == 0 { "Result" } else { "" })
                        .on_action(move |action| map(Message::Cell(index, column, action)))
                        .key_binding(cell_keys(map, shortcut, index, empty))
                        .size(13)
                        .padding([4, 6])
                        .style(style::field_editor(c)),
                );
            }
            line = line.push(
                row![
                    small("↑", (index > 0).then_some(Message::MoveRow(index, true))),
                    small(
                        "↓",
                        (index < last_row).then_some(Message::MoveRow(index, false))
                    ),
                    small("×", Some(Message::RemoveRow(index))),
                ]
                .spacing(0)
                .width(Length::Fixed(ACTIONS_WIDTH)),
            );
            rows = rows.push(line);
        }

        let mut editor = column![title, top, header, rows].spacing(10);
        editor = editor.push(
            row![
                button(text("+ Add Row").size(13))
                    .padding([4, 10])
                    .style(style::secondary(c))
                    .on_press(map(Message::AddRow)),
                text("Enter adds a row, Shift+Enter a new line in a result")
                    .size(12)
                    .color(c.text_muted),
            ]
            .spacing(12)
            .align_y(Center),
        );

        if let Some(index) = self.removing_column {
            let name = self.columns.get(index).map_or("", String::as_str);
            let name = if name.trim().is_empty() {
                "this column".to_string()
            } else {
                format!("the {} column", name.trim())
            };
            editor = editor.push(
                row![
                    text(format!("Remove {name} and the results in it?"))
                        .size(13)
                        .color(c.text),
                    button(text("Remove").size(12))
                        .padding([3, 8])
                        .style(style::secondary(c))
                        .on_press(map(Message::ConfirmRemoveColumn(true))),
                    button(text("Keep It").size(12))
                        .padding([3, 8])
                        .style(style::secondary(c))
                        .on_press(map(Message::ConfirmRemoveColumn(false))),
                ]
                .spacing(8)
                .align_y(Center),
            );
        }

        let problems = self.problems();
        let extra = self.calculate && self.rows.iter().any(|row| row.takes == 0);
        if !problems.is_empty() {
            let mut list = column![].spacing(4);
            for problem in problems {
                list = list.push(text(problem).size(12).color(c.warning));
            }
            if extra {
                list = list.push(
                    button(text("Remove the Rows That Can't Roll").size(12))
                        .padding([3, 8])
                        .style(style::secondary(c))
                        .on_press(map(Message::RemoveExtraRows)),
                );
            }
            editor = editor.push(list);
        }

        mouse_area(
            container(editor)
                .padding(12)
                .width(Fill)
                .style(style::dialog(c)),
        )
        .on_press(map(Message::Click))
        .into()
    }
}

/// The keys a result box takes: Enter adds a row after it, Shift+Enter a new line,
/// Backspace in an empty row removes it, and the app's shortcuts work.
fn cell_keys<M: Clone>(
    map: impl Fn(Message) -> M,
    shortcut: fn(&Key, Modifiers) -> Option<M>,
    row: usize,
    empty: bool,
) -> impl Fn(text_editor::KeyPress) -> Option<text_editor::Binding<M>> {
    move |key_press: text_editor::KeyPress| {
        if !matches!(key_press.status, text_editor::Status::Focused { .. }) {
            return None;
        }
        match key_press.key.as_ref() {
            Key::Named(key::Named::Enter) if !key_press.modifiers.shift() => {
                return Some(text_editor::Binding::Custom(map(Message::AddRowAfter(row))));
            }
            Key::Named(key::Named::Backspace) if empty && row > 0 => {
                return Some(text_editor::Binding::Custom(map(Message::BackspaceRow(
                    row,
                ))));
            }
            _ => {}
        }
        if key_press.modifiers.command()
            && let Some(message) = shortcut(&key_press.key, key_press.modifiers)
        {
            return Some(text_editor::Binding::Custom(message));
        }
        text_editor::Binding::from_key_press(key_press)
    }
}

/// `values`, in order, as runs: `1`, `4-6`, listed to read.
fn runs(values: &[i64]) -> String {
    let mut runs: Vec<(i64, i64)> = Vec::new();
    for value in values {
        match runs.last_mut() {
            Some((_, end)) if *end + 1 == *value => *end = *value,
            _ => runs.push((*value, *value)),
        }
    }
    let runs: Vec<String> = runs
        .into_iter()
        .map(|(start, end)| {
            if start == end {
                start.to_string()
            } else {
                format!("{start}-{end}")
            }
        })
        .collect();
    list(&runs)
}

/// `items` as a list to read: `1`, `1 and 3`, `1, 3 and 4-6`.
fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(source: &str) -> Editor {
        let table = tables::find(source).remove(0);
        Editor::new(source, &table, "Weather", Some(false))
    }

    fn numbers(editor: &Editor) -> Vec<String> {
        editor
            .spans()
            .into_iter()
            .map(|span| editor.numbers_text(span))
            .collect()
    }

    const WEATHER: &str = "Before.\n\n| d6  | Weather        |\n| --- | -------------- |\n| 1   | Clear and cold |\n| 2-3 | Grey drizzle   |\n| 4-6 | [[Storms]]     |\n\nAfter.\n";

    #[test]
    fn writes_back_the_table_it_read() {
        let editor = editor(WEATHER);
        assert!(editor.calculate);
        let table = &WEATHER[editor.range.clone()];
        assert_eq!(editor.markdown(), table);
        assert!(editor.problems().is_empty());
        assert!(!editor.changed());
    }

    #[test]
    fn keeps_numbers_that_have_gaps_as_written() {
        let source = "| 2d6 | Encounter |\n| --- | --- |\n| 2-6 | Wolves |\n| 8-12 | Bandits |\n";
        let editor = editor(source);
        assert!(!editor.calculate);
        assert_eq!(editor.rows[1].numbers, "8-12");
        assert_eq!(editor.problems(), ["No row has 7, which the 2d6 can roll"]);
        assert!(editor.markdown().contains("| 8-12 | Bandits"));
    }

    #[test]
    fn takes_more_from_the_rows_below() {
        let mut editor = editor(WEATHER);
        editor.update(Message::Takes(0, true));
        assert_eq!(numbers(&editor), ["1-2", "3", "4-6"]);
        // The second row has one number left, so the third gives it.
        editor.update(Message::Takes(0, true));
        assert_eq!(numbers(&editor), ["1-3", "4", "5-6"]);
        editor.update(Message::Takes(2, false));
        assert_eq!(numbers(&editor), ["1-3", "4-5", "6"]);
        assert!(editor.changed());
    }

    #[test]
    fn changing_the_die_keeps_each_rows_share() {
        let mut editor = editor(WEATHER);
        editor.update(Message::PickDie("d20".into()));
        assert_eq!(numbers(&editor), ["1-3", "4-10", "11-20"]);
        editor.update(Message::PickDie("d100".into()));
        assert_eq!(numbers(&editor), ["01-15", "16-50", "51-00"]);
        editor.update(Message::PickDie("d4".into()));
        assert_eq!(numbers(&editor), ["1", "2", "3-4"]);
    }

    #[test]
    fn rows_the_die_cant_roll() {
        let mut editor = editor(WEATHER);
        editor.update(Message::AddRow);
        editor.update(Message::AddRow);
        editor.update(Message::AddRow);
        editor.update(Message::AddRow);
        assert_eq!(numbers(&editor), ["1", "2", "3", "4", "5", "6", ""]);
        assert_eq!(editor.problems().len(), 1);
        editor.update(Message::RemoveExtraRows);
        assert_eq!(editor.rows.len(), 6);
        assert!(editor.problems().is_empty());
    }

    #[test]
    fn removing_a_row_gives_its_numbers_to_the_one_above() {
        let mut editor = editor(WEATHER);
        editor.update(Message::RemoveRow(1));
        assert_eq!(numbers(&editor), ["1-3", "4-6"]);
        editor.update(Message::RemoveRow(0));
        assert_eq!(numbers(&editor), ["1-6"]);
    }

    #[test]
    fn typed_numbers_are_checked() {
        let mut editor = editor(WEATHER);
        editor.update(Message::Calculate(false));
        assert_eq!(editor.rows[1].numbers, "2-3");
        editor.update(Message::Numbers(1, "2-4".into()));
        assert_eq!(
            editor.problems(),
            ["4 is in rows 2 and 3, so only row 2 is rolled for it"]
        );
        editor.update(Message::Numbers(1, "two".into()));
        assert_eq!(
            editor.problems()[0],
            "Row 2's numbers, \"two\", aren't a number or a range like 2-3"
        );
        // Ticking it again shares the die as the rows had it.
        editor.update(Message::Numbers(1, "2-3".into()));
        editor.update(Message::Calculate(true));
        assert_eq!(numbers(&editor), ["1", "2-3", "4-6"]);
    }

    #[test]
    fn line_breaks_and_pipes_are_written_so_the_table_holds() {
        let source = "| d4 | Result |\n| --- | --- |\n| 1-4 | One<br>Two \\| three |\n";
        let mut editor = editor(source);
        assert_eq!(editor.rows[0].cells[0].text(), "One\nTwo | three");
        editor.update(Message::Cell(
            0,
            0,
            text_editor::Action::Edit(text_editor::Edit::Paste(std::sync::Arc::new("x".into()))),
        ));
        assert!(editor.markdown().contains("One<br>Two \\| three"));
    }

    #[test]
    fn columns_are_added_moved_and_removed() {
        let mut editor = editor(WEATHER);
        editor.update(Message::AddColumn);
        editor.update(Message::Column(1, "Wind".into()));
        editor.update(Message::MoveColumn(1, true));
        assert_eq!(editor.columns, ["Wind", "Weather"]);
        // Removing a column with results in it asks first.
        editor.update(Message::RemoveColumn(1));
        assert_eq!(editor.columns.len(), 2);
        editor.update(Message::ConfirmRemoveColumn(true));
        assert_eq!(editor.columns, ["Wind"]);
        // The last column stays.
        editor.update(Message::RemoveColumn(0));
        assert_eq!(editor.columns, ["Wind"]);
    }

    #[test]
    fn chances_follow_the_dice() {
        let source =
            "| 2d6 | Result |\n| --- | --- |\n| 2-6 | Low |\n| 7 | Seven |\n| 8-12 | High |\n";
        let editor = editor(source);
        let chances: Vec<i64> = editor
            .chances()
            .into_iter()
            .map(|chance| (chance.unwrap() * 36.0).round() as i64)
            .collect();
        assert_eq!(chances, [15, 6, 15]);
    }

    #[test]
    fn spread_evenly() {
        let mut editor = editor(WEATHER);
        editor.update(Message::PickDie("d20".into()));
        editor.update(Message::SpreadEvenly);
        assert_eq!(numbers(&editor), ["1-6", "7-13", "14-20"]);
    }

    #[test]
    fn a_table_at_the_end_without_a_new_line_stays_that_way() {
        let source = "| d2 | Coin |\n| --- | --- |\n| 1 | Heads |\n| 2 | Tails |";
        let editor = editor(source);
        assert!(!editor.markdown().ends_with('\n'));
    }
}
