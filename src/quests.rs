//! The Quests list in the sidebar: the campaign's quests grouped by status, each with its
//! progress and steps, and quest lines (a quest whose steps are other quests) as a tree.
//!
//! The list keeps its own state and draws itself at whatever size it is given, so it can
//! move to another pane later. Quests are read again from their notes whenever the
//! campaign changes, so the notes stay the truth.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::widget::{button, column, container, mouse_area, row, space, text};
use iced::{Center, Element, Fill, FillPortion, Padding, mouse::Interaction};

use crate::links;
use crate::note;
use crate::style;
use crate::theme::Colors;
use crate::types::{ContentType, Kind};

/// The type id of quest notes. Kinds of quests count too.
pub const QUEST: &str = "quest";

/// Statuses whose quests are over, so their groups start collapsed.
const FINISHED: [&str; 3] = ["complete", "failed", "abandoned"];

/// One quest note.
#[derive(Debug, Clone, PartialEq)]
pub struct Quest {
    pub path: PathBuf,
    pub name: String,
    pub status: Option<String>,
    pub steps: Vec<Step>,
}

/// One step of a quest's steps fields.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// The step as shown, with its links' brackets taken off.
    pub text: String,
    pub done: bool,
    /// Where the `[ ]` of its task box is in the note's text, if it has one.
    pub at: Option<usize>,
    /// The note the step's first link points to.
    pub link: Option<PathBuf>,
}

impl Quest {
    /// Whether the quest's status is Complete, which counts it as done in the quests it
    /// is a step of.
    fn complete(&self) -> bool {
        self.status
            .as_deref()
            .is_some_and(|status| status.eq_ignore_ascii_case("complete"))
    }
}

/// Reads the quest at `path` from its `text`. Its steps are the lines of the `## `
/// sections of the type's steps fields.
pub fn read(
    path: PathBuf,
    name: String,
    text: &str,
    ty: &ContentType,
    index: &links::Index,
) -> Quest {
    let status = note::parse(text)
        .ok()
        .and_then(|note| note.front.get("status").map(note::value_to_text))
        .map(|status| status.trim().to_string())
        .filter(|status| !status.is_empty());
    let headings: Vec<String> = ty
        .fields
        .iter()
        .filter(|field| field.kind == Kind::Steps)
        .map(|field| field.label())
        .collect();
    let steps = steps(text, &headings)
        .into_iter()
        .map(|(line, done, at)| {
            let link = links::find(line)
                .first()
                .and_then(|link| index.resolve(&line[link.target.clone()], &path))
                .map(Path::to_path_buf);
            Step {
                text: shown(line),
                done,
                at,
                link,
            }
        })
        .collect();
    Quest {
        path,
        name,
        status,
        steps,
    }
}

/// The steps under the `## ` headings in `headings`: each line's text, whether it is
/// ticked, and where its task box is in `text`.
fn steps<'a>(text: &'a str, headings: &[String]) -> Vec<(&'a str, bool, Option<usize>)> {
    let body_start = match note::split_front_matter(text) {
        Ok((_, body)) => text.len() - body.len(),
        Err(_) => 0,
    };
    let mut found = Vec::new();
    let mut in_steps = false;
    let mut in_code = false;
    let mut offset = body_start;
    for line in text[body_start..].split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let line = line.trim_end_matches(['\n', '\r']);
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code = !in_code;
        }
        if !in_code && let Some(heading) = line.strip_prefix("## ") {
            in_steps = headings.iter().any(|wanted| wanted == heading.trim());
            continue;
        }
        if !in_steps || trimmed.is_empty() {
            continue;
        }
        let item = ["- ", "* ", "+ "]
            .iter()
            .find_map(|mark| trimmed.strip_prefix(mark))
            .or_else(|| {
                let digits = trimmed.find(|c: char| !c.is_ascii_digit())?;
                (digits > 0)
                    .then(|| trimmed[digits..].strip_prefix(". "))
                    .flatten()
            });
        let Some(item) = item.map(str::trim_start) else {
            found.push((trimmed.trim_end(), false, None));
            continue;
        };
        let at = start + (line.len() - item.len());
        match item.get(..3) {
            Some(tick @ ("[ ]" | "[x]" | "[X]")) => {
                found.push((item[3..].trim(), tick != "[ ]", Some(at)));
            }
            _ => found.push((item.trim_end(), false, None)),
        }
    }
    found
}

