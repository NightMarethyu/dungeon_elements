//! Undo and Redo for a note's text.
//!
//! Each open note keeps its text as of its last step, and a list of steps. A step holds
//! only the part of the text that changed (where, the old words and the new words), not a
//! copy of the note, so a long history takes little memory. Nothing is worked out while
//! typing: the app only notes the time, and a step is made when typing pauses or
//! something else happens (see [`History::record`]), by comparing the note's text with
//! the one kept.

use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::{Duration, Instant};

/// At most this many steps are kept for a note; the oldest go first.
pub const MAX_STEPS: usize = 1000;
/// At most this much changed text is kept for a note, in bytes; the oldest steps go first.
pub const MAX_BYTES: usize = 5 * 1024 * 1024;
/// Typing after a pause this long starts a new step.
pub const PAUSE: Duration = Duration::from_secs(1);

/// What a step is called in the Edit menu, after "Undo" or "Redo".
pub const TYPING: &str = "Typing";

/// One change to a note's text: at byte `at`, `old` became `new`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    at: usize,
    old: String,
    new: String,
}

impl Change {
    /// The change that turns `before` into `after`, if they differ: everything between
    /// the start and the end they share.
    pub fn between(before: &str, after: &str) -> Option<Change> {
        if before == after {
            return None;
        }
        let (b, a) = (before.as_bytes(), after.as_bytes());
        let mut start = common_prefix(b, a);
        while !before.is_char_boundary(start) || !after.is_char_boundary(start) {
            start -= 1;
        }
        let most = b.len().min(a.len()) - start;
        let mut end = common_suffix(&b[b.len() - most..], &a[a.len() - most..]);
        while !before.is_char_boundary(b.len() - end) || !after.is_char_boundary(a.len() - end) {
            end -= 1;
        }
        Some(Change {
            at: start,
            old: before[start..b.len() - end].to_string(),
            new: after[start..a.len() - end].to_string(),
        })
    }

    /// Makes the change in `text`, the text before it.
    fn apply(&self, text: &mut String) {
        splice(text, self.at, self.old.len(), &self.new);
    }

    /// Takes the change back in `text`, the text after it.
    fn revert(&self, text: &mut String) {
        splice(text, self.at, self.new.len(), &self.old);
    }

    /// The memory the change takes, roughly.
    fn size(&self) -> usize {
        self.old.len() + self.new.len() + std::mem::size_of::<Self>()
    }
}

/// How many bytes `a` and `b` start with alike. Compared a block at a time, which is
/// quick even on a long note.
fn common_prefix(a: &[u8], b: &[u8]) -> usize {
    const BLOCK: usize = 128;
    let most = a.len().min(b.len());
    let mut at = 0;
    while at + BLOCK <= most && a[at..at + BLOCK] == b[at..at + BLOCK] {
        at += BLOCK;
    }
    while at < most && a[at] == b[at] {
        at += 1;
    }
    at
}

/// How many bytes `a` and `b` end with alike.
fn common_suffix(a: &[u8], b: &[u8]) -> usize {
    const BLOCK: usize = 128;
    let most = a.len().min(b.len());
    let (a, b) = (&a[a.len() - most..], &b[b.len() - most..]);
    let mut same = 0;
    while same + BLOCK <= most
        && a[most - same - BLOCK..most - same] == b[most - same - BLOCK..most - same]
    {
        same += BLOCK;
    }
    while same < most && a[most - same - 1] == b[most - same - 1] {
        same += 1;
    }
    same
}

/// Replaces the `len` bytes at `at` in `text` with `with`. Positions past the end (which
/// a step can't make, but a text changed some other way could) are kept in the text.
fn splice(text: &mut String, at: usize, len: usize, with: &str) {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    let mut end = (at + len).min(text.len());
    while !text.is_char_boundary(end) {
        end += 1;
    }
    text.replace_range(at..end, with);
}

/// Where the cursor was when a step was made, to put it back there on Undo and Redo:
/// the form field (if the form showed), and the line and column in its text box.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Place {
    pub field: Option<usize>,
    pub cursor: Option<(usize, usize)>,
}

#[derive(Debug, Clone)]
struct Step {
    label: &'static str,
    change: Change,
    place: Place,
}

/// One note's Undo and Redo.
#[derive(Debug, Clone)]
pub struct History {
    /// The note's text as of the last step.
    base: String,
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    /// What the steps in `undo` and `redo` take, for [`MAX_BYTES`].
    bytes: usize,
    /// When the last key was typed, while the typing isn't a step yet.
    typing: Option<Instant>,
    /// A hash of the text as it was last saved (or read from the file).
    saved: u64,
}

