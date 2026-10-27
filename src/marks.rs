//! Finding the Markdown marks in a line, like `#`, `**`, `- [ ]` and `](url)`, so the
//! editors can draw them dimmer than the words and the words stand out.
//!
//! The marks at the start of a line (quotes, headings, list items and task boxes) are
//! read here; the rest of the line is left to pulldown-cmark, and whatever it doesn't
//! call text is a mark. Emphasis that spans lines isn't seen, as each line is read alone.

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser};

/// Where the marks are in one line of a note's text (not front matter or code), in order.
pub fn find(line: &str) -> Vec<Range<usize>> {
    let end = line.trim_end_matches(['\n', '\r']).len();
    let line = &line[..end];
    let mut marks = Vec::new();
    let mut i = 0;

    // Quotes, which can hold any of the rest.
    loop {
        i = skip_spaces(line, i);
        if line[i..].starts_with('>') {
            marks.push(i..i + 1);
            i += 1;
        } else {
            break;
        }
    }

    let rest = line[i..].trim_end();
    if is_rule(rest) {
        marks.push(i..i + rest.len());
        return marks;
    }

    if let Some(length) = heading(rest) {
        marks.push(i..i + length);
        i = skip_spaces(line, i + length);
    } else if let Some(length) = list_marker(rest) {
        marks.push(i..i + length);
        i = skip_spaces(line, i + length);
        if is_task_box(&line[i..]) {
            marks.push(i..i + 3);
            i = skip_spaces(line, i + 3);
        }
    }

    let table = line[i..].starts_with('|');
    if table && is_table_divider(&line[i..]) {
        marks.push(i..end);
        return marks;
    }

    // Everything between the words is a mark.
    let mut at = i;
    for range in words(&line[i..]) {
        let range = i + range.start..i + range.end;
        if range.start > at && !line[at..range.start].trim().is_empty() {
            marks.push(at..range.start);
        }
        at = at.max(range.end);
    }
    if at < end && !line[at..].trim().is_empty() {
        marks.push(at..end);
    }

    if table {
        marks.extend(table_pipes(line, i));
        marks.sort_by_key(|range| range.start);
    }

    marks
}

fn skip_spaces(line: &str, from: usize) -> usize {
    from + (line[from..].len() - line[from..].trim_start_matches([' ', '\t']).len())
}

/// The ranges of `text` that are words rather than marks: its text and the insides of
/// its code spans.
fn words(text: &str) -> Vec<Range<usize>> {
    let options = Options::ENABLE_STRIKETHROUGH;
    Parser::new_ext(text, options)
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Text(_) => Some(range),
            Event::Code(_) => {
                let code = &text[range.clone()];
                let start = code.len() - code.trim_start_matches('`').len();
                let end = code.trim_end_matches('`').len().max(start);
                Some(range.start + start..range.start + end)
            }
            _ => None,
        })
        .collect()
}

/// Whether the line is a rule (`---`, `***`, `___`) or underlines a heading (`===`).
fn is_rule(line: &str) -> bool {
    let Some(first) = line.chars().next() else {
        return false;
    };
    if first == '=' {
        return line.chars().all(|c| c == '=');
    }
    matches!(first, '-' | '*' | '_')
        && line.chars().all(|c| c == first || c == ' ' || c == '\t')
        && line.chars().filter(|&c| c == first).count() >= 3
}

/// How long the `#`s of a heading are, with the space after them.
fn heading(line: &str) -> Option<usize> {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    let after = &line[hashes..];
    ((1..=6).contains(&hashes) && (after.is_empty() || after.starts_with([' ', '\t'])))
        .then_some(hashes)
}

/// How long a list item's `-`, `*`, `+`, `1.` or `1)` is.
fn list_marker(line: &str) -> Option<usize> {
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    let length = match line.as_bytes().get(digits) {
        Some(b'.' | b')') if (1..=9).contains(&digits) => digits + 1,
        Some(b'-' | b'*' | b'+') if digits == 0 => 1,
        _ => return None,
    };
    let after = &line[length..];
    (after.is_empty() || after.starts_with([' ', '\t'])).then_some(length)
}

