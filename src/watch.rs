//! Watches the open campaign folder, so notes edited, renamed or deleted in another app
//! (or a file manager) are picked up without waiting for the window to be focused.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, new_debouncer};

/// How long the folder must be quiet before a change is sent, so a save that writes
/// several times (or a rename, which is a delete and a create) counts as one change.
const QUIET: Duration = Duration::from_millis(300);

/// Sends `()` each time files in `root` change, apart from hidden folders like
/// `.obsidian` and `.git`, which other apps change all the time.
// `Subscription::run_with` hands over a `&PathBuf`, not a `&Path`.
#[allow(clippy::ptr_arg)]
pub fn changes(root: &PathBuf) -> impl Stream<Item = ()> + use<> {
    let root = root.clone();
    iced::stream::channel(8, async move |mut output| {
        let (sender, mut changes) = mpsc::unbounded();
        let watched = root.clone();
        let debouncer = new_debouncer(QUIET, move |result: DebounceEventResult| {
            if let Ok(events) = result
                && events.iter().any(|event| !is_hidden(&watched, &event.path))
            {
                let _ = sender.unbounded_send(());
            }
        });
        let Ok(mut debouncer) = debouncer else {
            return;
        };
        if debouncer
            .watcher()
            .watch(&root, RecursiveMode::Recursive)
            .is_err()
        {
            return;
        }

        while changes.next().await.is_some() {
            if output.send(()).await.is_err() {
                break;
            }
        }
        drop(debouncer);
    })
}

/// Whether `path` is inside a hidden folder of `root` (or is a hidden file).
fn is_hidden(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        relative.components().any(|part| {
            matches!(part, Component::Normal(name) if name.to_string_lossy().starts_with('.'))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_hidden_folders() {
        let root = Path::new("/c");
        assert!(is_hidden(root, Path::new("/c/.obsidian/workspace.json")));
        assert!(is_hidden(root, Path::new("/c/npcs/.Bob.md.swp")));
        assert!(!is_hidden(root, Path::new("/c/npcs/Bob.md")));
    }
}
