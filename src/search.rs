//! Searching the campaign's notes from the sidebar's search box.
//!
//! Words are looked for in each note's name and text. A word like `status:alive` is a
//! filter instead: the note's field (by its key or label) must hold that value. Notes
//! are read as they are each time, with no index, which is fast enough for a campaign.

use std::path::PathBuf;

use serde_yaml_ng::Value;

use crate::note;

/// What was typed in the search box.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Query {
    /// Lowercase words (or quoted phrases) the note's name or text must hold.
    pub words: Vec<String>,
    /// `(field, value)` pairs, both lowercase: the field must hold the value.
    pub filters: Vec<(String, String)>,
}

impl Query {
    /// Splits `text` into words and `field:value` filters. Double quotes keep spaces in
    /// a phrase or a value, like `location:"Salt Marsh"`. A filter with no value yet is
    /// left out, so results don't vanish while it is being typed.
    pub fn parse(text: &str) -> Self {
        let mut query = Query::default();
        for token in tokens(text) {
            match split_filter(&token) {
                Some((field, value)) => {
                    let field = field.trim_matches('"');
                    let value = value.trim_matches('"').to_lowercase();
                    if !value.is_empty() {
                        query.filters.push((field.to_lowercase(), value));
                    }
                }
                _ => {
                    let word = token.trim_matches('"').to_lowercase();
                    if !word.is_empty() {
                        query.words.push(word);
                    }
                }
            }
        }
        query
    }
}

/// A token's `field` and `value`, split at its first colon outside quotes.
fn split_filter(token: &str) -> Option<(&str, &str)> {
    let mut quoted = false;
    for (at, c) in token.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted && at > 0 => return Some((&token[..at], &token[at + 1..])),
            _ => {}
        }
    }
    None
}

/// The words of `text`, keeping what is inside double quotes together.
fn tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                current.push(c);
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// A note the search found.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub path: PathBuf,
    /// 0 when every word is in the note's name, 1 when some are only in its text, so
    /// notes named for what was typed come first.
    pub rank: u8,
    /// The line that matched, to show under the note's name.
    pub line: Option<String>,
}

/// Whether the note `text` matches `query`, and how.
///
/// `names` are what the note is called besides its aliases (its file name, and its name
/// field when that differs). `fields` are its folder type's `(key, label)` pairs, so a
/// filter can name a field by either; a field with no type matches by its key.
pub fn find(
    query: &Query,
    path: PathBuf,
    text: &str,
    names: &[String],
    fields: &[(String, String)],
) -> Option<Hit> {
    let parsed = note::parse(text).ok();
    let mut names: Vec<String> = names.iter().map(|name| name.to_lowercase()).collect();
    if let Some(note) = &parsed {
        match note.front.get("aliases") {
            Some(Value::Sequence(items)) => names.extend(
                items
                    .iter()
                    .map(|alias| note::value_to_text(alias).to_lowercase()),
            ),
            Some(Value::String(alias)) => names.push(alias.to_lowercase()),
            _ => {}
        }
    }

    let mut line = None;
    for (field, value) in &query.filters {
        // A word with a colon that names no field, like `10:30`, is looked for as text.
        let Some(found) = filter(parsed.as_ref(), field, value, fields) else {
            if !text.to_lowercase().contains(&format!("{field}:{value}")) {
                return None;
            }
            continue;
        };
        line.get_or_insert(found?);
    }

    let lower = text.to_lowercase();
    let in_name = |word: &String| names.iter().any(|name| name.contains(word.as_str()));
    if !query
        .words
        .iter()
        .all(|word| in_name(word) || lower.contains(word.as_str()))
    {
        return None;
    }
    let rank = u8::from(!query.words.iter().all(in_name));
    if rank == 1 {
        line = matching_line(text, &query.words)
            .or_else(|| matching_field(parsed.as_ref()?, &query.words, fields))
            .or(line);
    }
    Some(Hit { path, rank, line })
}

/// Whether filter `field:value` matches the note: `None` if the note has no such field
/// in its type or front matter, otherwise `Some` of the matching `Label: value` line, or
/// `Some(None)` when it doesn't match.
fn filter(
    note: Option<&note::Note>,
    field: &str,
    value: &str,
    fields: &[(String, String)],
) -> Option<Option<String>> {
    let wanted = squash(field);
    let typed = fields
        .iter()
        .find(|(key, label)| squash(key) == wanted || squash(label) == wanted);
    let note = note?;

    // A long text field is a `## Label` section of the note's text.
    let section = note.sections.iter().find(|section| {
        squash(&section.heading) == wanted
            || typed.is_some_and(|(_, label)| squash(label) == squash(&section.heading))
    });
    let front = note.front.iter().find_map(|(key, held)| {
        let key = key.as_str()?;
        (squash(key) == wanted || typed.is_some_and(|(typed, _)| typed == key))
            .then_some((key, held))
    });

    if front.is_none() && section.is_none() && typed.is_none() {
        return None;
    }
    let label = |key: &str| {
        typed
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| key.to_string())
    };
    if let Some((key, held)) = front
        && holds(held, value)
    {
        return Some(Some(format!("{}: {}", label(key), shown(held))));
    }
    if let Some(section) = section
        && section.body.to_lowercase().contains(value)
    {
        return Some(matching_line(&section.body, &[value.to_string()]));
    }
    Some(None)
}