/// `line` with its links shown as their text: `[[Mira|the smith]]` as "the smith".
fn shown(line: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for link in links::find(line) {
        out.push_str(&line[last..link.range.start]);
        let inner = line[link.range.clone()]
            .trim_start_matches('!')
            .trim_start_matches("[[")
            .trim_end_matches("]]");
        out.push_str(inner.rsplit('|').next().unwrap_or(inner).trim());
        last = link.range.end;
    }
    out.push_str(&line[last..]);
    out
}

/// The quests with one status, and which of them start a quest line.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub status: String,
    /// The quests at the top of the group, by index: those no other quest has as a step.
    pub quests: Vec<usize>,
}

/// The quests grouped by status, in the order of `options` (the Status field's choices),
/// then any other statuses by name, then "Other" for quests with none. A quest that is a
/// step of another is shown under that one, not again at the top.
pub fn groups(quests: &[Quest], options: &[String]) -> Vec<Group> {
    let position = |path: &Path| quests.iter().position(|quest| quest.path == path);
    let mut stepped = vec![false; quests.len()];
    for (index, quest) in quests.iter().enumerate() {
        for step in &quest.steps {
            if let Some(child) = step.link.as_deref().and_then(position)
                && child != index
            {
                stepped[child] = true;
            }
        }
    }

    // Quests that are only steps of each other, in a loop, start the line themselves.
    let mut reached = vec![false; quests.len()];
    let mut stack: Vec<usize> = (0..quests.len()).filter(|&i| !stepped[i]).collect();
    while let Some(index) = stack.pop() {
        if std::mem::replace(&mut reached[index], true) {
            continue;
        }
        stack.extend(
            quests[index]
                .steps
                .iter()
                .filter_map(|step| step.link.as_deref().and_then(position)),
        );
    }
    let mut tops: Vec<usize> = (0..quests.len()).filter(|&i| !stepped[i]).collect();
    for index in 0..quests.len() {
        if !reached[index] {
            tops.push(index);
            let mut stack = vec![index];
            while let Some(index) = stack.pop() {
                if std::mem::replace(&mut reached[index], true) {
                    continue;
                }
                stack.extend(
                    quests[index]
                        .steps
                        .iter()
                        .filter_map(|step| step.link.as_deref().and_then(position)),
                );
            }
        }
    }
    tops.sort_by_cached_key(|&index| quests[index].name.to_lowercase());

    let status_of = |index: usize| quests[index].status.clone().unwrap_or_default();
    let mut statuses: Vec<String> = options.to_vec();
    let mut others: Vec<String> = Vec::new();
    for &index in &tops {
        let status = status_of(index);
        if !status.is_empty()
            && !statuses
                .iter()
                .any(|known| known.eq_ignore_ascii_case(&status))
            && !others.contains(&status)
        {
            others.push(status);
        }
    }
    others.sort_by_key(|status| status.to_lowercase());
    statuses.extend(others);
    statuses.push(String::new());

    statuses
        .into_iter()
        .filter_map(|status| {
            let in_group: Vec<usize> = tops
                .iter()
                .copied()
                .filter(|&index| status_of(index).eq_ignore_ascii_case(&status))
                .collect();
            (!in_group.is_empty()).then(|| Group {
                status: if status.is_empty() {
                    "Other".into()
                } else {
                    status
                },
                quests: in_group,
            })
        })
        .collect()
}

