<div align="center">

# 🐉 Dungeon Elements

**A local-first campaign manager for tabletop RPGs, built in Rust.**

Write your world in plain Markdown, link it together like a wiki, and keep every file on your own drive.

**[Website](https://nightmarethyu.github.io/dungeon_elements/) · [Download](https://github.com/NightMarethyu/dungeon_elements/releases/latest) · [Send feedback](https://forms.gle/BvS3x6pUP55mdYxt7)**

![Version: 0.3.0](https://img.shields.io/badge/version-0.3.0-blue)
![Status: early development](https://img.shields.io/badge/status-early%20development-orange)
![Language: Rust](https://img.shields.io/badge/language-Rust-b7410e?logo=rust)
![License: MIT](https://img.shields.io/badge/license-MIT-blue)

</div>

---

> [!NOTE]
> Dungeon Elements is in **early development**. **Version 0.3.0** is out, and Phases 0, 1 and 2 of the [plan](PLAN.md) are done: you can build a campaign as a wiki of linked notes, fill in forms instead of writing Markdown, let a walkthrough and roll tables help you invent your world, and track quests and dice beside it. Keep backups of campaigns you care about. _"Dungeon Elements" is a working title._

## Who it's for

Game Masters, solo players, and writers building a world. You don't need to know anything about code, Markdown or YAML: everything can be done from the app's menus, forms and buttons. If you do like plain text, your files are always there to open in Obsidian or any text editor.

## What it does

**Campaigns and notes**

- Open a campaign folder, or make a new one. **New Campaign** starts from the starter folders (Campaign, Sessions, Players, Quests, NPCs, Factions, Locations, Lore and Items, with subfolders like Enemies, Settlements and Weapons), an empty folder, or a template of your own. **Open Campaign** lists the campaigns you've used, newest first.
- The file tree on the left lists your folders and notes, and each note opens in a tab. Right-click anything to rename, move, or delete it (deleted notes go to the Recycle Bin or Trash), or drag a note onto another folder to move it.
- Each folder can give its notes fields, so an NPC opens as a form with a Name, Location, Wants and Status, and an item with its Value and Rarity. Fields can be text, numbers, dates, tags, dice, checklists of steps, links to other notes, or a roll table. **Campaign > Note Types...** changes a folder's fields, and the round **+** on a form adds a field to just one note.
- Subfolders are kinds of their folder: Weapons in Items has every Item field plus its own Damage.
- Share how your campaign is set up: **Campaign > Export Campaign Structure...** saves its folders and fields, without any notes, as a template others can import.

**Build your world with the walkthrough**

- **Campaign > Campaign Walkthrough...** asks you about your world one question at a time, in seven chapters: the campaign, the world, power, the extraordinary, where it starts, the heroes and the first session.
- Each answer goes into a note, and the notes it needs are made and linked for you: name the factions and each gets a note, name a faction's leader and an NPC is made.
- **Inspire Me** rolls an idea for any question when you're stuck. **Campaign > Walkthrough Questions...** lets you change, add or remove questions, with no files to edit.

**Roll tables and Inspire Me**

- Dozens of built-in tables to spark ideas: names from many cultures (for women and men, or mixed), fantasy and place names, inns and shops, groups, story prompts like Wants, Trouble, Twists and Tropes, and tables for worlds and adventures, like Faiths, Wonders, Rumors, Plot hooks and Secrets. They're written to fit any genre.
- Fields like an NPC's Name or a quest's Hook have an **Inspire Me** button that fills them from a table. Click again for another idea.
- Make your own tables in an editor that looks like a spreadsheet: pick the die, add rows, and it works out the ranges and shows each row's chance. Paste a list from a web page or a PDF to turn it into a table. Tables can roll other tables, like a given name then a surname.
- The **Tables** menu and the Roll Tables list (Ctrl+Shift+T) roll any table. Results are kept in the dice pane, ready to copy or turn into a new note.
- Tables that aren't for rolling, like price lists or calendars, can be edited the same way.

**Links, like a wiki**

- Type `[[` and pick a note to link to it, or select some text and press Ctrl+K. Ctrl+click a link to open the note; a link to a note that doesn't exist yet makes it.
- Rename a note, in the app or outside it, and every link to it follows. Notes can have aliases, so the villain can also go by their nickname.
- At the end of each note, **Linked from** lists every note that mentions it, so an NPC shows each quest, place and session they turn up in.
- **Edit > Find in Notes** (Ctrl+Shift+F) searches note names and text as you type, and can find notes by a field, like `status:alive` or `location:Saltmarsh`.

**Reading and writing**

- **Read** (Ctrl+R) shows a note formatted like a wiki page, with its fields in a box on the right, pictures, tables, and other notes embedded in it. Click a paragraph or a field to change it right there.
- No need to know Markdown: a formatting toolbar makes headings, bold and italic text, lists, task lists, links and pictures. If you do know it, just type it.
- **Undo** (Ctrl+Z) and **Redo** (Ctrl+Y) work on your notes and on the campaign itself: a rename, a move, a deleted folder, or a changed field.

**Quests**

- **View > Quests** (Ctrl+Shift+Q) lists your quests by status, each with a progress bar. Tick off steps right in the list.
- A quest's steps can link to other quests, so quest lines show as a tree.

**Dice**

- Open the dice roller (Ctrl+Shift+D) and click a die, or type an expression like `2d6+3`, `d%` or `4d6kh3`. Advantage and disadvantage work on any die.
- Fields with dice in them, like Damage `1d8 + 3 slashing`, get a roll button in the form and in the reading view.

**Help built in**

- **Help > How-To Guides** (F1) has short guides for everything above, **Help > Keyboard Shortcuts** (Ctrl+/) lists every shortcut, and **Help > What's New** shows what changed in your version.

**Your files stay yours**

- Campaigns are plain files, so you can edit them in Obsidian or any text editor at the same time. Changes made outside the app show up right away, and if you have unsaved changes it asks which version to keep.
- Notes the app can't show as intended (a broken link, a word in a number field) get a ⚠ in the file tree. Hover it to see what's wrong, and click it for ways to fix it.
- Pick a look in **View > Theme**: System (follows your computer's light or dark setting), Light (cream, like old parchment), Dark or High Contrast. You can also make your own (see [Themes](#themes)).

## Why Dungeon Elements?

Game Masters and writers pour years into their worlds. That work shouldn't be locked inside a proprietary format or someone else's server.

- **📝 Plain Markdown.** Every note, NPC, and location is a readable `.md` file you can open in any text editor.
- **💾 Local only.** No accounts and no cloud saves. Your campaign lives in a folder on your drive, and you back it up however you like.
- **🚪 No lock-in.** If you stop using Dungeon Elements, your files come with you, untouched.
- **🎲 Any system.** Not tied to D&D, Shadowdark, or any other game, and useful for a novel's world too.
- **🦀 Pure Rust.** A native desktop app for Windows, Linux and macOS, built with [iced](https://iced.rs), aiming to be fast, stable, and secure.

## How it works

Each campaign is just a folder. Open the folder and your campaign is there:

```text
my-campaign/
├── campaign/
├── sessions/
│   └── session-1.md
├── players/
├── quests/
├── npcs/
│   ├── enemies/
│   └── mira-the-innkeeper.md
├── factions/
├── locations/
│   ├── settlements/
│   ├── sites/
│   │   └── the-sunken-keep.md
│   └── ...
├── lore/
└── items/
```

Every note is a Markdown file with its fields at the top, and `[[links]]` tie them together the way Obsidian does, so the same folder opens in Obsidian too. Over time, your notes grow into a connected wiki of your world.

## Roadmap

| Phase | Name | Goals |
| :---: | --- | --- |
| **0** ✅ | _Call to Action_ (v0.1) | Write, save, and open notes · one folder per campaign · fields in each note · notice changes made in other apps |
| **1** ✅ | _Inciting Incident_ (v0.2) | Reading view · wiki-style `[[links]]` that survive renames · backlinks · formatting toolbar · dice roller |
| **2** ✅ | _Rising Action_ (v0.3) | Note types and forms · roll tables, name generators and Inspire Me · quests · campaign walkthrough · search · templates · Undo and Redo |
| **3** 🚧 | _Climax_ (v0.4) | Panes you can resize, move and collapse, like VS Code · saved layouts · a journal for solo play, GMs and authors · timers · a focus mode for writing · pop-out windows in v0.4.1 |
| **4** | _Falling Action_ | Map editor · maps linked to notes · map generators |
| **5** | _Resolution_ | Built-in templates for game systems and novels · reusable settings · PDF rulebook library · player handouts |

Coming first in v0.4: your own roll buttons that roll several tables at once, a theme editor (with a neon green theme), a progress bar for the walkthrough, and **Add to note** for dice and table results.

Dungeon Elements is **not** a virtual tabletop and has no multiplayer sync. It's for building and tracking campaigns and worlds, not hosting games.

See [PLAN.md](PLAN.md) for the full plan and checklists, and [CHANGELOG.md](CHANGELOG.md) for what changed in each version.

## Themes

Every color in the app comes from a theme. To make your own, copy [`themes/light.yaml`](themes/light.yaml) or [`themes/dark.yaml`](themes/dark.yaml), change its `name` and colors, and save it in your themes folder:

- Windows: `%APPDATA%\dungeon_elements\themes`
- Linux and macOS: `~/.config/dungeon_elements/themes`

Then choose **View > Theme > Reload Themes** and pick it from **View > Theme**. Any color you leave out comes from Dark (`base: dark`) or High Contrast (`base: light`). **Help > About** shows where your themes folder is.

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

## Feedback and ideas

Ideas, questions and problem reports are all welcome, and you don't need a GitHub account to send them:

- **[Send feedback with a short form](https://forms.gle/BvS3x6pUP55mdYxt7)**: the easiest way, no account needed.
- **[Open an issue on GitHub](https://github.com/NightMarethyu/dungeon_elements/issues)**, or choose **Help > Report a Problem...** in the app.

## License

Released under the [MIT License](LICENSE).
