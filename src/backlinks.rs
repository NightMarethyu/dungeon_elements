//! Backlinks: the notes that link to a note, so the reading view can list them under it
//! ("Linked from") the way a wiki does, and an NPC shows the quests, places and journal
//! entries they turn up in.
//!
//! A link counts however it names the note, the same as following it does
//! ([`Index::resolve`]): by file name or path, by an alias, with shown text
//! (`[[BBEG|Jerry]]`), to a heading, or as an embed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_yaml_ng::Mapping;

use crate::links::{self, Index};
use crate::note;

/// The most text shown from the line a link is on.
const LINE_LENGTH: usize = 160;

/// Where in a note a link is, to show beside the note in the list of backlinks.
#[derive(Debug, Clone, PartialEq)]
pub enum Mention {
    /// In the front matter field `key`, whose value reads `value` (links as their text).
    Field { key: String, value: String },
    /// In the body, on a line that reads like this (links as their text, cut short).
    Line(String),
}

/// The links in a whole note, in order: the note each names, and where it is.
pub fn mentions(text: &str) -> Vec<(String, Mention)> {
    let yaml_end = match note::split_front_matter(text) {
        Ok((Some(_), body)) => text.len() - body.len(),
        _ => 0,
    };
    let front: Option<Mapping> = match note::split_front_matter(text) {
        Ok((Some(yaml), _)) => serde_yaml_ng::from_str(yaml).ok(),
        _ => None,
    };

    let targets = links::targets(text);
    links::find(text)
        .iter()
        .zip(targets)
        .map(|(link, target)| {
            let mention = if link.range.start < yaml_end {
                let key = field_at(&text[..link.range.start]);
                let value = front
                    .as_ref()
                    .and_then(|front| front.get(key.as_str()))
                    .map(|value| shown(&note::value_to_text(value)))
                    .unwrap_or_default();
                Mention::Field { key, value }
            } else {
                Mention::Line(line_at(text, link.range.start))
            };
            (target, mention)
        })
        .collect()
}

/// The top-level front matter key whose value goes on at the end of `before`.
fn field_at(before: &str) -> String {
    before
        .lines()
        .rev()
        .find(|line| !line.starts_with([' ', '\t', '-', '#']) && line.contains(':'))
        .and_then(|line| line.split_once(':'))
        .map(|(key, _)| key.trim().trim_matches(['"', '\'']).to_string())
        .unwrap_or_default()
}

/// The line of `text` that byte `at` is on, as it reads: without list, quote or heading
/// marks, with links as their text, and cut to about [`LINE_LENGTH`] letters around `at`.
fn line_at(text: &str, at: usize) -> String {
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let end = text[at..].find('\n').map_or(text.len(), |i| at + i);
    let line = &text[start..end];

    // Cut long lines (a paragraph is one line) to the part around the link.
    let at = at - start;
    let before: Vec<(usize, char)> = line[..at].char_indices().collect();
    let from = if line.chars().count() > LINE_LENGTH && before.len() > LINE_LENGTH / 3 {
        // Start at a word, a third of the length before the link.
        let cut = before[before.len() - LINE_LENGTH / 3].0;
        line[cut..].find(' ').map_or(cut, |space| cut + space + 1)
    } else {
        0
    };
    let mut part = shown(trim_marks(&line[from..]));
    if part.chars().count() > LINE_LENGTH {
        let cut = part
            .char_indices()
            .nth(LINE_LENGTH)
            .map_or(part.len(), |(i, _)| i);
        let cut = part[..cut].rfind(' ').unwrap_or(cut);
        part.truncate(cut);
        part = format!("{}…", part.trim_end());
    }
    if from > 0 {
        part = format!("…{part}");
    }
    part
}

/// `line` without the Markdown marks at its start: `#`s, `>`, list bullets and numbers,
/// and task boxes.
fn trim_marks(line: &str) -> &str {
    let mut line = line.trim();
    loop {
        let before = line;
        line = line
            .trim_start_matches('#')
            .trim_start_matches('>')
            .trim_start();
        for bullet in ["- ", "* ", "+ ", "[ ] ", "[x] ", "[X] "] {
            line = line.strip_prefix(bullet).unwrap_or(line).trim_start();
        }
        let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits > 0
            && let Some(rest) = line[digits..]
                .strip_prefix(". ")
                .or_else(|| line[digits..].strip_prefix(") "))
        {
            line = rest.trim_start();
        }
        if line == before {
            return line;
        }
    }
}

/// `text` with each `[[link]]` as the text it shows: `[[BBEG|Jerry]]` reads "Jerry",
/// `[[Keep#Gate]]` "Keep > Gate".
fn shown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for link in links::find(text) {
        out.push_str(&text[last..link.range.start]);
        let inner = text[link.range.clone()]
            .trim_start_matches('!')
            .trim_start_matches("[[")
            .trim_end_matches("]]");
        match inner.split_once('|') {
            Some((_, alias)) => out.push_str(alias.trim()),
            None => out.push_str(&inner.trim().replace('#', " > ")),
        }
        last = link.range.end;
    }
    out.push_str(&text[last..]);
    out
}

/// What each note in the campaign links to, to look up the notes that link to one.
#[derive(Debug, Clone, Default)]
pub struct Backlinks {
    /// Each note's links to other notes: the note linked to, and where the link is.
    links: HashMap<PathBuf, Vec<(PathBuf, Mention)>>,
}