/// How many of the quest's steps are done, and how many it has. A step that is another
/// quest is done when its box is ticked or that quest is Complete.
pub fn progress(quests: &[Quest], index: usize) -> (usize, usize) {
    let steps = &quests[index].steps;
    let done = steps.iter().filter(|step| step_done(quests, step)).count();
    (done, steps.len())
}

fn step_done(quests: &[Quest], step: &Step) -> bool {
    step.done
        || step.link.as_deref().is_some_and(|link| {
            quests
                .iter()
                .any(|quest| quest.path == link && quest.complete())
        })
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Open the quest's note.
    Open(PathBuf),
    /// Tick or untick the step whose task box is at this place in the note's text.
    Tick(PathBuf, usize),
    /// Show or hide a quest's steps.
    Expand(PathBuf),
    /// Collapse or expand a status group.
    Group(String),
    Close,
}

/// The Quests list and what is shown of it.
#[derive(Debug, Default)]
pub struct Quests {
    pub open: bool,
    quests: Vec<Quest>,
    groups: Vec<Group>,
    /// The quests whose steps show.
    expanded: HashSet<PathBuf>,
    /// The groups opened or closed from how they start (finished ones closed), by status.
    toggled: HashSet<String>,
}

impl Quests {
    /// Shows `quests`, grouped by the Status choices in `options`.
    pub fn set(&mut self, quests: Vec<Quest>, options: &[String]) {
        self.groups = groups(&quests, options);
        self.quests = quests;
    }

