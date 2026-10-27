# Changelog

All notable changes to Dungeon Elements are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Links

- Link notes the Obsidian way with `[[Note]]`. Typing `[[` closes it with `]]` and suggests notes by name under the editor; press Tab (or click one) to insert it. Link fields in forms suggest notes the same way.
- **Edit > Insert Link** (Ctrl+K) turns the selected text into a link.
- Links are colored in the editor, and links to notes that don't exist yet are dimmer.
- Ctrl+click a link, or use the bar under the editor, to open the note. Following a link to a note that doesn't exist yet creates it. Link fields have an **Open** button.
- Link fields link to a folder: a Location field suggests locations first, and typing a place that doesn't exist yet offers **+ New Location**, which makes it in the locations folder with its name filled in. Set a field's folder in **Edit > Types** (Links to).
- Drag a note in the file tree onto another folder to move it. It becomes that folder's type and keeps the fields you filled in, and links to it are updated.
- Renaming a note by changing its name and saving updates every link to it across the campaign, including links in front matter fields and in open files with unsaved changes. Shown text (`[[Mira|the smuggler]]`), headings (`[[Mira#Appearance]]`), and embeds are kept.
- Notes renamed or moved outside the app (in Explorer, a file manager, or Obsidian with link updating off) are noticed: their tabs follow them, their name field takes the new name, and every link to them is updated.
- Links that look broken are listed as problems: ones to a note that was renamed or deleted outside the app, or to a missing note when one is spelled almost the same (`[[Bobby]]` when there's a Bob). Links to notes you haven't written yet are fine and aren't flagged.

- Links work with any name: apostrophes and quotes (`John's Long Johns`), accents, other alphabets and emoji. In the front matter a link is read as the YAML string it's in, so `owner: '[[John''s Long Johns]]'` links to John's Long Johns, and renaming or relinking writes names with the escaping YAML needs. Typing a link in the front matter in the Markdown source escapes it too.
- A note's name can hold characters a file name can't (`Chapter 1: The Inn`, `What?`, `The "Lucky" Inn`). The name field keeps the name as typed, and the file gets a name that works in Windows, macOS, Linux, and in links (`Chapter 1 - The Inn.md`, `What.md`, `The 'Lucky' Inn.md`). Links written with the full name still find the note.
- Fixed a crash when a line with an accented letter or other non-English character held a `[[link]]`.

### Fixing problems

- Hover the ⚠ next to a note in the file tree to see what's wrong with it, and click it for a dialog of fixes: link to the note it probably meant, make the missing note (in the link field's folder), remove the link, or ignore it; move a long text field from the front matter into its `## ` section; use the folder's type when `type:` names none; or change a choice like `rare` to `Rare`. **Problems** in the status bar has a **Fix...** button for each note too.
- A note with problems shows the first one under the editor, with **Fix...**.
- Problems are checked again as you type, in the form or the Markdown source, so fixing one makes its ⚠ go away right away, before saving.
- The campaign folder is watched, so notes edited, renamed, or deleted in another app are picked up right away, without switching back to the window first. Hidden folders like `.obsidian` are left out.

### Right-click menus

- Right-click a note or folder in the file tree to open it (or open it in the reading view), rename it, move it to another folder, delete it, copy a `[[link]]` to it, or show it in Explorer. Folders also offer **New Note Here**, **New Folder Here**, and their note fields (**Edit > Types**). Right-click the space around the tree for a new note or folder at the top of the campaign.
- Deleting asks first and sends the note or folder to the Recycle Bin (the Trash on Linux), so it can be brought back.
- Renaming a note changes its name field too. Renaming or moving a folder moves everything in it, updates the links to its notes, and points link fields that linked to it at its new place.
- Right-click a tab to save it, close it, close the others, close the tabs to its right, or close them all.
- Right-click in a text editor (a note's text, a long text field, or a block you're editing in the reading view) to cut, copy, paste, or select all, open or create the `[[link]]` you clicked, or insert a link or picture. Right-clicking puts the cursor where you clicked, unless text is selected.
- Right-click a form or the reading view to add a field, switch views, add text at the end, copy a link to the note, or find it in the file tree.

### Reading view

