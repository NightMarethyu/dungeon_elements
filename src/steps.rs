//! Undo and Redo for a dialog while it is open, like Note Types.
//!
//! A dialog's whole state is small, so each step keeps a copy of it as it was before the
//! change. Typing in one box is one step until a pause, as in a note.

use std::time::{Duration, Instant};

use iced::Element;
use iced::widget::{button as plain_button, container, svg, text, tooltip};

use crate::style;
use crate::theme::{self, Colors};

/// The Undo arrow, drawn in black for [`button`] to color.
pub const UNDO_ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M5 2.5 1.5 6 5 9.5M1.5 6H10a4 4 0 0 1 0 8H6.5" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;
/// The Redo arrow.
pub const REDO_ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M11 2.5 14.5 6 11 9.5M14.5 6H6a4 4 0 0 0 0 8h3.5" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;

/// At most this many steps are kept; the oldest go first.
pub const MAX_STEPS: usize = 200;
/// Typing after a pause this long starts a new step.
pub const PAUSE: Duration = Duration::from_secs(1);
/// What a step of typing is called, after "Undo" or "Redo".
pub const TYPING: &str = "Typing";

/// What a change is called as a step, and for typing, the box typed in.
pub struct Step {
    pub label: String,
    /// The box typed in, so a burst of typing in one box is one step.
    pub typed: Option<String>,
}

impl Step {
    /// A step called `label`, like "Remove Field Status".
    pub fn named(label: impl Into<String>) -> Self {
        Step {
            label: label.into(),
            typed: None,
        }
    }

    /// Typing in the box called `what`.
    pub fn typing(what: impl Into<String>) -> Self {
        Step {
            label: TYPING.into(),
            typed: Some(what.into()),
        }
    }
}

/// The steps Undo can take back and Redo can do again, each with the state it had before.
pub struct Steps<T> {
    undo: Vec<(T, String)>,
    redo: Vec<(T, String)>,
    /// The box last typed in and when.
    typing: Option<(String, Instant)>,
}

impl<T> Default for Steps<T> {
    fn default() -> Self {
        Steps {
            undo: Vec::new(),
            redo: Vec::new(),
            typing: None,
        }
    }
}

impl<T: PartialEq> Steps<T> {
    /// Notes a change from `before` to `after`. Nothing is kept if nothing changed, or
    /// for more typing in the box typed in last, soon after.
    pub fn record(&mut self, before: T, after: &T, step: Step) {
        self.record_at(before, after, step, Instant::now());
    }

    fn record_at(&mut self, before: T, after: &T, step: Step, now: Instant) {
        if before == *after {
            return;
        }
        let burst = step.typed.as_ref().is_some_and(|typed| {
            self.typing
                .as_ref()
                .is_some_and(|(last, at)| last == typed && now.duration_since(*at) < PAUSE)
        });
        if !burst {
            self.undo.push((before, step.label));
            if self.undo.len() > MAX_STEPS {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.typing = step.typed.map(|typed| (typed, now));
    }

    /// What Undo would take back, like "Add Field".
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|(_, label)| label.as_str())
    }

    /// What Redo would do again.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|(_, label)| label.as_str())
    }

    /// Takes back the last step (or with `redo`, does the last one taken back again),
    /// given the state as it is `now`. Gives the state to go back to and what the step
    /// was, or `None` with nothing to do.
    pub fn undo(&mut self, now: T, redo: bool) -> Option<(T, String)> {
        let (from, to) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        let (state, label) = from.pop()?;
        to.push((now, label.clone()));
        self.typing = None;
        Some((state, label))
    }
}

/// The Undo (or with `redo`, Redo) arrow, greyed out with nothing to do, and a tooltip
/// saying what it would do, like "Undo Add Field (Ctrl+Z)". `what` is that step.
pub fn button<'a, M: Clone + 'a>(
    c: Colors,
    redo: bool,
    what: Option<String>,
    message: M,
) -> Element<'a, M> {
    let (name, keys, icon) = if redo {
        ("Redo", "Ctrl+Y", REDO_ICON)
    } else {
        ("Undo", "Ctrl+Z", UNDO_ICON)
    };
    let color = if what.is_some() { c.text } else { c.text_muted };
    let icon = icon.replace("#000", &theme::to_hex(color));
    let icon = svg(svg::Handle::from_memory(icon.into_bytes()))
        .width(16)
        .height(16);
    let hint = match what {
        Some(ref what) => format!("{name} {what} ({keys})"),
        None => format!("Nothing to {} ({keys})", name.to_lowercase()),
    };
    tooltip(
        plain_button(container(icon).center_y(20))
            .padding([2, 6])
            .style(style::flat(c, false))
            .on_press_maybe(what.map(|_| message)),
        container(text(hint).size(12))
            .padding([2, 6])
            .style(style::dropdown(c)),
        tooltip::Position::Bottom,
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undoes_and_redoes_steps() {
        let mut steps = Steps::default();
        steps.record(1, &2, Step::named("Add"));
        steps.record(2, &3, Step::named("Add Again"));
        steps.record(3, &3, Step::named("Nothing"));
        assert_eq!(steps.undo_label(), Some("Add Again"));

        assert_eq!(steps.undo(3, false), Some((2, "Add Again".into())));
        assert_eq!(steps.redo_label(), Some("Add Again"));
        assert_eq!(steps.undo(2, false), Some((1, "Add".into())));
        assert_eq!(steps.undo(1, false), None);
        assert_eq!(steps.undo(1, true), Some((2, "Add".into())));

        // A new change ends what can be done again.
        steps.record(2, &5, Step::named("Change"));
        assert_eq!(steps.redo_label(), None);
        assert_eq!(steps.undo(5, false), Some((2, "Change".into())));
    }

    #[test]
    fn a_burst_of_typing_in_one_box_is_one_step() {
        let mut steps = Steps::default();
        let start = Instant::now();
        steps.record_at("".to_string(), &"S".into(), Step::typing("label"), start);
        steps.record_at("S".into(), &"St".into(), Step::typing("label"), start);
        steps.record_at("St".into(), &"Sta".into(), Step::typing("label"), start);
        // Another box, then a pause, start new steps.
        steps.record_at("Sta".into(), &"Sta2".into(), Step::typing("key"), start);
        steps.record_at(
            "Sta2".into(),
            &"Sta2!".into(),
            Step::typing("key"),
            start + PAUSE * 2,
        );
        assert_eq!(steps.undo("Sta2!".into(), false).unwrap().0, "Sta2");
        assert_eq!(steps.undo("Sta2".into(), false).unwrap().0, "Sta");
        assert_eq!(steps.undo("Sta".into(), false).unwrap().0, "");
        assert_eq!(steps.undo("".into(), false), None);
    }

    #[test]
    fn keeps_at_most_so_many_steps() {
        let mut steps = Steps::default();
        for n in 0..MAX_STEPS + 10 {
            steps.record(n, &(n + 1), Step::named("Add"));
        }
        let mut undone = 0;
        let mut now = MAX_STEPS + 10;
        while let Some((state, _)) = steps.undo(now, false) {
            now = state;
            undone += 1;
        }
        assert_eq!(undone, MAX_STEPS);
        assert_eq!(now, 10);
    }
}