/// Whether the front matter value `held` holds `value`: any item of a list, the note a
/// link points to, or yes and no for a checkbox.
fn holds(held: &Value, value: &str) -> bool {
    match held {
        Value::Sequence(items) => items.iter().any(|item| holds(item, value)),
        Value::Bool(checked) => match value {
            "yes" | "true" | "on" | "x" => *checked,
            "no" | "false" | "off" => !*checked,
            _ => false,
        },
        held => shown(held).to_lowercase().contains(value),
    }
}

/// The first field in the note's front matter holding one of `words`, as `Label: value`.
fn matching_field(
    note: &note::Note,
    words: &[String],
    fields: &[(String, String)],
) -> Option<String> {
    note.front.iter().find_map(|(key, value)| {
        let key = key.as_str()?;
        let value = shown(value);
        let lower = value.to_lowercase();
        words
            .iter()
            .any(|word| lower.contains(word.as_str()))
            .then(|| {
                let label = fields
                    .iter()
                    .find(|(field, _)| field == key)
                    .map_or(key, |(_, label)| label.as_str());
                format!("{label}: {value}")
            })
    })
}

/// A front matter value as the search shows it, links without their brackets.
fn shown(value: &Value) -> String {
    note::value_to_text(value)
        .replace("[[", "")
        .replace("]]", "")
}

/// A field name with case, spaces, `_` and `-` taken out, so `Known to players`,
/// `known_to_players` and `knowntoplayers` are the same field.
fn squash(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, ' ' | '_' | '-'))
        .flat_map(char::to_lowercase)
        .collect()
}

/// The first line of the note's text (not its front matter) holding one of `words`,
/// trimmed and shortened to fit under a name in the sidebar.
fn matching_line(text: &str, words: &[String]) -> Option<String> {
    let body = note::split_front_matter(text)
        .map(|(_, body)| body)
        .unwrap_or(text);
    let line = body.lines().find(|line| {
        let line = line.to_lowercase();
        words.iter().any(|word| line.contains(word.as_str()))
    })?;
    let line = line.trim().trim_start_matches(['#', '-', '*', '>', ' ']);
    const MAX: usize = 80;
    Some(if line.chars().count() > MAX {
        let short: String = line.chars().take(MAX).collect();
        format!("{}…", short.trim_end())
    } else {
        line.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRUK: &str = "---\ntype: npc\nname: Gruk\naliases:\n- The Butcher\nstatus: Alive\n\
        location: '[[Saltmarsh]]'\nfactions:\n- '[[Sea Princes]]'\n- '[[Thieves Guild]]'\n\
        known_to_players: false\n---\n\nA half-orc cook.\n\n## Secrets\n\nWorks for the \
        smugglers of the Salt Marsh.\n";

    fn fields() -> Vec<(String, String)> {
        [
            ("status", "Status"),
            ("location", "Location"),
            ("factions", "Factions"),
            ("known_to_players", "Known to players"),
            ("secrets", "Secrets"),
        ]
        .iter()
        .map(|(key, label)| (key.to_string(), label.to_string()))
        .collect()
    }

    fn search(query: &str) -> Option<Hit> {
        find(
            &Query::parse(query),
            PathBuf::from("npcs/Gruk.md"),
            GRUK,
            &["Gruk".into()],
            &fields(),
        )
    }

    #[test]
    fn parses_words_filters_and_quotes() {
        let query = Query::parse(r#"Cook  status:Alive location:"Salt Marsh" "half orc" age:"#);
        assert_eq!(query.words, ["cook", "half orc"]);
        assert_eq!(
            query.filters,
            [
                ("status".to_string(), "alive".to_string()),
                ("location".to_string(), "salt marsh".to_string())
            ]
        );
        assert_eq!(Query::parse("   "), Query::default());
    }

    #[test]
    fn names_come_before_text() {
        assert_eq!(search("gruk").unwrap().rank, 0);
        // An alias is a name too.
        assert_eq!(search("butcher").unwrap().rank, 0);
        let hit = search("cook").unwrap();
        assert_eq!(hit.rank, 1);
        assert_eq!(hit.line.as_deref(), Some("A half-orc cook."));
        assert!(search("gruk elf").is_none());
        // A word only in a field shows the field.
        assert_eq!(
            search("thieves").unwrap().line.as_deref(),
            Some("Factions: Sea Princes, Thieves Guild")
        );
    }

    #[test]
    fn filters_by_fields() {
        assert_eq!(
            search("status:alive").unwrap().line.as_deref(),
            Some("Status: Alive")
        );
        assert!(search("status:dead").is_none());
        // A link field matches the note it points to; a list matches any of its items.
        assert!(search("location:saltmarsh").is_some());
        assert!(search("factions:thieves").is_some());
        assert!(search("factions:pirates").is_none());
        // Fields can go by their label, and checkboxes by yes and no.
        assert!(search(r#""known to players":no"#).is_some());
        assert!(search("knowntoplayers:no").is_some());
        assert!(search("known_to_players:yes").is_none());
        // A long text field is the section under its heading.
        assert_eq!(
            search("secrets:smugglers").unwrap().line.as_deref(),
            Some("Works for the smugglers of the Salt Marsh.")
        );
        // Words and filters go together.
        assert!(search("cook status:alive").is_some());
        assert!(search("elf status:alive").is_none());
    }

    #[test]
    fn colon_words_without_a_field_are_text() {
        let text = "Meet at 10:30 by the docks.";
        let hit = |query: &str| find(&Query::parse(query), PathBuf::from("a.md"), text, &[], &[]);
        assert!(hit("10:30").is_some());
        assert!(hit("11:30").is_none());
    }
}
