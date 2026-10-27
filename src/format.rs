//! Formatting Markdown for people who don't know it: the toolbar's text types, bold,
//! italic, strikethrough and lists, which write the Markdown marks into the text editor,
//! and lists that carry on when Enter is pressed.
//!
//! Each command works on the lines the cursor or selection is on, or the words selected
//! (the word the cursor is in when nothing is), so it does what a word processor would.
//! Pressing it again takes the marks away.

use std::fmt;
use std::sync::Arc;

use iced::widget::text_editor::{Action, Content, Cursor, Edit, Position};

/// What a line of text is, picked from the toolbar's dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextType {
    Paragraph,
    /// `#` to `####`.
    Heading(u8),
    /// `> `
    Quote,
}

impl TextType {
    pub const ALL: [TextType; 6] = [
        TextType::Paragraph,
        TextType::Heading(1),
        TextType::Heading(2),
        TextType::Heading(3),
        TextType::Heading(4),
        TextType::Quote,
    ];
}

impl fmt::Display for TextType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextType::Paragraph => write!(f, "Paragraph"),
            TextType::Heading(level) => write!(f, "Heading {level}"),
            TextType::Quote => write!(f, "Quote"),
        }
    }
}

/// A kind of list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum List {
    /// `- `
    Bullet,
    /// `1. `
    Numbered,
    /// `- [ ] `
    Task,
}

/// Something the toolbar (or Ctrl+B, Ctrl+I) does to the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Text(TextType),
    Bold,
    Italic,
    Strikethrough,
    List(List),
}

/// The marks at the start of a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    None,
    Heading(u8),
    Quote,
    /// `-`, `*` or `+`.
    Bullet(char),
    /// The number and what follows it, `.` or `)`.
    Numbered(u64, char),
}

/// A line split into its indent, the marks at its start, and the text after them.
#[derive(Debug, Clone, Copy)]
struct Line<'a> {
    indent: &'a str,
    mark: Mark,
    /// `Some(done)` for a task: `[ ] ` after a list mark.
    task: Option<bool>,
    /// Where the text after the marks starts.
    start: usize,
    text: &'a str,
}