    /// Handles the messages about what is shown. Opening, ticking and closing are the
    /// workspace's to do.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Expand(path) => {
                if !self.expanded.remove(&path) {
                    self.expanded.insert(path);
                }
            }
            Message::Group(status) => {
                let status = status.to_lowercase();
                if !self.toggled.remove(&status) {
                    self.toggled.insert(status);
                }
            }
            Message::Open(_) | Message::Tick(..) | Message::Close => {}
        }
    }

    fn collapsed(&self, status: &str) -> bool {
        let status = status.to_lowercase();
        FINISHED.contains(&status.as_str()) != self.toggled.contains(&status)
    }

    pub fn view(&self, c: Colors, selected: Option<&Path>) -> Element<'_, Message> {
        let header = row![
            text("QUESTS").size(11).color(c.text_muted),
            space::horizontal(),
            button(text("✕").size(11))
                .padding([2, 6])
                .style(style::flat(c, false))
                .on_press(Message::Close),
        ]
        .align_y(Center)
        .padding([2, 8]);
        let mut rows = vec![header.into()];
        if self.quests.is_empty() {
            rows.push(
                container(
                    text("No quests yet. Notes in a Quests folder show here.")
                        .size(12)
                        .color(c.text_muted),
                )
                .padding([2, 8])
                .into(),
            );
        }
        for group in &self.groups {
            let collapsed = self.collapsed(&group.status);
            rows.push(
                button(
                    row![
                        text(if collapsed { "▸" } else { "▾" }).size(11).width(12),
                        text(&group.status).size(12).color(c.text),
                        text(group.quests.len().to_string())
                            .size(11)
                            .color(c.text_muted),
                    ]
                    .spacing(6)
                    .align_y(Center),
                )
                .width(Fill)
                .padding([3, 8])
                .style(style::flat(c, false))
                .on_press(Message::Group(group.status.clone()))
                .into(),
            );
            if !collapsed {
                for &index in &group.quests {
                    self.quest(c, selected, index, 8.0, None, &mut vec![index], &mut rows);
                }
            }
        }
        column(rows).spacing(1).into()
    }

    /// Adds the rows of quest `index`, `left` from the sidebar's edge, and of its steps if
    /// they show. A quest that is a step of another has that step's box (`tick`) in front.
    /// `line` holds the quests above it, so a loop of quests stops.
    #[allow(clippy::too_many_arguments)]
    fn quest<'a>(
        &'a self,
        c: Colors,
        selected: Option<&Path>,
        index: usize,
        left: f32,
        tick: Option<Element<'a, Message>>,
        line: &mut Vec<usize>,
        rows: &mut Vec<Element<'a, Message>>,
    ) {
        let quest = &self.quests[index];
        let (done, total) = progress(&self.quests, index);
        let expanded = self.expanded.contains(&quest.path);

        let arrow: Element<'_, Message> = if total > 0 {
            button(text(if expanded { "▾" } else { "▸" }).size(11))
                .padding([0, 2])
                .width(16)
                .style(style::flat(c, false))
                .on_press(Message::Expand(quest.path.clone()))
                .into()
        } else {
            space().width(16).into()
        };
        let name = button(
            text(&quest.name)
                .size(13)
                .color(c.text)
                .wrapping(text::Wrapping::None),
        )
        .width(Fill)
        .padding([1, 4])
        .style(style::flat(c, selected == Some(quest.path.as_path())))
        .on_press(Message::Open(quest.path.clone()));
        let count =
            (total > 0).then(|| text(format!("{done}/{total}")).size(11).color(c.text_muted));
        let bar_left = if tick.is_some() { 40.0 } else { 20.0 };
        let mut lines = column![
            row![]
                .push(tick)
                .push(arrow)
                .push(container(name).clip(true).width(Fill))
                .push(count)
                .spacing(4)
                .align_y(Center)
        ]
        .spacing(2);
        if total > 0 {
            lines = lines.push(container(bar(c, done, total)).padding(pad(0.0, 4.0, bar_left)));
        }
        rows.push(container(lines).padding(pad(2.0, 8.0, left)).into());

        if !expanded {
            return;
        }
        let left = left + 20.0;
        for step in &quest.steps {
            let tick = task_box(
                c,
                step_done(&self.quests, step),
                step.at.map(|at| Message::Tick(quest.path.clone(), at)),
            );
            let child = step
                .link
                .as_deref()
                .and_then(|link| self.quests.iter().position(|quest| quest.path == link))
                .filter(|child| !line.contains(child));
            match child {
                // A step that is another quest shows that quest, with its own steps.
                Some(child) => {
                    line.push(child);
                    self.quest(c, selected, child, left, Some(tick), line, rows);
                    line.pop();
                }
                None => rows.push(
                    container(
                        row![tick, text(&step.text).size(12).color(c.text)]
                            .spacing(6)
                            .align_y(Center),
                    )
                    .padding(pad(1.0, 8.0, left))
                    .into(),
                ),
            }
        }
    }
}

/// `vertical` above and below, `right` on the right and `left` on the left.
fn pad(vertical: f32, right: f32, left: f32) -> Padding {
    Padding {
        top: vertical,
        right,
        bottom: vertical,
        left,
    }
}

/// A thin bar filled as far as the quest has got.
fn bar<'a>(c: Colors, done: usize, total: usize) -> Element<'a, Message> {
    let part = |portion: usize, color| {
        container(space())
            .width(FillPortion(u16::try_from(portion).unwrap_or(u16::MAX)))
            .height(3)
            .style(move |_: &iced::Theme| container::Style::default().background(color))
    };
    let mut parts = row![];
    if done > 0 {
        parts = parts.push(part(done, c.accent));
    }
    if done < total {
        parts = parts.push(part(total - done, c.border));
    }
    parts.width(Fill).into()
}