impl Backlinks {
    /// The backlinks between the notes in `notes` (each with its [`mentions`]).
    pub fn new(notes: HashMap<PathBuf, Vec<(String, Mention)>>, index: &Index) -> Self {
        let mut backlinks = Backlinks::default();
        for (path, mentions) in notes {
            backlinks.set(path, mentions, index);
        }
        backlinks
    }

    /// Sets what the note at `path` links to, after it was edited.
    pub fn set(&mut self, path: PathBuf, mentions: Vec<(String, Mention)>, index: &Index) {
        let resolved: Vec<(PathBuf, Mention)> = mentions
            .into_iter()
            .filter_map(|(target, mention)| {
                let to = index.resolve(&target, &path)?;
                // A link to a heading in the same note isn't a backlink.
                (to != path).then(|| (to.to_path_buf(), mention))
            })
            .collect();
        if resolved.is_empty() {
            self.links.remove(&path);
        } else {
            self.links.insert(path, resolved);
        }
    }

    /// The notes that link to the note at `path`, in order of their paths, each with
    /// where its links are (each place once).
    pub fn to(&self, path: &Path) -> Vec<(&Path, Vec<&Mention>)> {
        let mut found: Vec<(&Path, Vec<&Mention>)> = self
            .links
            .iter()
            .filter_map(|(from, links)| {
                let mut mentions: Vec<&Mention> = Vec::new();
                for (to, mention) in links {
                    if to == path && !mentions.contains(&mention) {
                        mentions.push(mention);
                    }
                }
                (!mentions.is_empty()).then_some((from.as_path(), mentions))
            })
            .collect();
        found.sort_by(|a, b| a.0.cmp(b.0));
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> Mention {
        Mention::Line(text.into())
    }

    #[test]
    fn finds_mentions_in_fields_and_lines() {
        let text = "---\nname: The Lost Ring\ngiver: '[[Mira]]'\nallies:\n  - '[[BBEG|Jerry]]'\n  - \"[[Bob]]\"\n---\n\n## Steps\n\n- [ ] Ask [[Mira#Past|Mira]] about the ring.\n> Seen near ![[Old Mill]].\n";
        let found = mentions(text);
        assert_eq!(
            found,
            [
                (
                    "Mira".to_string(),
                    Mention::Field {
                        key: "giver".into(),
                        value: "Mira".into()
                    }
                ),
                (
                    "BBEG".to_string(),
                    Mention::Field {
                        key: "allies".into(),
                        value: "Jerry, Bob".into()
                    }
                ),
                (
                    "Bob".to_string(),
                    Mention::Field {
                        key: "allies".into(),
                        value: "Jerry, Bob".into()
                    }
                ),
                ("Mira".to_string(), line("Ask Mira about the ring.")),
                ("Old Mill".to_string(), line("Seen near Old Mill.")),
            ]
        );
    }

    #[test]
    fn reads_yaml_quotes_in_fields() {
        let text = "---\nowner: '[[John''s Shop]]'\n---\n";
        assert_eq!(mentions(text)[0].0, "John's Shop");
        assert_eq!(shown("[[Keep#Gate]] and [[A|B]]"), "Keep > Gate and B");
    }

    #[test]
    fn cuts_long_lines_around_the_link() {
        let words = "word ".repeat(60);
        let text = format!("{words}[[Mira]] {words}");
        let Mention::Line(shown) = &mentions(&text)[0].1 else {
            panic!("not a line");
        };
        assert!(shown.starts_with('…') && shown.ends_with('…'));
        assert!(shown.contains("word Mira word"));
        assert!(shown.chars().count() <= LINE_LENGTH + 2);
    }

    #[test]
    fn counts_links_by_name_path_and_alias() {
        let root = Path::new("/c");
        let bbeg = root.join("npcs/BBEG.md");
        let mira = root.join("npcs/Mira.md");
        let quest = root.join("quests/Ring.md");
        let journal = root.join("journal/Day 1.md");
        let index = Index::new(
            root,
            [bbeg.clone(), mira.clone(), quest.clone(), journal.clone()],
        )
        .with_aliases(&HashMap::from([(bbeg.clone(), vec!["Jerry".to_string()])]));

        let notes = HashMap::from([
            (
                quest.clone(),
                mentions("Beat [[BBEG|Jerry]] with [[npcs/Mira]]."),
            ),
            (
                journal.clone(),
                mentions("Met [[Jerry]]. Then [[jerry]] again.\n[[Nobody]]"),
            ),
            // Links to its own headings don't count.
            (bbeg.clone(), mentions("See [[#Plans]] and [[BBEG#Plans]].")),
        ]);
        let backlinks = Backlinks::new(notes, &index);

        let to_bbeg = backlinks.to(&bbeg);
        assert_eq!(
            to_bbeg,
            [
                (
                    journal.as_path(),
                    vec![&line("Met Jerry. Then jerry again.")]
                ),
                (quest.as_path(), vec![&line("Beat Jerry with npcs/Mira.")]),
            ]
        );
        assert_eq!(backlinks.to(&mira).len(), 1);
        assert!(backlinks.to(&quest).is_empty());
    }

    #[test]
    fn follows_edits() {
        let root = Path::new("/c");
        let (a, b) = (root.join("A.md"), root.join("B.md"));
        let index = Index::new(root, [a.clone(), b.clone()]);
        let mut backlinks = Backlinks::new(HashMap::from([(a.clone(), mentions("[[B]]"))]), &index);
        assert_eq!(backlinks.to(&b).len(), 1);

        backlinks.set(a.clone(), mentions("no links now"), &index);
        assert!(backlinks.to(&b).is_empty());
    }
}