impl History {
    /// A history starting from `text`, which is the note as it is saved.
    pub fn new(text: &str) -> Self {
        History {
            base: text.to_string(),
            undo: VecDeque::new(),
            redo: Vec::new(),
            bytes: 0,
            typing: None,
            saved: hash(text),
        }
    }

    /// The text as of the last step.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Whether typing has happened since the last step.
    pub fn is_typing(&self) -> bool {
        self.typing.is_some()
    }

    /// Whether the typing since the last step paused long enough, as of `now`, that the
    /// next key typed starts a new step.
    pub fn paused(&self, now: Instant) -> bool {
        self.typing
            .is_some_and(|last| now.saturating_duration_since(last) >= PAUSE)
    }

    /// Notes that a key was typed, to make a step of later.
    pub fn typed(&mut self, now: Instant) {
        self.typing = Some(now);
    }

    /// Makes a step of whatever changed between the last step and `text`, called
    /// `label` (or [`TYPING`] for typing). Returns whether there was a change.
    pub fn record(&mut self, text: &str, label: &'static str, place: Place) -> bool {
        self.typing = None;
        let Some(change) = Change::between(&self.base, text) else {
            return false;
        };
        self.base.clear();
        self.base.push_str(text);
        self.bytes += change.size();
        self.undo.push_back(Step {
            label,
            change,
            place,
        });
        for step in self.redo.drain(..) {
            self.bytes -= step.change.size();
        }
        while self.undo.len() > MAX_STEPS || (self.bytes > MAX_BYTES && self.undo.len() > 1) {
            if let Some(step) = self.undo.pop_front() {
                self.bytes -= step.change.size();
            }
        }
        true
    }

    /// What Undo would undo, like "Typing".
    pub fn undo_label(&self) -> Option<&'static str> {
        self.undo.back().map(|step| step.label)
    }

    /// What Redo would do again.
    pub fn redo_label(&self) -> Option<&'static str> {
        self.redo.last().map(|step| step.label)
    }

    /// Takes back the last step: the text as it was before it, and where the cursor was.
    /// Make a step of any changes first (see [`Self::record`]), or they are lost.
    pub fn undo(&mut self) -> Option<(String, Place)> {
        let step = self.undo.pop_back()?;
        step.change.revert(&mut self.base);
        let place = step.place;
        self.redo.push(step);
        Some((self.base.clone(), place))
    }

    /// Does the last step taken back again.
    pub fn redo(&mut self) -> Option<(String, Place)> {
        let step = self.redo.pop()?;
        step.change.apply(&mut self.base);
        let place = step.place;
        self.undo.push_back(step);
        Some((self.base.clone(), place))
    }

    /// Notes that `text` was saved, so going back to it shows the note as saved.
    pub fn saved(&mut self, text: &str) {
        self.saved = hash(text);
    }

    /// Whether `text` is the note as it was last saved.
    pub fn is_saved(&self, text: &str) -> bool {
        hash(text) == self.saved
    }

    /// How many steps there are to undo and to redo.
    #[cfg(test)]
    fn len(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }
}