fn is_task_box(line: &str) -> bool {
    matches!(line.get(..3), Some("[ ]" | "[x]" | "[X]"))
        && line[3..]
            .chars()
            .next()
            .is_none_or(|c| c == ' ' || c == '\t')
}

/// Whether a table row is the one under the header, like `| --- | :-: |`.
fn is_table_divider(line: &str) -> bool {
    line.contains('-')
        && line
            .trim()
            .chars()
            .all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// The `|`s between a table row's cells, skipping escaped ones and code spans.
fn table_pipes(line: &str, from: usize) -> Vec<Range<usize>> {
    let bytes = line.as_bytes();
    let mut pipes = Vec::new();
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'`' => {
                let run = bytes[i..].iter().take_while(|&&b| b == b'`').count();
                let fence = &line[i..i + run];
                i += run;
                if let Some(end) = line[i..].find(fence) {
                    i += end + run;
                }
            }
            b'|' => {
                pipes.push(i..i + 1);
                i += 1;
            }
            _ => i += 1,
        }
    }
    pipes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The marks in `line`, as text.
    fn marks(line: &str) -> Vec<&str> {
        find(line).into_iter().map(|range| &line[range]).collect()
    }

    #[test]
    fn plain_text_has_no_marks() {
        assert!(marks("Just some words, and a 3.5 gp price.").is_empty());
        assert!(marks("").is_empty());
        assert!(marks("#hashtag is not a heading").is_empty());
    }

    #[test]
    fn headings_quotes_and_rules() {
        assert_eq!(marks("## The Sunken Keep"), ["##"]);
        assert_eq!(marks("> > Quoted twice"), [">", ">"]);
        assert_eq!(marks("> # Heading in a quote"), [">", "#"]);
        assert_eq!(marks("---"), ["---"]);
        assert_eq!(marks("* * *"), ["* * *"]);
        assert_eq!(marks("==="), ["==="]);
    }

    #[test]
    fn lists_and_task_boxes() {
        assert_eq!(marks("- Rope"), ["-"]);
        assert_eq!(marks("12. Twelfth"), ["12."]);
        assert_eq!(marks("  - [ ] Find the key"), ["-", "[ ]"]);
        assert_eq!(marks("1. [x] Done"), ["1.", "[x]"]);
        assert_eq!(marks("> - [X] Quoted task"), [">", "-", "[X]"]);
        // Not a list without the space.
        assert!(marks("-5 hit points").is_empty());
    }

    #[test]
    fn emphasis_code_and_links() {
        assert_eq!(marks("A **bold** word"), ["**", "**"]);
        assert_eq!(marks("*Italic* and ~~struck~~"), ["*", "*", "~~", "~~"]);
        assert_eq!(marks("- **Bold** item"), ["-", "**", "**"]);
        assert_eq!(marks("Roll `2d6` now"), ["`", "`"]);
        assert_eq!(
            marks("See [the map](maps/keep.png)."),
            ["[", "](maps/keep.png)"]
        );
        assert_eq!(marks("![Keep](keep.png)"), ["![", "](keep.png)"]);
        assert_eq!(marks(r"Not \*italic\*"), [r"\", r"\"]);
        // Stars that don't make emphasis are words.
        assert!(marks("5 * 3 = 15").is_empty());
    }

    #[test]
    fn tables() {
        assert_eq!(marks("| Name | HP |"), ["|", "|", "|"]);
        assert_eq!(marks("| --- | :-: |"), ["| --- | :-: |"]);
        assert_eq!(
            marks("| `a|b` | **c** |"),
            ["|", "`", "`", "|", "**", "**", "|"]
        );
    }

    #[test]
    fn ranges_are_in_order_and_skip_line_ends() {
        let line = "# **Title**\r\n";
        let ranges = find(line);
        assert!(ranges.windows(2).all(|w| w[0].end <= w[1].start));
        assert_eq!(marks(line), ["#", "**", "**"]);
    }
}
