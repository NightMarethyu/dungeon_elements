<div align="center">

# 🐉 Dungeon Elements

**A local-first campaign manager for tabletop RPGs, built in Rust.**

Write your world in plain Markdown, link it together like a wiki, and keep every file on your own drive.

![Status: early development](https://img.shields.io/badge/status-early%20development-orange)
![Language: Rust](https://img.shields.io/badge/language-Rust-b7410e?logo=rust)
![License: MIT](https://img.shields.io/badge/license-MIT-blue)

</div>

---

> [!NOTE]
> Dungeon Elements is in **early development**. Right now the app is a single text editor: you can write notes and save them as a Markdown (`.md`) file with **Save**, **Save As...**, or Ctrl+S. The roadmap lives in [PLAN.md](PLAN.md). _"Dungeon Elements" is a working title._

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

Highlight a name in your notes and choose **Add link** to create an Obsidian-compatible `[[link]]`. If a page with that name exists, it gets linked; if not, a new one is created for you. Over time, your notes grow into a connected wiki of your world.

## Roadmap

| Phase | Name | Goals |
| :---: | --- | --- |
| **0** | _Call to Action_ | Basic iced GUI · write, save, and open Markdown · one folder per campaign · YAML metadata · detect outside edits |
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

## Building

You need a recent stable [Rust toolchain](https://rustup.rs) (edition 2024, Rust 1.85 or newer).

```sh
cargo run
```

On Windows, Rust uses the Microsoft C++ linker, so install the **Desktop development with C++** workload (MSVC build tools and a Windows SDK) from the Visual Studio Installer.

On Linux, iced also needs the usual windowing libraries (for example `libxkbcommon-x11` and Wayland or X11 development packages) installed through your distro.

## Contributing

The project is just getting started. Ideas and feedback are welcome, so feel free to [open an issue](https://github.com/NightMarethyu/dungeon_elements/issues).

## License

Released under the [MIT License](LICENSE).