/// A step's box, ticked if it is `done`, which sends `tick` when clicked.
fn task_box<'a>(c: Colors, done: bool, tick: Option<Message>) -> Element<'a, Message> {
    let check = container(
        text(if done { "✓" } else { "" })
            .size(10)
            .color(c.accent_text)
            .center(),
    )
    .center(13)
    .style(style::task_box(c, done));
    match tick {
        Some(tick) => mouse_area(check)
            .on_press(tick)
            .interaction(Interaction::Pointer)
            .into(),
        None => check.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quest_type() -> ContentType {
        crate::types::parse(include_str!("../templates/default/quests.yaml")).unwrap()
    }

    fn index(root: &Path, names: &[&str]) -> links::Index {
        links::Index::new(
            root,
            names
                .iter()
                .map(|name| root.join(format!("quests/{name}.md"))),
        )
    }

    fn quest(name: &str, status: &str, steps: &str, index: &links::Index) -> Quest {
        let text = format!(
            "---\ntype: quest\nname: {name}\nstatus: {status}\n---\n\n## Objectives\n\n{steps}\n"
        );
        read(
            PathBuf::from(format!("/c/quests/{name}.md")),
            name.into(),
            &text,
            &quest_type(),
            index,
        )
    }

    #[test]
    fn reads_steps_with_where_their_boxes_are() {
        let text = "---\ntype: quest\nstatus: Active\n---\n\n## Hook\n\n- [ ] not a step\n\n## Objectives\n\n- [x] Find [[Mira|the smith]]\n- [ ] Forge the blade\n1. Plain step\n";
        let quest = read(
            PathBuf::from("/c/quests/Q.md"),
            "Q".into(),
            text,
            &quest_type(),
            &index(Path::new("/c"), &[]),
        );
        assert_eq!(quest.status.as_deref(), Some("Active"));
        let shown: Vec<_> = quest
            .steps
            .iter()
            .map(|step| (step.text.as_str(), step.done))
            .collect();
        assert_eq!(
            shown,
            [
                ("Find the smith", true),
                ("Forge the blade", false),
                ("Plain step", false)
            ]
        );
        let at = quest.steps[1].at.unwrap();
        assert_eq!(&text[at..at + 3], "[ ]");
        assert_eq!(quest.steps[2].at, None);
        // Ticking it with the reading view's toggle changes only that box.
        let ticked = crate::preview::toggle_task(text, at).unwrap();
        assert!(ticked.contains("- [x] Forge the blade"));
    }

    #[test]
    fn groups_by_status_with_sub_quests_under_their_quest_line() {
        let index = index(Path::new("/c"), &["Big", "Small", "Side", "Odd"]);
        let quests = vec![
            quest("Big", "Active", "- [ ] [[Small]]\n- [ ] Win", &index),
            quest("Small", "Complete", "- [x] Done it", &index),
            quest("Side", "Rumored", "", &index),
            quest("Odd", "Sleeping", "", &index),
        ];
        let options = ["Rumored", "Active", "Complete"].map(String::from);
        let groups = groups(&quests, &options);
        let shown: Vec<_> = groups
            .iter()
            .map(|group| (group.status.as_str(), group.quests.clone()))
            .collect();
        // Small is a step of Big, so it isn't shown again at the top of Complete.
        assert_eq!(
            shown,
            [
                ("Rumored", vec![2]),
                ("Active", vec![0]),
                ("Sleeping", vec![3])
            ]
        );
        // Big's step that is Small counts as done, since Small is Complete.
        assert_eq!(progress(&quests, 0), (1, 2));
    }

    #[test]
    fn quests_that_step_to_each_other_still_show() {
        let index = index(Path::new("/c"), &["A", "B"]);
        let quests = vec![
            quest("A", "", "- [ ] [[B]]", &index),
            quest("B", "", "- [ ] [[A]]", &index),
        ];
        let groups = groups(&quests, &[]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].status, "Other");
        assert_eq!(groups[0].quests.len(), 1);
    }

    #[test]
    fn finished_groups_start_collapsed() {
        let mut quests = Quests::default();
        assert!(quests.collapsed("Complete"));
        assert!(!quests.collapsed("Active"));
        quests.update(Message::Group("Complete".into()));
        assert!(!quests.collapsed("Complete"));
    }
}
