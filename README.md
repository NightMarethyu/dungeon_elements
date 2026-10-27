<div align="center">

# 🐉 Dungeon Elements

**A local-first campaign manager for tabletop RPGs, built in Rust.**

Write your world in plain Markdown, link it together like a wiki, and keep every file on your own drive.

![Version: 0.2.0](https://img.shields.io/badge/version-0.2.0-blue)
![Status: early development](https://img.shields.io/badge/status-early%20development-orange)
![Language: Rust](https://img.shields.io/badge/language-Rust-b7410e?logo=rust)
![License: MIT](https://img.shields.io/badge/license-MIT-blue)

</div>

---

> [!NOTE]
> Dungeon Elements is in **early development**, but Phases 0 and 1 of the [plan](PLAN.md) are done: you can build a campaign as a wiki of linked Markdown notes, read it formatted, and roll dice beside it. Keep backups of campaigns you care about. _"Dungeon Elements" is a working title._

## What it does

**Campaigns and notes**

- Open a campaign folder, or make a new one with starter folders (items, journal, locations, lore, NPCs, players, quests). The file tree on the left lists its folders and notes, and each note opens in a tab.
- Everything is in the **File**, **Edit**, **View**, and **Help** menus, with the usual shortcuts: **New Note** (Ctrl+N), **Open File...** (Ctrl+O), **Save** (Ctrl+S), **Save As...** (Ctrl+Shift+S), **Save All** (Ctrl+Alt+S), and **Close Tab** (Ctrl+W). **Help > How-To Guides** (F1) explains the basics, and **Help > Keyboard Shortcuts** (Ctrl+/) lists every shortcut.
- A folder can give its notes fields in a `_type.yaml` (each starter folder has one). Its notes open as a form, like Name, Value, History, and Stats for an item, and are saved as Markdown with YAML front matter that Obsidian can read. **Edit > Types** changes a folder's fields, and the round **+** on a form adds a field to just that note. **Form**, **Markdown**, and **Read** at the top right switch between the form, the Markdown text, and the reading view.
- Right-click the file tree, tabs, editors, and forms to rename, move, or delete notes and folders (deleted ones go to the Recycle Bin or Trash), make new ones, copy a link, and more. Drag a note onto another folder to move it; it takes that folder's type and keeps what you filled in.

**Links, like a wiki**

- Link notes the Obsidian way: type `[[` and pick a note from the suggestions, or select some text and press **Edit > Insert Link** (Ctrl+K). Links can show other text (`[[Mira|the smuggler]]`) or point to a heading (`[[Mira#Appearance]]`), and notes can have aliases, so the BBEG can also go by `[[Jerry]]`.
- Ctrl+click a link to open the note; a link to a note that doesn't exist yet makes it. Link fields point at a folder, so typing a new place in an NPC's Location offers to make it in your locations folder.
- Rename a note (in the app or outside it) and every link to it follows. Note names can hold any characters (`Chapter 1: The Inn`, `John's Long Johns`); the file gets a name that works everywhere.
- At the end of each note, **Linked from** lists the notes that link to it, so an NPC shows every quest, place, and journal entry they turn up in.

**Reading and writing**

- **Read** (Ctrl+R) shows a note formatted, the way GitHub shows a README: its name as a title, its fields in a box on the right like a wiki's infobox, then headings, lists, tables, quotes, colored code, pictures (`![[map.png]]`, or from the web), and other notes embedded with `![[Note]]`.
- Edit right in the reading view: click a paragraph or a field to change it in place, or a task box to tick it.
- No need to know Markdown: a formatting toolbar makes headings, bold and italic text, lists, task lists, links, and pictures, and shows the Markdown it writes. If you do know it, type it: Enter carries lists on, Ctrl+B and Ctrl+I work, and the marks are dimmed so the words stand out.
- **Edit > Insert Picture**, or dropping a picture on the window, copies it into the campaign and embeds it.

**Dice**

- The d20 button at the right of the tab bar opens a dice roller: click a die, or type an expression like `2d6+3`, `d%`, or `4d6kh3`. Advantage and disadvantage work on a d20, or on any die for systems that allow it.
- Fields with dice in them, like Damage `1d8 + 3 slashing`, get a roll button in the form and in the reading view.

**Your files stay yours**

- Campaigns are plain files, so you can edit them in Obsidian or any text editor at the same time. The campaign folder is watched: notes changed, renamed, or deleted elsewhere show up right away, and if you have unsaved changes it asks which version to keep.
- Notes the app can't show as intended (broken front matter, a word in a number field, a broken link) get a ⚠ in the tree. Hover it to see what's wrong, and click it for a list of fixes.
- Pick a color theme in **View** (Dark or High Contrast), or make your own (see [Themes](#themes)).

## Why Dungeon Elements?

Game Masters pour years into their campaigns. That work shouldn't be locked inside a proprietary format or someone else's server.

- **📝 Plain Markdown.** Every note, NPC, and location is a readable `.md` file you can open in any text editor.
- **💾 Local only.** No accounts and no cloud saves. Your campaign lives in a folder on your drive, and you back it up however you like.
- **🚪 No lock-in.** If you stop using Dungeon Elements, your files come with you, untouched.
- **🦀 Pure Rust.** A native desktop app built with [iced](https://iced.rs), aiming to be fast, stable, and secure.
- **🎲 Any system.** System-agnostic: not tied to D&D, Shadowdark, or any other game. Built for GMs and solo players on Windows and Linux.

## How it works

Each campaign is just a folder. Open the folder and your campaign is there:

```text
my-campaign/
├── locations/
│   └── the-sunken-keep.md
├── npcs/
│   └── mira-the-innkeeper.md
├── players/
├── quests/
├── items/
├── journal/
└── lore/
```

Every note is a Markdown file with its fields in YAML front matter, and `[[links]]` tie them together the way Obsidian does, so the same folder opens in Obsidian too. Over time, your notes grow into a connected wiki of your world.

## Roadmap

| Phase | Name | Goals |
| :---: | --- | --- |
| **0** ✅ | _Call to Action_ (v0.1) | Basic iced GUI · write, save, and open Markdown · one folder per campaign · YAML metadata · detect outside edits |
| **1** ✅ | _Inciting Incident_ (v0.2) | Reading view · Obsidian-style `[[links]]` that survive renames · backlinks · formatting toolbar · built-in dice roller |
| **2** | _Rising Action_ | Basic SVG map editor · name and roll table generators |

Dungeon Elements is **not** a virtual tabletop and has no multiplayer sync. It's for building and tracking campaigns, not hosting them.

### Planned features

- 🧙 **Player management:** character sheets, backstories, and player notes
- 👥 **NPC management**
- 🗺️ **Map generators:** world, region, city, building, and dungeon (later in development)
- 📜 **Quest tracker:** build quests and get story suggestions for inspiration (suggestions, not generated stories)
- 📚 **World and lore wiki:** your Markdown files as a browsable wiki, linked to your maps
- 🖨️ **PDF export:** player handouts and table maps with a square, hex, or blank overlay
- 📓 **Solo journaling:** a side-by-side view of your journal with lore, rules, or roll tables

See [PLAN.md](PLAN.md) for the full plan and checklists.

## Themes

Every color in the app comes from a theme. To make your own, copy [`themes/dark.yaml`](themes/dark.yaml), change its `name` and colors, and save it in your themes folder:

- Windows: `%APPDATA%\dungeon_elements\themes`
- Linux and macOS: `~/.config/dungeon_elements/themes`

Then choose **View > Reload Themes** and pick it from the **View** menu. Any color you leave out comes from Dark (`base: dark`) or High Contrast (`base: light`). **Help > About** shows where your themes folder is.

## Download

Prebuilt Windows, Linux, and macOS builds are on the [Releases](https://github.com/NightMarethyu/dungeon_elements/releases) page. Unzip and run `dungeon_elements` (on a Mac, `Dungeon Elements.app`); there's no installer. The Mac build isn't signed with an Apple Developer ID, so the first time, allow it in **System Settings > Privacy & Security > Open Anyway**. You can also pass a campaign folder to open it right away: `dungeon_elements path/to/my-campaign`.

## Building

You need a recent stable [Rust toolchain](https://rustup.rs) (edition 2024, Rust 1.88 or newer).

```sh
cargo run
```

For an optimized build, run `cargo build --release`; the app is `target/release/dungeon_elements` (`.exe` on Windows). It is a single file with the themes and templates built in. [RELEASING.md](RELEASING.md) has the steps for packaging a release.

On Windows, Rust uses the Microsoft C++ linker, so install the **Desktop development with C++** workload (MSVC build tools and a Windows SDK) from the Visual Studio Installer.

On Linux, building needs only a C compiler (`build-essential` on Debian and Ubuntu). Running the app needs the usual desktop libraries, which desktop distros already have; on a minimal system or WSL, install them first (on Ubuntu: `sudo apt install libwayland-client0 libwayland-cursor0 libwayland-egl1 libxkbcommon0 libxkbcommon-x11-0 libegl1 libgl1`).

## Contributing

The project is just getting started. Ideas and feedback are welcome, so feel free to [open an issue](https://github.com/NightMarethyu/dungeon_elements/issues).

## License

Released under the [MIT License](LICENSE).
