# PLAN.md - Full Campaign Builder

Dungeon Elements _working title_

## What it is

Dungeon Elements is a full campaign building suite for Table Top Role Playing Games. The files are saved and formatted in Markdown and they live on the user's hard drive. No cloud saves (unless the user uses cloud backups). If the software no longer works or the user wants to switch to different software, they can take their markdown files with them.

The suite will also include drawing tools and generators to allow the Game Master to build out worlds, cities, and dungeons. These will enable the GM to build completely custom from scratch locations to generated on the fly because the players did very unexpected things.

## Constraints

- Built in pure Rust (GUI in iced), for security and stability
- Output for files are common, readable text files and editable image files (probably SVG, but image filetypes are to be determined)
- Everything stays local, the user owns what they create and it all stays on their drive.
- No VTT, this is for building and keeping track of campaigns, not for hosting them
- No multiplayer sync, this is a tool for GMs and solo play, not for everyone to have everything together
- Windows and Linux, maybe MacOS but I don't have a system to build on
- System agnostic, not tied to D&D, Shadowdark, etc. Any System, any playstyle

## Phases

### Phase 0: call to action

- [x] Basic GUI, built in pure Rust (using iced, everything in Rust, everything local)
- [x] Able to input text
- [x] Save text, stored in a folder in Markdown format
- [x] Open saved text files
- [ ] Save folder structure for campaigns
  - campaigns have locations, quests, npcs, players, items, lore, etc.
- [ ] Different folders represent different campaigns, GM can open a folder and their campaign is there
- [ ] YAML metadata for formatting and to ensure links and other data work consistently
- [ ] Check files on load for edits outside software, warn of improper formatting

### Phase 1: inciting incident

- [ ] Display markdown files in a nice way
- [ ] Allow internal linking of files, make this easy highlight text and select "add link" (right-click context menu or file menu) will check if a file already exists with that name and link it, if a file doesn't exist it will create one. Links formatted in Obsidian compatible [[link text]] format
- [ ] Ensure name safe links, if filename changes, links are robust enough to stay in place
  - rename links to protect Obsidian compatibility
  - Possibly save as part of YAML metadata
- [ ] Syntax-highlighting and live preview
- [ ] Built-in dice roller, keep everything close without needing to leave the app

Stretch goal:

- [ ] Add editor features to make Markdown more intuitive (WYSIWYG)

### Phase 2: rising action

- [ ] Add basic SVG editor for maps
- [ ] Add tools for generators
  - [ ] name generators
  - [ ] roll table generators

## Planned features

- Player management features
  - save all the player information, notes, character sheets, backstory, etc.
- NPC management
- Map generator (world, region, city, building, and dungeon)
  - save map generators for later in development
- Quest Tracker
  - build quests
  - suggest story outlines (don't generate them, but offer suggestions for inspiration and building things out)
- World and Lore management
  - This is the wiki-interface of the markdown files
  - allow for linking between displayed maps and the world wiki
- PDF generation
  - player handouts
  - table maps
    - GM can choose the overlay (squares, hexes, none)
- Journaling tool
  - Built for solo-play
  - has dice rollers built in to let the player do checks without leaving the screen
  - side-by-side view with journal and lore or rules or roll table
- More to come as I have more ideas!