fn hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn here() -> Place {
        Place::default()
    }

    fn applied(change: &Change, text: &str) -> String {
        let mut text = text.to_string();
        change.apply(&mut text);
        text
    }

    fn reverted(change: &Change, text: &str) -> String {
        let mut text = text.to_string();
        change.revert(&mut text);
        text
    }

    #[test]
    fn a_change_holds_only_what_changed() {
        let change = Change::between("The old inn", "The new inn").unwrap();
        assert_eq!(
            change,
            Change {
                at: 4,
                old: "old".into(),
                new: "new".into(),
            }
        );
        assert_eq!(Change::between("same", "same"), None);
        // Typing at the end, and deleting from the start.
        let typed = Change::between("Gor", "Gorbash").unwrap();
        assert_eq!(
            (typed.at, typed.old.as_str(), typed.new.as_str()),
            (3, "", "bash")
        );
        let cut = Change::between("Gorbash", "bash").unwrap();
        assert_eq!((cut.at, cut.old.as_str(), cut.new.as_str()), (0, "Gor", ""));
        // A repeated letter isn't counted twice.
        let doubled = Change::between("aa", "aaa").unwrap();
        assert_eq!(applied(&doubled, "aa"), "aaa");
        assert_eq!(reverted(&doubled, "aaa"), "aa");
    }

    #[test]
    fn changes_keep_to_whole_letters() {
        for (before, after) in [
            ("Þórr", "Þorr"),
            ("ä", "ö"),
            ("日本", "日米"),
            ("café", "cafe"),
            ("🐉 dragon", "🐲 dragon"),
        ] {
            let change = Change::between(before, after).unwrap();
            assert_eq!(applied(&change, before), after);
            assert_eq!(reverted(&change, after), before);
        }
    }

    #[test]
    fn undo_and_redo_go_back_and_forth() {
        let mut history = History::new("Gorbash");
        assert!(history.record("Gorbash the Bold", "Typing", here()));
        assert!(history.record("Gorbash the Bold\n\n**Orc**", "Bold", here()));
        assert_eq!(history.undo_label(), Some("Bold"));

        assert_eq!(history.undo().unwrap().0, "Gorbash the Bold");
        assert_eq!(history.undo().unwrap().0, "Gorbash");
        assert!(history.undo().is_none());
        assert_eq!(history.redo_label(), Some("Typing"));
        assert_eq!(history.redo().unwrap().0, "Gorbash the Bold");
        assert_eq!(history.redo().unwrap().0, "Gorbash the Bold\n\n**Orc**");
        assert!(history.redo().is_none());
        assert_eq!(history.len(), (2, 0));
    }

    #[test]
    fn a_new_change_ends_redo() {
        let mut history = History::new("a");
        history.record("ab", "Typing", here());
        history.undo();
        assert_eq!(history.len(), (0, 1));
        history.record("ac", "Typing", here());
        assert_eq!(history.len(), (1, 0));
        assert_eq!(history.bytes, history.undo[0].change.size());
    }

    #[test]
    fn nothing_changed_is_no_step() {
        let mut history = History::new("text");
        history.typed(Instant::now());
        assert!(!history.record("text", TYPING, here()));
        assert!(!history.is_typing());
        assert_eq!(history.len(), (0, 0));
    }

    #[test]
    fn typing_is_one_step_until_a_pause() {
        let start = Instant::now();
        let mut history = History::new("");
        history.typed(start);
        assert!(!history.paused(start + Duration::from_millis(400)));
        assert!(history.paused(start + PAUSE));
        assert!(history.record("Hello", TYPING, here()));
        assert!(!history.is_typing());
        assert_eq!(history.undo_label(), Some(TYPING));
    }

    #[test]
    fn the_oldest_steps_go_first() {
        let mut history = History::new("");
        let mut text = String::new();
        for _ in 0..MAX_STEPS + 50 {
            text.push('x');
            history.record(&text, "Typing", here());
        }
        assert_eq!(history.len(), (MAX_STEPS, 0));
        // Undoing them all leaves the text as it was at the oldest step kept.
        while history.undo().is_some() {}
        assert_eq!(history.base(), "x".repeat(50));

        // A step bigger than the limit is still kept, alone.
        let mut history = History::new("");
        history.record("small", "Typing", here());
        let big = "y".repeat(MAX_BYTES + 1);
        history.record(&big, "Paste", here());
        assert_eq!(history.len(), (1, 0));
        assert_eq!(history.undo().unwrap().0, "small");
    }

    #[test]
    fn saved_text_is_known_again() {
        let mut history = History::new("saved");
        history.record("saved and changed", "Typing", here());
        assert!(!history.is_saved(history.base()));
        let (text, _) = history.undo().unwrap();
        assert!(history.is_saved(&text));
        history.redo();
        history.saved("saved and changed");
        assert!(history.is_saved("saved and changed"));
    }

    #[test]
    fn steps_stay_small_and_quick_on_a_long_note() {
        // A 1 MB note, typed in one word at a time: each step holds the word, not the note.
        let mut text = "Lorem ipsum dolor sit amet. ".repeat(40_000);
        let mut history = History::new(&text);
        let start = Instant::now();
        for word in 0..200 {
            text.insert_str(text.len() / 2, &format!("word{word} "));
            history.record(&text, TYPING, here());
        }
        assert!(history.bytes < 200 * 100, "{} bytes", history.bytes);
        while history.undo().is_some() {}
        assert_eq!(
            history.base(),
            "Lorem ipsum dolor sit amet. ".repeat(40_000)
        );
        // Generous, for slow test machines and debug builds: well under a second.
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{:?}",
            start.elapsed()
        );
    }
}
