//! Notes on disk: Markdown with optional YAML front matter, split into `## ` sections.
//!
//! This is the same layout Obsidian uses, so notes stay readable and editable there.

use serde_yaml_ng::{Mapping, Value};

/// A note split into the parts a form needs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Note {
    /// The keys in the front matter, in the order they appear.
    pub front: Mapping,
    /// The body text before the first `## ` heading.
    pub preamble: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub heading: String,
    pub body: String,
    /// Whether to write the heading even when the body is empty.
    pub keep_empty: bool,
}

/// Splits `text` into front matter and sections.
///
/// Fails if the front matter is not closed or is not valid YAML keys and values.
pub fn parse(text: &str) -> Result<Note, String> {
    let (front, body) = split_front_matter(text)?;

    let front = match front {
        None => Mapping::new(),
        Some(yaml) => match serde_yaml_ng::from_str::<Value>(yaml) {
            Ok(Value::Null) => Mapping::new(),
            Ok(Value::Mapping(mapping)) => mapping,
            Ok(_) => return Err("the front matter is not a list of keys and values".into()),
            Err(error) => return Err(format!("the front matter is not valid YAML: {error}")),
        },
    };

    let mut preamble = Vec::new();
    let mut sections: Vec<(String, Vec<&str>)> = Vec::new();
    let mut in_code_block = false;

    for line in body.lines() {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            in_code_block = !in_code_block;
        }

        match line.strip_prefix("## ") {
            Some(heading) if !in_code_block => {
                sections.push((heading.trim().to_string(), Vec::new()))
            }
            _ => match sections.last_mut() {
                Some((_, lines)) => lines.push(line),
                None => preamble.push(line),
            },
        }
    }

    Ok(Note {
        front,
        preamble: trim_blank_lines(&preamble),
        sections: sections
            .into_iter()
            .map(|(heading, lines)| Section {
                heading,
                body: trim_blank_lines(&lines),
                keep_empty: false,
            })
            .collect(),
    })
}

/// Returns the front matter's YAML (if the text starts with `---`) and the text after it.
fn split_front_matter(text: &str) -> Result<(Option<&str>, &str), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);

    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return Ok((None, text));
    };

    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" || trimmed == "..." {
            return Ok((Some(&rest[..offset]), &rest[offset + line.len()..]));
        }
        offset += line.len();
    }

    Err("the front matter has no closing `---` line".into())
}

/// Joins `lines`, dropping blank lines at the start and end.
fn trim_blank_lines(lines: &[&str]) -> String {
    let start = lines.iter().position(|line| !line.trim().is_empty());
    let end = lines.iter().rposition(|line| !line.trim().is_empty());

    match (start, end) {
        (Some(start), Some(end)) => lines[start..=end].join("\n"),
        _ => String::new(),
    }
}

/// Writes a note: front matter, then `rest` as it is, then each section that has text
/// (or is marked [`Section::keep_empty`]).
///
/// `rest` comes first because text after the last section would become part of it.
pub fn to_markdown(front: &Mapping, sections: &[Section], rest: &str) -> Result<String, String> {
    let mut text = String::new();

    if !front.is_empty() {
        let yaml = serde_yaml_ng::to_string(front).map_err(|error| error.to_string())?;
        text.push_str("---\n");
        text.push_str(&yaml);
        if !yaml.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("---\n");
    }

    let rest = rest.trim();
    if !rest.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(rest);
        text.push('\n');
    }

    for section in sections {
        let body = section.body.trim_end();
        if body.trim().is_empty() && !section.keep_empty {
            continue;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        if body.trim().is_empty() {
            text.push_str(&format!("## {}\n", section.heading));
        } else {
            text.push_str(&format!("## {}\n\n{body}\n", section.heading));
        }
    }

    Ok(text)
}

/// Shows a front matter value as text: lists are joined with commas.
pub fn value_to_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Sequence(items) => items
            .iter()
            .map(value_to_text)
            .collect::<Vec<_>>()
            .join(", "),
        Value::Mapping(_) | Value::Tagged(_) => serde_yaml_ng::to_string(value)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

/// Removes the `[[` and `]]` around an Obsidian link.
pub fn unlink(text: &str) -> &str {
    let text = text.trim();
    text.strip_prefix("[[")
        .and_then(|inner| inner.strip_suffix("]]"))
        .unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RING: &str = "---\ntype: item\nname: The One Ring\nowner: '[[BBEG]]'\n---\n\nAn intro.\n\n## History\n\nForged in fire.\n\nLost for ages.\n\n## Stats\n\n```\n## not a heading\n```\n";

    #[test]
    fn parses_front_matter_and_sections() {
        let note = parse(RING).unwrap();

        assert_eq!(note.front.get("name"), Some(&Value::from("The One Ring")));
        assert_eq!(note.preamble, "An intro.");
        assert_eq!(note.sections.len(), 2);
        assert_eq!(note.sections[0].heading, "History");
        assert_eq!(note.sections[0].body, "Forged in fire.\n\nLost for ages.");
        assert_eq!(note.sections[1].body, "```\n## not a heading\n```");
    }

    #[test]
    fn parses_plain_markdown() {
        let note = parse("# Title\n\nJust text.\n").unwrap();
        assert!(note.front.is_empty());
        assert_eq!(note.preamble, "# Title\n\nJust text.");

        let note = parse("---\r\n---\r\nBody\r\n").unwrap();
        assert!(note.front.is_empty());
        assert_eq!(note.preamble, "Body");
    }

    #[test]
    fn rejects_broken_front_matter() {
        assert!(parse("---\nname: x\n").is_err());
        assert!(parse("---\nname: [x\n---\n").is_err());
        assert!(parse("---\n- a list\n---\n").is_err());
    }

    #[test]
    fn writes_what_it_reads() {
        let note = parse(RING).unwrap();
        let text = to_markdown(&note.front, &note.sections, &note.preamble).unwrap();
        let again = parse(&text).unwrap();

        assert_eq!(again, note);
        assert!(text.starts_with("---\ntype: item\n"));
    }

    #[test]
    fn skips_empty_sections() {
        let sections = [
            Section {
                heading: "History".into(),
                body: "  \n".into(),
                keep_empty: false,
            },
            Section {
                heading: "Stats".into(),
                body: "+1".into(),
                keep_empty: false,
            },
            Section {
                heading: "Durability".into(),
                body: String::new(),
                keep_empty: true,
            },
        ];
        let text = to_markdown(&Mapping::new(), &sections, "").unwrap();
        assert_eq!(text, "## Stats\n\n+1\n\n## Durability\n");
    }

    #[test]
    fn converts_values() {
        assert_eq!(value_to_text(&Value::from(3)), "3");
        assert_eq!(
            value_to_text(&Value::Sequence(vec!["a".into(), "b".into()])),
            "a, b"
        );
        assert_eq!(unlink(" [[BBEG]] "), "BBEG");
        assert_eq!(unlink("BBEG"), "BBEG");
    }
}
