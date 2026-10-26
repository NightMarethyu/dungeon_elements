<div align="center">

# 🐉 Dungeon Elements

**A local-first campaign manager for tabletop RPGs, built in Rust.**

Write your world in plain Markdown, link it together like a wiki, and keep every file on your own drive.

![Version: 0.1.0](https://img.shields.io/badge/version-0.1.0-blue)
![Status: early development](https://img.shields.io/badge/status-early%20development-orange)
![Language: Rust](https://img.shields.io/badge/language-Rust-b7410e?logo=rust)
![License: MIT](https://img.shields.io/badge/license-MIT-blue)

</div>

---

> [!NOTE]
> Dungeon Elements is in **early development**. When it starts, you can open a campaign folder or create a new one (optionally with a starter set of folders). Inside a campaign, a collapsible file tree on the left lists your folders and Markdown (`.md`) files, and each file you open gets its own tab. Everything is in the **File**, **Edit**, **View**, and **Help** menus, with shortcuts for **New Note** (Ctrl+N), **Open File...** (Ctrl+O), **Save** (Ctrl+S), **Save As...** (Ctrl+Shift+S), and **Close Tab** (Ctrl+W). **View** picks a color theme (Dark or High Contrast, or your own; see [Themes](#themes)) and can show file extensions and folders' `.yaml` type files.
>
> Folders can define the fields their notes have in a `_type.yaml` (new campaigns come with one in each starter folder). Notes in those folders open as a form, like Name, Value, History, and Stats for an item, and are saved as Markdown with YAML front matter that Obsidian can read. The **+** next to a folder makes a new note in it, the round **+** under the file tree makes a new folder (optionally with a note type), and the round **+** on a form adds a field to just that note. **Edit > Types** changes the fields of every note in a folder. **View > Show Markdown Source** (Ctrl+E) shows the file itself.
>
> Link notes together the Obsidian way: type `[[` and pick a note from the suggestions (Tab inserts the first), or select some text and press **Edit > Insert Link** (Ctrl+K). Links can show other text (`[[Mira|the smuggler]]`) or point to a heading (`[[Mira#Appearance]]`). Ctrl+click a link, or use the bar under the editor, to open the note; a link to a note that doesn't exist yet makes it. Link fields point at a folder, so typing a new place in an NPC's Location offers to make it in your locations folder. Renaming a note (by changing its Name and saving) rewrites every link to it across the campaign, including links in fields and in files you have open. Drag a note onto another folder in the file tree to move it; it takes that folder's type and keeps what you filled in.
>
> Press **Read** at the top right (or Ctrl+R) to see a note formatted, the way GitHub shows a README: its name as a title, its fields in a box on the right like a wiki, then headings, lists, tables, quotes, colored code, pictures (`![[map.png]]`, or from the web), and other notes embedded with `![[Note]]`. **Edit > Insert Picture** adds a picture from anywhere on your computer. Click a link there to read the note it points to. **View > Open Notes in Reading View** makes notes open that way. You can edit right there: click a paragraph or a field to change it in place, and it's formatted again when you click away.
>
> Campaigns stay plain files, so you can edit them in Obsidian or any text editor. When a campaign opens, Dungeon Elements checks its notes and marks any it can't show as intended with a ⚠ (the ⚠ in the status bar lists what's wrong). When you switch back to the app, open files that changed elsewhere are reloaded, and if you have unsaved changes it asks which version to keep. The roadmap lives in [PLAN.md](PLAN.md). _"Dungeon Elements" is a working title._

## Why Dungeon Elements?

Game Masters pour years into their campaigns. That work shouldn't be locked inside a proprietary format or someone else's server.

- **📝 Plain Markdown.** Every note, NPC, and location is a readable `.md` file you can open in any text editor.
- **💾 Local only.** No accounts and no cloud saves. Your campaign lives in a folder on your drive, and you back it up however you like.
- **🚪 No lock-in.** If you stop using Dungeon Elements, your files come with you, untouched.
- **🦀 Pure Rust.** A native desktop app built with [iced](https://iced.rs), aiming to be fast, stable, and secure.
- **🎲 Any system.** System-agnostic: not tied to D&D, Shadowdark, or any other game. Built for GMs and solo players on Windows and Linux.

## How it will work

Each campaign is just a folder. Open the folder and your campaign is there:

```text
my-campaign/
├── locations/
│   ├── the-sunken-keep.md
│   └── the-sunken-keep.svg
├── npcs/
│   └── mira-the-innkeeper.md
├── players/
├── quests/
├── items/
└── lore/
```

Type `[[` and pick a note, or select a name in your notes and press Ctrl+K, to create an Obsidian-compatible `[[link]]`. If a page with that name exists, it gets linked; if not, opening the link creates it for you. Rename a page and every link to it follows. Over time, your notes grow into a connected wiki of your world.

## Roadmap

| Phase | Name | Goals |
| :---: | --- | --- |
| **0** ✅ | _Call to Action_ (v0.1) | Basic iced GUI · write, save, and open Markdown · one folder per campaign · YAML metadata · detect outside edits |
| **1** | _Inciting Incident_ | Polished file display · Obsidian-style `[[links]]` that survive renames · WYSIWYG editing · built-in dice roller |
| **2** | _Rising Action_ | Basic SVG map editor · name and roll table generators |

Dungeon Elements is **not** a virtual tabletop and has no multiplayer sync. It's for building and tracking campaigns, not hosting them.

### Planned features

- 🧙 **Player management:** character sheets, backstories, and player notes
- 👥 **NPC management**
- 🗺️ **Map generators:** world, region, city, building, and dungeon (later in development)
- 📜 **Quest tracker:** build quests and get story suggestions for inspiration (suggestions, not generated stories)
- 📚 **World and lore wiki:** your Markdown files as a browsable wiki, linked to your maps
- 🖨️ **PDF export:** player handouts and table maps with a square, hex, or blank overlay
- 📓 **Solo journaling:** built-in dice rollers and a side-by-side view of your journal with lore, rules, or roll tables

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
