<div align="center">

# 🐉 Dungeon Elements

**A local-first campaign manager for tabletop RPGs, built in Rust.**

Write your world in plain Markdown, link it together like a wiki, and keep every file on your own drive.

![Status: planning](https://img.shields.io/badge/status-planning-orange)
![Language: Rust](https://img.shields.io/badge/language-Rust-b7410e?logo=rust)
![License: MIT](https://img.shields.io/badge/license-MIT-blue)

</div>

---

> [!NOTE]
> Dungeon Elements is in the **planning stage**. There is no code to build or run yet. The roadmap lives in [PLAN.md](PLAN.md). _"Dungeon Elements" is a working title._

## Why Dungeon Elements?

Game Masters pour years into their campaigns. That work shouldn't be locked inside a proprietary format or someone else's server.

- **📝 Plain Markdown.** Every note, NPC, and location is a readable `.md` file you can open in any text editor.
- **💾 Local only.** No accounts and no cloud saves. Your campaign lives in a folder on your drive, and you back it up however you like.
- **🚪 No lock-in.** If you stop using Dungeon Elements, your files come with you, untouched.
- **🦀 Built in Rust.** Aiming for a fast, stable, secure desktop app.

## How it will work

Each campaign is just a folder. Open the folder and your campaign is there:

```text
my-campaign/
├── npcs/
│   └── mira-the-innkeeper.md
├── locations/
│   └── the-sunken-keep.md
├── maps/
│   └── the-sunken-keep.svg
└── sessions/
    └── session-01.md
```

Highlight a name in your notes and choose **Add link**. If a page with that name exists, it gets linked; if not, a new one is created for you. Over time, your notes grow into a connected wiki of your world.

## Roadmap

| Phase | Name | Goals |
| :---: | --- | --- |
| **0** | _Call to Action_ | Basic Rust GUI · write and save Markdown · one folder per campaign |
| **1** | _Inciting Incident_ | WYSIWYG Markdown editing · polished file display · one-click internal links |
| **2** | _Rising Action_ | Basic SVG map editor · map, name, and roll table generators |

### Planned features

- 🧙 **Player management:** character sheets, backstories, and player notes
- 👥 **NPC management**
- 🗺️ **Map generators:** world, region, city, building, and dungeon
- 📜 **Quest tracker:** build quests and get story suggestions for inspiration (suggestions, not generated stories)
- 📚 **World and lore wiki:** your Markdown files as a browsable wiki, linked to your maps
- 🎲 **Solo journaling:** built-in dice rollers and a side-by-side view of your journal with lore, rules, or roll tables

See [PLAN.md](PLAN.md) for the full plan and checklists.

## Contributing

The project is just getting started. Ideas and feedback are welcome, so feel free to [open an issue](https://github.com/NightMarethyu/dungeon_elements/issues).

## License

Released under the [MIT License](LICENSE).