impl<'a> Line<'a> {
    fn parse(line: &'a str) -> Line<'a> {
        let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
        let (indent, after) = line.split_at(indent_len);
        let plain = Line {
            indent,
            mark: Mark::None,
            task: None,
            start: indent_len,
            text: after,
        };
        // A mark ends with a space, or the line ends right after it.
        let with = |mark, len: usize| {
            let rest = &after[len..];
            let len = match rest.strip_prefix([' ', '\t']) {
                Some(_) => len + 1,
                None if rest.is_empty() => len,
                None => return None,
            };
            Some((mark, len))
        };

        let hashes = after.len() - after.trim_start_matches('#').len();
        let marked = if (1..=6).contains(&hashes) {
            with(Mark::Heading(hashes as u8), hashes)
        } else if let Some(quoted) = after.strip_prefix('>') {
            // `>text` is a quote too.
            Some((Mark::Quote, if quoted.starts_with(' ') { 2 } else { 1 }))
        } else if let Some(bullet) = after
            .chars()
            .next()
            .filter(|c| matches!(c, '-' | '*' | '+'))
        {
            with(Mark::Bullet(bullet), 1)
        } else {
            let digits = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            match after[digits..].chars().next() {
                Some(close @ ('.' | ')')) if (1..=9).contains(&digits) => {
                    let number = after[..digits].parse().unwrap_or(1);
                    with(Mark::Numbered(number, close), digits + 1)
                }
                _ => None,
            }
        };
        let Some((mark, len)) = marked else {
            return plain;
        };

        let mut line = Line {
            mark,
            start: indent_len + len,
            text: &after[len..],
            ..plain
        };
        if matches!(mark, Mark::Bullet(_) | Mark::Numbered(..)) {
            let task = match line.text.get(..3) {
                Some("[ ]") => Some(false),
                Some("[x]" | "[X]") => Some(true),
                _ => None,
            };
            let rest = line.text.get(3..).unwrap_or("");
            if let Some(done) = task
                && (rest.is_empty() || rest.starts_with(' '))
            {
                let len = if rest.is_empty() { 3 } else { 4 };
                line.task = Some(done);
                line.start += len;
                line.text = &line.text[len..];
            }
        }
        line
    }

    fn list(&self) -> Option<List> {
        match (self.mark, self.task) {
            (Mark::Bullet(_) | Mark::Numbered(..), Some(_)) => Some(List::Task),
            (Mark::Bullet(_), None) => Some(List::Bullet),
            (Mark::Numbered(..), None) => Some(List::Numbered),
            _ => None,
        }
    }

    fn text_type(&self) -> Option<TextType> {
        match self.mark {
            Mark::None => Some(TextType::Paragraph),
            Mark::Heading(level) => Some(TextType::Heading(level)),
            Mark::Quote => Some(TextType::Quote),
            _ => None,
        }
    }
}

/// What the line the cursor is on is, for the toolbar's dropdown; `None` in a list.
pub fn text_type(content: &Content) -> Option<TextType> {
    let line = content.line(content.cursor().position.line)?;
    Line::parse(&line.text).text_type()
}

/// The kind of list the line the cursor is on is in, if any, to show its button pressed.
pub fn list(content: &Content) -> Option<List> {
    let line = content.line(content.cursor().position.line)?;
    Line::parse(&line.text).list()
}

/// Does `command` to the text in `content`.
pub fn apply(content: &mut Content, command: Command) {
    let cursor = content.cursor();
    let (start, end) = ordered(cursor);
    let lines: Vec<String> = (start.line..=end.line)
        .map(|index| {
            content
                .line(index)
                .map(|line| line.text.into_owned())
                .unwrap_or_default()
        })
        .collect();
    let selection = Selection {
        start: (0, start.column),
        end: (end.line - start.line, end.column),
        caret: cursor
            .selection
            .is_none_or(|selection| selection == cursor.position),
    };

    let changed = match command {
        Command::Text(text_type) => set_text_type(&lines, selection, text_type),
        Command::List(list) => toggle_list(&lines, selection, list),
        Command::Bold => toggle_style(&lines, selection, Style::Bold),
        Command::Italic => toggle_style(&lines, selection, Style::Italic),
        Command::Strikethrough => toggle_style(&lines, selection, Style::Strikethrough),
    };
    if changed.lines != lines {
        replace_lines(content, start.line, &lines, &changed.lines);
    }

    let at = |(line, column): (usize, usize)| Position {
        line: start.line + line,
        column,
    };
    content.move_to(Cursor {
        position: at(changed.end),
        selection: (changed.start != changed.end).then(|| at(changed.start)),
    });
}

/// What `Enter` does at the end of a list item or quote: starts the next one, numbered
/// one more and with an empty task box. On an item with nothing in it, it ends the list
/// instead. Returns whether it did either, or `Enter` should do what it always does.
pub fn continue_list(content: &mut Content) -> bool {
    let cursor = content.cursor();
    if cursor
        .selection
        .is_some_and(|selection| selection != cursor.position)
    {
        return false;
    }
    let Some(text) = content
        .line(cursor.position.line)
        .map(|line| line.text.into_owned())
    else {
        return false;
    };
    let line = Line::parse(&text);
    if !matches!(
        line.mark,
        Mark::Bullet(_) | Mark::Numbered(..) | Mark::Quote
    ) || cursor.position.column < line.start
    {
        return false;
    }

    if line.text.trim().is_empty() {
        // An empty item ends the list.
        let line_index = cursor.position.line;
        replace_lines(
            content,
            line_index,
            std::slice::from_ref(&text),
            &[String::new()],
        );
        content.move_to(Cursor {
            position: Position {
                line: line_index,
                column: 0,
            },
            selection: None,
        });
        return true;
    }

    let mark = match line.mark {
        Mark::Bullet(bullet) => format!("{bullet} "),
        Mark::Numbered(number, close) => format!("{}{close} ", number + 1),
        _ => "> ".into(),
    };
    let task = if line.task.is_some() { "[ ] " } else { "" };
    content.perform(Action::Edit(Edit::Enter));
    // Spaces after the cursor would come after the marks.
    while content
        .line(cursor.position.line + 1)
        .is_some_and(|line| line.text.starts_with(' '))
    {
        content.perform(Action::Edit(Edit::Delete));
    }
    content.perform(Action::Edit(Edit::Paste(Arc::new(format!(
        "{}{mark}{task}",
        line.indent
    )))));
    true
}

/// The start and end of the selection (or the cursor twice), in order.
fn ordered(cursor: Cursor) -> (Position, Position) {
    let other = cursor.selection.unwrap_or(cursor.position);
    let key = |p: &Position| (p.line, p.column);
    if key(&other) < key(&cursor.position) {
        (other, cursor.position)
    } else {
        (cursor.position, other)
    }
}

/// Puts `new` in place of the lines `old`, which start at line `first` of `content`.
fn replace_lines(content: &mut Content, first: usize, old: &[String], new: &[String]) {
    let last = first + old.len() - 1;
    content.move_to(Cursor {
        position: Position {
            line: last,
            column: old.last().map_or(0, String::len),
        },
        selection: Some(Position {
            line: first,
            column: 0,
        }),
    });
    let text = new.join("\n");
    if text.is_empty() {
        content.perform(Action::Edit(Edit::Backspace));
    } else {
        content.perform(Action::Edit(Edit::Paste(Arc::new(text))));
    }
}

/// Where the selection is in the lines a command works on, as (line, byte column).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Selection {
    start: (usize, usize),
    end: (usize, usize),
    /// Whether nothing is selected.
    caret: bool,
}

