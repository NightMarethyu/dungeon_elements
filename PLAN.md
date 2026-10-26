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
- [x] Save folder structure for campaigns
  - campaigns have locations, quests, npcs, players, items, lore, etc.
- [x] Different folders represent different campaigns, GM can open a folder and their campaign is there
- [x] Real menu bar (File, Edit, View, Help) with the logo, replacing the button row
- [x] Customizable color themes (see [Themes](#themes)), Dark by default
- [x] View menu toggles: show file extensions, show `_type.yaml` files (editable in the app's text editor)
- [x] YAML metadata for formatting and to ensure links and other data work consistently
  - [x] Folder content types defined in `_type.yaml` (see [Content types](#content-types))
  - [x] Notes in a typed folder open as a form; raw Markdown is one toggle away (View > Show Markdown Source, Ctrl+E)
  - [x] New campaigns get a `_type.yaml` in each starter folder (from `templates/default/`)
  - [x] + on a folder makes a new note of its type
  - [x] + in the main pane adds a field to that note only (stored as an extra front matter key or `##` section; extra keys and sections added in Obsidian show as fields too)
  - [x] Any field except the name can be removed from a single note (listed in the note's `removed_fields`); the main pane's + brings it back
  - [x] Big + in the sidebar makes a new folder, offering a default type template
  - [x] Edit > Types dialog: folders on the left, their type's fields on the right; add, remove and reorder fields, set their name, key, kind, width and options, and give an untyped folder a type (saving rewrites `_type.yaml` without its comments)
- [x] Check files on load for edits outside software, warn of improper formatting
  - [x] Opening or refreshing a campaign checks every note and `_type.yaml`: broken front matter, values that don't fit their field (a word in a number field, a choice that isn't an option), longtext fields stored in the front matter, repeated sections, unknown `type:`s, and nested YAML the form can't show. Files with problems get a ⚠ in the tree, and the status bar's ⚠ lists them; nothing is changed
  - [x] Coming back to the window notices files changed in another app: open tabs without unsaved changes reload, and tabs with unsaved changes ask whether to reload or keep your version. Saving never overwrites an outside edit without asking

### Phase 1: inciting incident

- [x] Display markdown files in a nice way
  - [x] Reading view (Edit / Read at the top right, Ctrl+R): the note's name as a title, its fields in a wiki-style box on the right half beside the opening text, then the body as formatted Markdown with headings, lists, task lists, tables, quotes, code blocks, rules and pictures, like GitHub shows a README
  - [x] `[[Links]]` in it can be clicked (dimmer when the note doesn't exist yet), web links open in the browser, and `![[picture.png]]` embeds show the picture
  - [x] View > Open Notes in Reading View, for opening notes formatted
  - [ ] Pictures from the web, syntax colors in code blocks, and embedding one note in another (`![[Note]]` shows as a link for now)
- [x] Allow internal linking of files, make this easy highlight text and select "add link" (right-click context menu or file menu) will check if a file already exists with that name and link it, if a file doesn't exist it will create one. Links formatted in Obsidian compatible [[link text]] format
  - [x] Typing `[[` in a text editor (or a long text field) closes it with `]]` and suggests notes by name; Tab or a click inserts one. Link fields suggest notes the same way (Enter picks the first)
  - [x] Edit > Insert Link (Ctrl+K) wraps the selected text in `[[ ]]`
  - [x] Links are colored in the editor, dimmer when no note has that name yet
  - [x] Ctrl+click a link (or use the bar under the editor, or a link field's Open button) to open the note. A link to a missing note creates it, in the campaign folder or the folder the link's path names
  - [x] Obsidian's forms work: `[[Note|shown text]]`, `[[Note#Heading]]`, `![[embeds]]`, `[[folder/Note]]` for notes that share a name. Links are matched without case, and when names clash the same folder wins, like in Obsidian
  - [x] Link fields link to a folder (Edit > Types, "Links to"; the starter types set it, and a field named like a type, such as Location, finds that type's folder). The folder's notes are suggested first, and a name with no note offers to make it there, as a note of that folder's type
  - [ ] Right-click menu (iced has no context menus yet)
- [x] Ensure name safe links, if filename changes, links are robust enough to stay in place
  - [x] Renaming a note in the app (changing its name and saving) rewrites every link to it across the campaign: in note bodies, front matter fields, and open tabs, unsaved changes included. Shown text and headings are kept
  - [x] Links to other notes that a rename would make ambiguous are rewritten as paths, so they keep pointing where they did
  - [x] Drag a note in the file tree onto a folder (or a note in it) to move it there. It takes the folder's type and keeps every field that has something in it; fields the new type doesn't have become the note's own. Links that named its old folder are rewritten
  - [ ] Notice renames done outside the app (in Explorer or Obsidian with link updating off) and offer to fix the links that broke
- [ ] Syntax-highlighting and live preview
- [x] Built-in dice roller, keep everything close without needing to leave the app
  - [x] The d20 button at the right of the tab bar opens a pane along the bottom of the main pane (`reference/dungeon_elements_layout_plan_with_roll_results.png`): the result on the left, and a button for each die (d20, d12, d10, d100, d8, d6, d4) on the right
  - [x] Each die has a Count box; clicking the die (or pressing Enter in its box) rolls that many and shows each result, their total, and the few rolls before
  - [ ] Dice expressions like `2d6+3`, advantage and disadvantage, and rolling from a note's fields

Stretch goal:

- [ ] Add editor features to make Markdown more intuitive (WYSIWYG)

### Phase 2: rising action

- [ ] Add basic SVG editor for maps
- [ ] Add tools for generators
  - [ ] name generators
  - [ ] roll table generators

## Design

The layout plan is in `reference/dungeon_elements_layout_plan.png` and the logo in `reference/dungeon_elements.svg`. The reference is drawn in black and white for contrast only; the app is themed. The colored logo, `reference/dungeon_elements_colors.svg`, is the app icon (window, taskbar, and the Windows `.exe`, from `assets/dungeon_elements.ico`).

### Themes

- A theme is a YAML file that sets a color for each named role (background, sidebar, menu bar, tabs, borders, text, fields, accent, status bar, and so on). Any role left out falls back to the theme's `base` (dark or light).
- Built-in themes ship with the app (the YAML lives in `themes/`): Dark (the default) and High Contrast (the reference drawing). More built-in themes later.
- User themes are YAML files in the app's config folder (`%APPDATA%\dungeon_elements\themes` on Windows, `~/.config/dungeon_elements/themes` on Linux) and appear under View > Theme. A broken theme file is skipped with a warning, never a crash.
- The logo is recolored to match the theme (black becomes the text color, white the background).
- Later: a theme editor with a color picker per role and live preview.

### Layouts

Customization is a big part of the app, so the window's layout should be the user's to arrange too. Not started; for later in development.

- Panes (the file tree, the editor, the dice roller, and later maps, roll tables and the journal) can pop out into their own windows, and be put back.
- The user chooses where each pane appears in the main window (left, right, bottom, or as a tab), and resizes it.
- An arrangement can be saved as a named layout (for example "Prep" and "At the table") and switched from the View menu. Layouts are saved in the app's config folder, like themes, so they can be shared.
- The default layout is the one in `reference/dungeon_elements_layout_plan.png`.

### Content types

- Each folder can hold a `_type.yaml` that defines the fields every note in that folder has. One type per folder.
- Field kinds: `text`, `number`, `choice` (with `options`), `tags`, `link` (an Obsidian `[[link]]`), `checkbox`, and `longtext`.
- Short fields are stored in the note's YAML front matter. `longtext` fields are stored as `## Label` sections in the Markdown body, so notes stay readable in Obsidian.
- The note's front matter records `type:`, so a note moved to another folder still knows its type.
- The `name` field is the filename (`The One Ring.md`). The tree shows names without `.md`, like Windows Explorer, unless View > Show File Extensions is on.
- Front matter keys and `##` sections the type doesn't know about become fields of that note only (the main pane's + adds one; each has a Remove button). Values that can't be a field (nested lists or maps) and text before the first section are kept and shown in an "Other" area. Saving never drops data added in Obsidian or another editor.
- Any field except `name` can be removed from a single note: the note's front matter lists it under `removed_fields`, and the main pane's + offers to bring it back.
- A folder with no `_type.yaml` is a plain Markdown folder.
- Later:
  - Subfolders inherit their parent's type unless they have their own `_type.yaml`.
  - A `steps` field kind: an ordered checklist where each step is text or a `[[link]]` to another note. The Quest type uses it so a step can be another quest, building quest lines (a Big Quest made of smaller quests).
  - Types can be saved for reuse in other campaigns from the Edit > Types dialog.
  - Campaign templates: New Campaign offers templates (e.g. D&D, Shadowdark, Cyberpunk) that set up folders and types. Templates are shareable files, so GMs can build their own and give them to friends.

## Planned features

- Player management features
  - save all the player information, notes, character sheets, backstory, etc.
- NPC management
- Map generator (world, region, city, building, and dungeon)
  - save map generators for later in development
- Quest Tracker
  - build quests
  - quests have steps or actions, and a step can be another quest, so a big quest is made of smaller quests (quest lines)
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