- **Read** at the top right of the editor (or **View > Reading View**, Ctrl+R) shows a note formatted, like GitHub shows a README: the note's name as a title, its fields in a box on the right like a wiki's infobox (beside the note's opening text), then its headings, lists, task lists, tables, quotes, code blocks, and pictures. **Edit** goes back to the form or the Markdown source.
- Links in the reading view can be clicked. `[[Links]]` open the note (links to notes that don't exist yet are dimmer, and clicking one makes the note), web links open in your browser, and Markdown links to files in the campaign open them.
- Pictures next to a note, or embedded the Obsidian way (`![[harbor.png]]`, found anywhere in the campaign), show in the reading view. PNG, JPEG, GIF, WebP, and SVG work.
- Pictures from the web (`![Map](https://...)`) download in the background and show in the reading view.
- **Edit > Insert Picture** (Ctrl+Shift+K), or dropping a picture file on the window, adds a picture from anywhere on your computer: it's copied into the campaign's `pictures` folder (so the campaign stays complete when you move or share it) and embedded where the cursor is, or at the end of the note in the reading view. Full paths like `![](C:/Pictures/map.png)` show too.
- Code blocks are colored by their language (` ```python `, ` ```yaml `, ` ```js `, and most others). Themes set the colors with `code_comment`, `code_string`, `code_number`, `code_keyword`, `code_function`, and `code_type`.
- `![[Note]]` shows another note's text inside this one, in a box with its name, which opens it. `![[Note#Heading]]` shows just that heading's section. Notes embedded there show as links.
- **View > Open Notes in Reading View** opens notes formatted instead of in an editor.
- Edit right in the reading view: click a paragraph, list, table, or heading to edit its Markdown in place, and it's formatted again when you click elsewhere or press Escape. Click a field in the box to change it (a choice shows its options, and a checkbox flips), click the title to rename the note, and click under the note to add more.

### Dice roller

- The d20 button at the right of the tab bar opens a dice roller along the bottom of the window. Click a die (d20, d12, d10, d100, d8, d6, or d4) to roll it; the result shows on the left.
- Type a number in a die's **Count** box to roll several at once. Each result shows, with their total, and the last few rolls are listed under it.

## [0.1.0] - 2026-10-25

The first release: Phase 0, _Call to Action_, of the [plan](PLAN.md). Dungeon Elements is in early development, so expect rough edges, and keep backups of campaigns you care about.

### Campaigns

- Start on a splash screen that opens a campaign folder or creates a new one. New campaigns can start with folders for items, journal, locations, lore, NPCs, players, and quests.
- Open a campaign straight from the command line: `dungeon_elements path/to/my-campaign`.
- Browse the campaign in a collapsible file tree, with a tab for each open file and a status bar.
- Make folders with the round **+** under the tree, optionally with a note type.

### Notes

- Write notes in a text editor and save them as Markdown (`.md`) files, with **New Note** (Ctrl+N), **Open File...** (Ctrl+O), **Save** (Ctrl+S), **Save As...** (Ctrl+Shift+S), and **Close Tab** (Ctrl+W). Closing a tab or the window with unsaved changes asks first.
- **File > Save All** (Ctrl+Alt+S) saves every open note with unsaved changes. New notes without a file yet ask where to go, one at a time, and the status bar says how many were saved and why any weren't.
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
- The colored logo is the app icon on the window, the taskbar, the Windows executable, and the macOS app.
- Builds for Windows, Linux, and macOS (universal, for Apple silicon and Intel).

### Known limitations

- `[[links]]` are stored but not clickable yet, and Markdown isn't rendered (Phase 1).
- **Open Campaign...** on Linux uses the desktop's file chooser portal (or `zenity`); if neither is installed, open a campaign from the command line instead.
- Builds are not code-signed, so Windows SmartScreen may warn the first time you run the app ("More info" > "Run anyway").
- macOS: the app is built on GitHub's Macs but hasn't been tested on one by hand yet. It isn't signed with an Apple Developer ID, so the first time, macOS says it can't check the app: open **System Settings > Privacy & Security**, scroll down, and click **Open Anyway** (on macOS 14 and older, Control-click the app and choose **Open**). Settings and themes live in `~/.config/dungeon_elements`.

[Unreleased]: https://github.com/NightMarethyu/dungeon_elements/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/NightMarethyu/dungeon_elements/releases/tag/v0.1.0