/// The lines after a command, and what to select in them.
#[derive(Debug, PartialEq)]
struct Changed {
    lines: Vec<String>,
    start: (usize, usize),
    end: (usize, usize),
}

/// Changes the marks at the start of each line with `remark`, which gets the line and
/// its index among the lines changed; the selection keeps its place in the text.
fn remark_lines(
    lines: &[String],
    selection: Selection,
    mut remark: impl FnMut(&Line, usize) -> Option<String>,
) -> Changed {
    let mut changed = Vec::with_capacity(lines.len());
    // How far each line's text moved, and where it starts now.
    let mut moves = Vec::with_capacity(lines.len());
    let mut count = 0;
    for line in lines {
        let parsed = Line::parse(line);
        match remark(&parsed, count) {
            Some(marks) => {
                count += 1;
                moves.push((parsed.start, marks.len()));
                changed.push(format!("{marks}{}", parsed.text));
            }
            None => {
                moves.push((0, 0));
                changed.push(line.clone());
            }
        }
    }
    let place = |(line, column): (usize, usize)| {
        let (old, new) = moves[line];
        (line, column.max(old) - old + new)
    };
    Changed {
        lines: changed,
        start: place(selection.start),
        end: place(selection.end),
    }
}

fn set_text_type(lines: &[String], selection: Selection, text_type: TextType) -> Changed {
    remark_lines(lines, selection, |line, _| {
        // Blank lines between paragraphs stay blank, unless the cursor is on one.
        if line.text.trim().is_empty() && lines.len() > 1 {
            return None;
        }
        Some(match text_type {
            TextType::Paragraph => String::new(),
            TextType::Heading(level) => format!("{} ", "#".repeat(level as usize)),
            TextType::Quote => format!("{}> ", line.indent),
        })
    })
}

