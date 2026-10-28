//! Watches the open campaign folder, so notes edited, renamed or deleted in another app
//! (or a file manager) are picked up without waiting for the window to be focused.

use std::path::{Component, Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use notify_debouncer_mini::notify::{self, EventKind, RecursiveMode, Watcher};

/// How long the folder must be quiet before a change is sent, so a save that writes
/// several times (or a rename, which is a delete and a create) counts as one change.
const QUIET: Duration = Duration::from_millis(300);

/// Sends `()` each time files in `root` change, apart from hidden folders like
/// `.obsidian` and `.git`, which other apps change all the time.
///
/// Files only being read don't count: on Linux the watcher also reports files being
/// opened and closed, and reading the changed notes again would then look like another
/// change, over and over.
// `Subscription::run_with` hands over a `&PathBuf`, not a `&Path`.
#[allow(clippy::ptr_arg)]
pub fn changes(root: &PathBuf) -> impl Stream<Item = ()> + use<> {
    let root = root.clone();
    iced::stream::channel(8, async move |mut output| {
        let (sender, mut changes) = mpsc::unbounded();
        let (events, quiet) = std_mpsc::channel();
        let watched = root.clone();
        let watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
            if let Ok(event) = result
                && !matches!(event.kind, EventKind::Access(_))
                && event.paths.iter().any(|path| !is_hidden(&watched, path))
            {
                let _ = events.send(());
            }
        });
        let Ok(mut watcher) = watcher else {
            return;
        };
        if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
            return;
        }

        // Waits for the folder to be quiet, then sends one change for everything before.
        std::thread::spawn(move || {
            while quiet.recv().is_ok() {
                while quiet.recv_timeout(QUIET).is_ok() {}
                if sender.unbounded_send(()).is_err() {
                    break;
                }
            }
        });

        while changes.next().await.is_some() {
            if output.send(()).await.is_err() {
                break;
            }
        }
        drop(watcher);
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
