# Changelog

All notable changes to Dungeon Elements are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-10-25

The first release: Phase 0, _Call to Action_, of the [plan](PLAN.md). Dungeon Elements is in early development, so expect rough edges, and keep backups of campaigns you care about.

### Campaigns

- Start on a splash screen that opens a campaign folder or creates a new one. New campaigns can start with folders for items, journal, locations, lore, NPCs, players, and quests.
- Open a campaign straight from the command line: `dungeon_elements path/to/my-campaign`.
- Browse the campaign in a collapsible file tree, with a tab for each open file and a status bar.
- Make folders with the round **+** under the tree, optionally with a note type.

### Notes

- Write notes in a text editor and save them as Markdown (`.md`) files, with **New Note** (Ctrl+N), **Open File...** (Ctrl+O), **Save** (Ctrl+S), **Save As...** (Ctrl+Shift+S), and **Close Tab** (Ctrl+W). Closing a tab or the window with unsaved changes asks first.
- Give a folder a content type in its `_type.yaml`, and its notes open as a form (like Name, Value, Rarity, History, and Stats for an item). Fields can be text, numbers, choices, tags, `[[links]]`, checkboxes, or long text. Notes are saved as Markdown with YAML front matter that Obsidian can read.
- The **+** next to a folder makes a new note of its type, saved as `<name>.md`.
- Add a field to a single note with the round **+** on its form, and remove any field except the name from a single note.
- Change the fields of every note in a folder in **Edit > Types**.
- See the file itself with **View > Show Markdown Source** (Ctrl+E).

### Checking files

- Opening or refreshing a campaign checks every note and `_type.yaml` for formatting the app can't show as intended (broken front matter, a word in a number field, a choice that isn't one of the options, and so on). Files with problems get a ⚠ in the tree, and the ⚠ in the status bar lists what's wrong. Nothing is changed for you.
- Switching back to the app notices files edited elsewhere, such as in Obsidian. Open files without unsaved changes reload; files with unsaved changes ask whether to reload or keep your version, and saving never silently overwrites an outside edit.

### Look and feel

- A real menu bar (File, Edit, View, Help) with the logo.
- Color themes: Dark (the default) and High Contrast, plus your own theme files (see the README). **View** can also show file extensions and `_type.yaml` files.
- The colored logo is the app icon on the window, the taskbar, and the Windows executable.

### Known limitations

- `[[links]]` are stored but not clickable yet, and Markdown isn't rendered (Phase 1).
- **Open Campaign...** on Linux uses the desktop's file chooser portal (or `zenity`); if neither is installed, open a campaign from the command line instead.
- Builds are not code-signed, so Windows SmartScreen may warn the first time you run the app ("More info" > "Run anyway").

[Unreleased]: https://github.com/NightMarethyu/dungeon_elements/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/NightMarethyu/dungeon_elements/releases/tag/v0.1.0