fn toggle_list(lines: &[String], selection: Selection, list: List) -> Changed {
    let blank = |line: &Line| line.text.trim().is_empty() && line.mark == Mark::None;
    let parsed: Vec<Line> = lines.iter().map(|line| Line::parse(line)).collect();
    let all_in_it = parsed
        .iter()
        .filter(|line| !blank(line) || lines.len() == 1)
        .all(|line| line.list() == Some(list));

    remark_lines(lines, selection, |line, index| {
        if blank(line) && lines.len() > 1 {
            return None;
        }
        if all_in_it {
            return Some(line.indent.to_string());
        }
        // A task keeps its tick when a list becomes tasks again.
        let done = line.task.unwrap_or(false);
        let bullet = match line.mark {
            Mark::Bullet(bullet) => bullet,
            _ => '-',
        };
        Some(match list {
            List::Bullet => format!("{}{bullet} ", line.indent),
            List::Numbered => format!("{}{}. ", line.indent, index + 1),
            List::Task => format!(
                "{}{bullet} [{}] ",
                line.indent,
                if done { 'x' } else { ' ' }
            ),
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Style {
    Bold,
    Italic,
    Strikethrough,
}

/// Bold, italic and strikethrough marks around a piece of text.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Styles {
    bold: bool,
    italic: bool,
    strikethrough: bool,
}

impl Styles {
    /// What the marks `marks` (like `~~**`) before some text make it.
    fn read(marks: &str) -> Styles {
        let stars = marks.matches(['*', '_']).count();
        Styles {
            bold: stars >= 2,
            italic: stars % 2 == 1,
            strikethrough: marks.matches('~').count() >= 2,
        }
    }

    fn get_mut(&mut self, style: Style) -> &mut bool {
        match style {
            Style::Bold => &mut self.bold,
            Style::Italic => &mut self.italic,
            Style::Strikethrough => &mut self.strikethrough,
        }
    }

    /// The marks before the text, and after it.
    fn marks(self) -> (String, String) {
        let stars = "*".repeat(usize::from(self.bold) * 2 + usize::from(self.italic));
        let tildes = if self.strikethrough { "~~" } else { "" };
        (format!("{tildes}{stars}"), format!("{stars}{tildes}"))
    }
}

fn is_mark(byte: u8) -> bool {
    matches!(byte, b'*' | b'~' | b'_')
}

/// Makes the selected text (or the word the cursor is in) bold, italic or crossed out, or
/// not if it already is. With nothing to style, the marks go in with the cursor between.
fn toggle_style(lines: &[String], selection: Selection, style: Style) -> Changed {
    let mut changed: Vec<String> = lines.to_vec();
    // The piece of each line to style.
    let mut pieces: Vec<(usize, usize, usize)> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let from = if index == selection.start.0 {
            selection.start.1
        } else {
            Line::parse(line).start
        };
        let to = if index == selection.end.0 {
            selection.end.1
        } else {
            line.len()
        };
        let (from, to) = if selection.caret {
            word_at(line, from)
        } else {
            // Spaces at the ends stay outside the marks.
            let piece = &line[from..to.max(from)];
            let start = from + (piece.len() - piece.trim_start().len());
            (start, start.max(from + piece.trim_end().len()))
        };
        if from < to || selection.caret {
            pieces.push((index, from, to));
        }
    }

    // Every piece gets the style if any lacks it, or loses it if all have it.
    let has = |line: &str, from: usize, to: usize| {
        let (start, core_start, _, _) = spread(line, from, to);
        *Styles::read(&line[start..core_start]).get_mut(style)
    };
    let all_have = pieces
        .iter()
        .all(|&(index, from, to)| has(&lines[index], from, to));

    let mut first = None;
    let mut last = (selection.end.0, selection.end.1);
    for &(index, from, to) in &pieces {
        let line = &lines[index];
        let (start, core_start, core_end, end) = spread(line, from, to);
        let mut styles = Styles::read(&line[start..core_start]);
        *styles.get_mut(style) = !all_have;
        let (before, after) = styles.marks();
        let core = &line[core_start..core_end];
        changed[index] = format!("{}{before}{core}{after}{}", &line[..start], &line[end..]);

        let core_at = start + before.len();
        if selection.caret {
            // The cursor stays where it was in the word.
            let column = core_at + selection.start.1.clamp(core_start, core_end) - core_start;
            return Changed {
                lines: changed,
                start: (index, column),
                end: (index, column),
            };
        }
        first.get_or_insert((index, core_at));
        last = (index, core_at + core.len());
    }

    Changed {
        lines: changed,
        start: first.unwrap_or(selection.start),
        end: last,
    }
}

/// The word around `column` in `line` (empty if the cursor isn't in one).
fn word_at(line: &str, column: usize) -> (usize, usize) {
    let is_word = |c: char| c.is_alphanumeric() || matches!(c, '\'' | '-');
    let start = line[..column]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(column, |(at, _)| at);
    let end = line[column..]
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(line.len(), |(at, _)| column + at);
    (start, end)
}

/// The piece `from..to` of `line` spread over the marks around it: where its marks start,
/// where the text inside them starts and ends, and where the marks after it end. Marks
/// inside the piece count too, so selecting `**word**` or `word` finds the same marks.
fn spread(line: &str, from: usize, to: usize) -> (usize, usize, usize, usize) {
    let bytes = line.as_bytes();
    let (mut core_start, mut core_end) = (from, to);
    // Marks just inside the piece, while there is text between them.
    while core_end >= core_start + 3
        && is_mark(bytes[core_start])
        && bytes[core_end - 1] == bytes[core_start]
    {
        core_start += 1;
        core_end -= 1;
    }
    let (mut start, mut end) = (core_start, core_end);
    while start > 0
        && end < bytes.len()
        && is_mark(bytes[start - 1])
        && bytes[end] == bytes[start - 1]
    {
        start -= 1;
        end += 1;
    }
    (start, core_start, core_end, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.split('\n').map(str::to_string).collect()
    }

    fn select(start: (usize, usize), end: (usize, usize)) -> Selection {
        Selection {
            start,
            end,
            caret: start == end,
        }
    }

    #[test]
    fn reads_the_marks_at_the_start_of_lines() {
        let line = Line::parse("  - [x] Ask Ann");
        assert_eq!(
            (line.indent, line.mark, line.task),
            ("  ", Mark::Bullet('-'), Some(true))
        );
        assert_eq!(line.text, "Ask Ann");
        assert_eq!(Line::parse("12) Rest").mark, Mark::Numbered(12, ')'));
        assert_eq!(Line::parse("### Gate").mark, Mark::Heading(3));
        assert_eq!(Line::parse("#hashtag").mark, Mark::None);
        assert_eq!(Line::parse(">quote").text, "quote");
        assert_eq!(Line::parse("-").mark, Mark::Bullet('-'));
        assert_eq!(Line::parse("**bold**").mark, Mark::None);
        assert_eq!(Line::parse("2024 was").mark, Mark::None);
    }

    #[test]
    fn sets_the_text_type() {
        let changed = set_text_type(
            &lines("## The inn"),
            select((0, 5), (0, 5)),
            TextType::Heading(1),
        );
        assert_eq!(changed.lines, ["# The inn"]);
        assert_eq!(changed.start, (0, 4));

        let changed = set_text_type(&lines("The inn"), select((0, 0), (0, 0)), TextType::Quote);
        assert_eq!(changed.lines, ["> The inn"]);
        assert_eq!(changed.start, (0, 2));

        let changed = set_text_type(
            &lines("- Rope"),
            select((0, 6), (0, 6)),
            TextType::Paragraph,
        );
        assert_eq!(changed.lines, ["Rope"]);
        assert_eq!(changed.end, (0, 4));
    }

    #[test]
    fn makes_and_unmakes_lists() {
        let text = lines("Rope\n\nTorch\n- Lamp");
        let changed = toggle_list(&text, select((0, 0), (3, 6)), List::Numbered);
        assert_eq!(changed.lines, ["1. Rope", "", "2. Torch", "3. Lamp"]);

        let changed = toggle_list(&changed.lines, select((0, 0), (3, 7)), List::Numbered);
        assert_eq!(changed.lines, ["Rope", "", "Torch", "Lamp"]);

        let changed = toggle_list(
            &lines("* Rope\n  - [x] Torch"),
            select((0, 0), (1, 3)),
            List::Task,
        );
        assert_eq!(changed.lines, ["* [ ] Rope", "  - [x] Torch"]);

        // On an empty line, the mark goes in with the cursor after it.
        let changed = toggle_list(&lines(""), select((0, 0), (0, 0)), List::Bullet);
        assert_eq!(changed.lines, ["- "]);
        assert_eq!(changed.end, (0, 2));
    }

    #[test]
    fn styles_the_selection_or_the_word() {
        let changed = toggle_style(
            &lines("Ask the miller"),
            select((0, 4), (0, 7)),
            Style::Bold,
        );
        assert_eq!(changed.lines, ["Ask **the** miller"]);
        assert_eq!((changed.start, changed.end), ((0, 6), (0, 9)));

        // Again takes it off, whether the marks are selected or not.
        let changed = toggle_style(&changed.lines, select((0, 4), (0, 11)), Style::Bold);
        assert_eq!(changed.lines, ["Ask the miller"]);

        let changed = toggle_style(
            &lines("Ask the miller"),
            select((0, 10), (0, 10)),
            Style::Italic,
        );
        assert_eq!(changed.lines, ["Ask the *miller*"]);
        assert_eq!(changed.end, (0, 11));

        let changed = toggle_style(&lines("**miller**"), select((0, 2), (0, 8)), Style::Italic);
        assert_eq!(changed.lines, ["***miller***"]);
        let changed = toggle_style(&changed.lines, select((0, 3), (0, 9)), Style::Bold);
        assert_eq!(changed.lines, ["*miller*"]);
        let changed = toggle_style(&changed.lines, select((0, 1), (0, 7)), Style::Strikethrough);
        assert_eq!(changed.lines, ["~~*miller*~~"]);

        // With no word, the marks go in with the cursor between them.
        let changed = toggle_style(&lines("Ask "), select((0, 4), (0, 4)), Style::Bold);
        assert_eq!(changed.lines, ["Ask ****"]);
        assert_eq!(changed.end, (0, 6));
        let changed = toggle_style(&changed.lines, select((0, 6), (0, 6)), Style::Bold);
        assert_eq!(changed.lines, ["Ask "]);
    }

    #[test]
    fn styles_each_line_of_a_selection() {
        let changed = toggle_style(
            &lines("- Rope \n- Torch"),
            select((0, 2), (1, 7)),
            Style::Bold,
        );
        assert_eq!(changed.lines, ["- **Rope** ", "- **Torch**"]);
        assert_eq!((changed.start, changed.end), ((0, 4), (1, 9)));
    }

    #[test]
    fn carries_lists_on() {
        let mut content = Content::with_text("1. [x] Sneak in");
        content.move_to(Cursor {
            position: Position {
                line: 0,
                column: 15,
            },
            selection: None,
        });
        assert!(continue_list(&mut content));
        assert_eq!(content.text(), "1. [x] Sneak in\n2. [ ] ");

        // An empty item ends the list.
        assert!(continue_list(&mut content));
        assert_eq!(content.text(), "1. [x] Sneak in\n");

        content.perform(Action::Edit(Edit::Paste(Arc::new("Plain".into()))));
        assert!(!continue_list(&mut content));
    }

    #[test]
    fn applies_to_an_editor() {
        let mut content = Content::with_text("Rope\nTorch");
        content.move_to(Cursor {
            position: Position { line: 1, column: 2 },
            selection: Some(Position { line: 0, column: 1 }),
        });
        apply(&mut content, Command::List(List::Bullet));
        assert_eq!(content.text(), "- Rope\n- Torch");
        assert_eq!(list(&content), Some(List::Bullet));

        apply(&mut content, Command::Text(TextType::Heading(2)));
        assert_eq!(content.text(), "## Rope\n## Torch");
        assert_eq!(text_type(&content), Some(TextType::Heading(2)));
    }
}
