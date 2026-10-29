# PLAN.md - Full Campaign Builder

Dungeon Elements _working title_

## What it is

Dungeon Elements is a full campaign building suite for Table Top Role Playing Games. The files are saved and formatted in Markdown and they live on the user's hard drive. No cloud saves (unless the user uses cloud backups). If the software no longer works or the user wants to switch to different software, they can take their markdown files with them.

The suite will also include drawing tools and generators to allow the Game Master to build out worlds, cities, and dungeons. These will enable the GM to build completely custom from scratch locations to generated on the fly because the players did very unexpected things.

TTRPGs stay the focus, but nothing about the app should be locked to them. With campaign templates (see [Campaign templates](#campaign-templates)), the same tool can hold a novel's world, a setting bible, or any other world-building notes, in the spirit of tools like Campfire.

## Constraints

- Built in pure Rust (GUI in iced), for security and stability
- Output for files are common, readable text files and editable image files (probably SVG, but image filetypes are to be determined)
- Everything stays local, the user owns what they create and it all stays on their drive.
- No VTT, this is for building and keeping track of campaigns, not for hosting them
- No multiplayer sync, this is a tool for GMs and solo play, not for everyone to have everything together
  - There should be an export for GMs to give "handouts" to their players that work within this system as well
- Windows and Linux, GitHub actions builds the MacOS version
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
  - [x] - on a folder makes a new note of its type
  - [x] - in the main pane adds a field to that note only (stored as an extra front matter key or `##` section; extra keys and sections added in Obsidian show as fields too)
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
  - [x] Pictures from the web, syntax colors in code blocks, and embedding one note in another (`![[Note]]` and `![[Note#Heading]]` show the note's text in a box). Edit > Insert Picture (or dropping a picture on the window) copies a picture from anywhere on the computer into the campaign's `pictures` folder and embeds it
- [x] Allow internal linking of files, make this easy highlight text and select "add link" (right-click context menu or file menu) will check if a file already exists with that name and link it, if a file doesn't exist it will create one. Links formatted in Obsidian compatible [[link text]] format
  - [x] Typing `[[` in a text editor (or a long text field) closes it with `]]` and suggests notes by name; Tab or a click inserts one. Link fields suggest notes the same way (Enter picks the first)
  - [x] Edit > Insert Link (Ctrl+K) wraps the selected text in `[[ ]]`
  - [x] Links are colored in the editor, dimmer when no note has that name yet
  - [x] Ctrl+click a link (or use the bar under the editor, or a link field's Open button) to open the note. A link to a missing note creates it, in the campaign folder or the folder the link's path names
  - [x] Obsidian's forms work: `[[Note|shown text]]`, `[[Note#Heading]]`, `![[embeds]]`, `[[folder/Note]]` for notes that share a name. Links are matched without case, and when names clash the same folder wins, like in Obsidian
  - [x] Link fields link to a folder (Edit > Types, "Links to"; the starter types set it, and a field named like a type, such as Location, finds that type's folder). The folder's notes are suggested first, and a name with no note offers to make it there, as a note of that folder's type
  - [x] Right-click menus (iced has none, so the app draws its own): the file tree (open, rename, move, delete to the Recycle Bin / Trash, new note or folder here, edit the folder's fields, copy a link, show in Explorer), tabs (save, close, close others / to the right / all), text editors (cut, copy, paste, open the link under the mouse, insert a link or picture), forms and the reading view. Renaming or moving a folder updates the links to its notes and the link fields that pointed at it
- [x] Ensure name safe links, if filename changes, links are robust enough to stay in place
  - [x] Renaming a note in the app (changing its name and saving) rewrites every link to it across the campaign: in note bodies, front matter fields, and open tabs, unsaved changes included. Shown text and headings are kept
  - [x] Links to other notes that a rename would make ambiguous are rewritten as paths, so they keep pointing where they did
  - [x] Drag a note in the file tree onto a folder (or a note in it) to move it there. It takes the folder's type and keeps every field that has something in it; fields the new type doesn't have become the note's own. Links that named its old folder are rewritten
  - [x] Notice renames done outside the app (in Explorer or Obsidian with link updating off) and offer to fix the links that broke. The campaign folder is watched, so outside edits, renames and deletes show up right away; a note renamed outside keeps its tab and its links are updated. Links that look broken (a note spelled almost the same exists, or the note was renamed or deleted outside) get a ⚠ with a tooltip, and clicking it opens a dialog of fixes
- [x] Backlinks, so the notes read like a wiki: the reading view lists the notes that link to this one ("Linked from"), so an NPC shows the quests, locations and journal entries they appear in
- [x] Live preview (syntax highlighting was dropped: this isn't meant to be a code editor; see the lighter typing helpers below)
  - [x] Live preview in the reading view: click a block (paragraph, list, table, heading...) to edit its Markdown in place; it is formatted again when you click elsewhere or press Escape. Fields in the box, and the title, are edited the same way, and clicking under the note adds text
  - [x] Link suggestions while typing `[[` in a block (Tab or a click inserts one), and in a field in the box (Enter picks the first). Names, aliases and file names are ranked together, so whatever starts with what was typed comes first
  - [x] Toggling task boxes with a click: clicking a box in the reading view ticks or unticks it in the note (`- [ ]` to `- [x]`), in numbered lists too (`1. [ ]`)
- [x] Formatting without knowing Markdown, a lighter take on syntax highlighting and WYSIWYG: those who know Markdown type it, and those who don't use buttons that write it for them
  - [x] A formatting toolbar over the note while it's edited (in the Markdown source, the form, or a block of the reading view; View > Formatting Toolbar hides it): a dropdown for what the line is (Paragraph, Heading 1–4, Quote), Bold, Italic, Strikethrough, bulleted, numbered and task lists, Link and Picture. Each tooltip shows the Markdown it writes, and pressing a button again takes the format away
  - [x] Enter continues a list, task list or quote (`- `, `1. ` numbered on, `- [ ] `) and ends it on an empty item; Ctrl+B / Ctrl+I make the selection (or the word at the cursor) bold or italic
  - [x] Dim the Markdown marks (`#`, `**`, `[ ]`) in the editors so the words stand out: headings, emphasis, list and quote marks, task boxes, link brackets and addresses, table pipes, and code and front matter fences take the muted text color
- [x] Built-in dice roller, keep everything close without needing to leave the app
  - [x] The d20 button at the right of the tab bar opens a pane along the bottom of the main pane (`reference/dungeon_elements_layout_plan_with_roll_results.png`): the result on the left, and a button for each die (d20, d12, d10, d100, d8, d6, d4) on the right
  - [x] Each die has a Count box; clicking the die (or pressing Enter in its box) rolls that many and shows each result, their total, and the few rolls before
  - [x] Dice expressions like `2d6+3`, advantage and disadvantage, and rolling from a note's fields

Stretch goal:

- [ ] ~~Add editor features to make Markdown more intuitive (WYSIWYG).~~ Being done the lighter way above: a formatting toolbar instead of a full WYSIWYG editor

### Phase 2: rising action

The campaign's people and plots. Everything here is notes, types and links, built on Phase 1's wiki, so it needs no new kinds of panes.

- [x] Help menu improvements: a Help window (F1) with a search box, opened from the Help menu and the start screen
  - [x] Show all the keyboard shortcuts (Help > Keyboard Shortcuts, Ctrl+/). The list is in `src/help.rs`, and a test fails if a menu shows a shortcut that isn't in it
  - [x] Basic how-tos (Help > How-To Guides, F1): getting started, notes and fields, links, reading and formatting, dice, and your files. Each guide is a Markdown file in `help/en/` built into the app, so a new guide only takes a new file (see `help/README.md`); guides link to each other with `[text](guide:id)`
  - [x] What's New (this version's part of the changelog), Report a Problem (opens a GitHub issue), and a link to the project on GitHub
  - Grows with the app: each new feature adds its shortcuts to the list and a guide (or a section of one) when it needs explaining
- [x] Content types grow up (see [Content types](#content-types))
  - [x] Subfolders inherit their parent's type, like child classes: a subfolder with its own `_type.yaml` is a kind of its parent's type (Weapons in Items, Enemies in NPCs), with the parent's fields and its own, and can change or remove inherited fields for itself. Right-click a folder for **New Subfolder...** and a new note of each kind; Edit > Types shows the inherited fields
  - [x] A `steps` field kind: an ordered checklist where each step is text or a `[[link]]` to another note, saved as a `- [ ]` task list under its `## ` heading. Tab and Shift+Tab move between a form's fields
  - [x] A `dice` field kind: pick the die, how many, and modifiers (`+2`, advantage, disadvantage) instead of typing an expression, and roll it from the note (builds on Phase 1's dice expressions)
  - [x] Types can be saved for reuse in other campaigns from the Edit > Types dialog
    - **Export Type...** saves a folder's type, with what it inherits, as `<Name>.type.yaml`; saved types start new folders in any campaign. **Export Campaign Structure...** saves the folders and types as a template (pulled forward from Phase 5, at Ammon's ask, with Open Campaign listing known campaigns)
  - [x] Improve built-in fields (make Date fields that use the system date but can be overridden with an "in universe" date (like Star Trek's Stardate), Make tags selectable in addition to comma separated lists, add multiple linked notes in a single field (like a magic system might have a list of Users that are linked notes) add new fields to improve QoL)
    - Date fields pick from a calendar (or are plain text with the calendar off), tags have tag notes listing links to the tagged notes, and a "links to notes" field holds many links
  - [x] Edit > Types has an **Add New** button at the bottom of its list: a new type with a new folder for its notes
- [ ] Adjust the default folders and types: go over the starter folders (`templates/default/`) and each type's fields now that fields can do much more (dates, tags, links to notes, steps, dice, subfolders as kinds), and change what a new campaign starts with to match

**Player Management Excluded**
The items in Player Management are all things that will just be built as templates, these can be based off character sheets for different TTRPGs. These items are all already included as fields and types

- [x] ~~Player management~~
  - [x] ~~The players type holds everything about a player and their character: player notes, backstory, and a character sheet built from the type's fields (numbers, choices, tags), so any system's sheet can be made in Edit > Types~~
  - [x] ~~Fields with dice in them (like `1d8+2` or `+3`) can be rolled from the note (builds on Phase 1's dice expressions)~~
- [ ] NPC management
  - [x] Link fields for where an NPC lives, who they serve, and who they know; backlinks show where they appear
    - This was completed when we ensured links and backlinks were working properly
  - [ ] A status field (alive, dead, missing...) and filtering the folder by a field, so the GM can find "every NPC in this town"
    - This might just need to be the foundation of a search feature. The search wouldn't need to be fancy, but some sort of search would be useful
- [ ] Quest tracker
  - [x] The Quest type uses `steps`, so a step can be another quest, building quest lines (a big quest made of smaller quests)
  - [ ] A quest view: open quests and their progress, ticking off steps, and the quest line as a tree
- [ ] Generators
  - [ ] Roll tables as notes (a Markdown table with a die column), rolled from the dice pane or a link, with results that can roll on other tables
  - [ ] Name generators, built on roll tables, with a few built-in lists to start from
  - [ ] Story prompts: suggestions for inspiration when building quests and places (drawn from roll tables, never writing the story for the GM)

### Phase 3: climax

Play at the table and solo play. This needs panes the user can arrange, so layouts come first.

- [ ] Layouts (see [Layouts](#layouts))
  - [ ] Split the main pane to show two notes side by side, and choose where each pane goes (left, right, bottom, or a tab) and its size
  - [ ] Collapse panes (file tree, extra notes) while keeping them hidden with an arrow allowing them to be pulled back
  - [ ] Pop panes out into their own windows, and put them back
  - [ ] Resize any pane (Click and drag pane edges)
  - [ ] Save arrangements as named layouts ("Prep", "At the table") and switch from the View menu
- [ ] Journaling tool, built for solo play
  - [ ] A journal folder whose notes are dated sessions, added to quickly while playing
  - [ ] The dice roller and roll tables built into the journal: a roll, or an oracle question and its answer, goes straight into the entry
  - [ ] A side-by-side view of the journal with lore, a roll table, or (once Phase 5 has the reader) the rules
- [ ] Session notes for GMs use the same journal: what happened at the table, linked to the NPCs, quests and places involved

### Phase 4: falling action

Maps. They come after the notes and panes are settled, so a map can be one more pane and every pin can point at a note.

- [ ] Basic SVG editor for maps, saved as SVG files in the campaign folder
- [ ] Link maps and the wiki: pins and regions on a map link to notes, and a location note shows the maps it's on
- [ ] Map generators, starting small and growing: dungeon, building, city, region, then world. A generated map is an ordinary SVG the editor can change
- [ ] Generator settings can be saved and reused

### Phase 5: resolution

Sharing and reference: everything that leaves the campaign folder or comes in from outside it. It comes last because templates and settings should hold everything the earlier phases add (types, roll tables, dice presets, journals, maps).

- [ ] Campaign templates (see [Campaign templates](#campaign-templates))
  - [x] New Campaign offers a template: the starter folders, empty, or one of the user's own
  - [ ] Opening an Obsidian vault for the first time offers a template, without changing existing notes
  - [x] Save a campaign's setup as a template, and share it as a file (Edit > Types > Export Campaign Structure; Import... adds one)
  - [x] Open Campaign lists the campaigns opened or made before, newest first, with a search box, instead of only a folder picker
  - [ ] Templates can set dice presets and suggest a game system
  - [ ] Built-in templates for specific systems and uses, offered by New Campaign beside the starter folders (for example D&D 5e, Shadowdark, Cairn and Cyberpunk, and uses beyond TTRPGs like a novel or a setting bible)
  - Made in the same format as the user's own templates (a zip of folders and `_type.yaml` files), so they show what a template can do and can be exported, changed and shared like any other
  - Player and character types come from these: each system's character sheet as fields (see Player Management Excluded below)
  - Once this exists, Ammon builds templates for a few systems to find where it's awkward before the format settles
- [ ] Settings: reuse a world in another campaign or book (see [Settings](#settings))
  - [ ] Mark folders as setting and export them, or start a new campaign from an existing setting (a copy)
  - Copies are the plan. A shared setting with live edits is only a maybe, for interconnected campaigns or books, if people ask for it
- [ ] PDF manual library (see [PDF library](#pdf-library))
  - [ ] Library folders, systems as tags, a library panel; links to PDFs open them in the system's PDF viewer
  - [ ] Built-in PDF reader beside a note (a pane, using Phase 3's layouts): opens at the right page, with the PDF's table of contents and the user's bookmarks
  - [ ] Search across all manuals, and copy text into a note with a link back to the page
- [ ] Player handouts (see [Handouts](#handouts))
  - [ ] Export chosen notes (or parts of them) for players, without GM-only fields and sections, as Markdown that opens in Dungeon Elements or Obsidian
  - [ ] The same handouts as PDFs to print or send
  - [ ] Table maps as PDFs: print a map at table scale with a chosen overlay (squares, hexes, or none)

### After 1.0: stretch goals

- [ ] Many languages (i18n), see [Languages](#languages)
  - [ ] Every word the app shows (menus, buttons, dialogs, messages) comes from translation files instead of the code, starting with English
  - [ ] Pick the language in the View menu (or a settings dialog), following the system's language at first
  - [ ] The how-to guides translated, one folder per language in `help/` (falling back to English for guides not translated yet)
  - [ ] Right-to-left languages (Arabic, Hebrew) and scripts that need other fonts (Chinese, Japanese, Korean)

## Design

The layout plan is in `reference/dungeon_elements_layout_plan.png` and the logo in `reference/dungeon_elements.svg`. The reference is drawn in black and white for contrast only; the app is themed. The colored logo, `reference/dungeon_elements_colors.svg`, is the app icon (window, taskbar, and the Windows `.exe`, from `assets/dungeon_elements.ico`).

### Themes

- A theme is a YAML file that sets a color for each named role (background, sidebar, menu bar, tabs, borders, text, fields, accent, status bar, and so on). Any role left out falls back to the theme's `base` (dark or light).
- Built-in themes ship with the app (the YAML lives in `themes/`): Dark (the default) and High Contrast (the reference drawing). More built-in themes later.
- User themes are YAML files in the app's config folder (`%APPDATA%\dungeon_elements\themes` on Windows, `~/.config/dungeon_elements/themes` on Linux) and appear under View > Theme. A broken theme file is skipped with a warning, never a crash.
- The logo is recolored to match the theme (black becomes the text color, white the background).
- Later: a theme editor (View > Theme > Edit Themes...). The user picks the main colors and accents with a color picker, sees the app update live, and saves the result as a theme file in the user themes folder, ready to use or share. Fine-tuning single roles is optional, because anything left out is worked out from the main colors or the `base`.

### Layouts

Phase 3. Customization is a big part of the app, so the window's layout should be the user's to arrange too.

- Panes (the file tree, the editor, the dice roller, and later maps, roll tables and the journal) can pop out into their own windows, and be put back.
- The user chooses where each pane appears in the main window (left, right, bottom, or as a tab), and resizes it.
- An arrangement can be saved as a named layout (for example "Prep" and "At the table") and switched from the View menu. Layouts are saved in the app's config folder, like themes, so they can be shared.
- The default layout is the one in `reference/dungeon_elements_layout_plan.png`.

### Content types

- Each folder can hold a `_type.yaml` that defines the fields every note in that folder has. One type per folder.
- Field kinds: `text`, `number`, `choice` (with `options`), `tags`, `link` (an Obsidian `[[link]]`), `checkbox`, `longtext`, and `steps` (a checklist whose steps can be `[[links]]`).
- Short fields are stored in the note's YAML front matter. `longtext` and `steps` fields are stored as `## Label` sections in the Markdown body (steps as a `- [ ]` task list), so notes stay readable in Obsidian.
- The note's front matter records `type:`, so a note moved to another folder still knows its type.
- The `name` field is the filename (`The One Ring.md`). The tree shows names without `.md`, like Windows Explorer, unless View > Show File Extensions is on.
- Front matter keys and `##` sections the type doesn't know about become fields of that note only (the main pane's + adds one; each has a Remove button). Values that can't be a field (nested lists or maps) and text before the first section are kept and shown in an "Other" area. Saving never drops data added in Obsidian or another editor.
- Any field except `name` can be removed from a single note: the note's front matter lists it under `removed_fields`, and the main pane's + offers to bring it back.
- A folder with no `_type.yaml` is a plain Markdown folder.
- Phase 2:
  - Subfolders inherit their parent's type, like child classes. A subfolder with no `_type.yaml` has its parent's type. One with its own `_type.yaml` is a kind of its parent's type: its own `type:` id, the parent's fields in the parent's order, then its own. A field with the same key as an inherited one changes it in place, and `removed_fields: [key]` leaves an inherited field out (never `name`). Only what the subfolder adds or changes is saved in its `_type.yaml`, so changes to the parent show up in it. Folders in the campaign folder itself don't inherit the campaign folder's type.
  - A note is read with its folder's type when its `type:` is that type or one it is a kind of, so a note that says `type: item` in Weapons is a Weapon (and becomes `type: weapon` when saved).
  - A `steps` field kind: an ordered checklist where each step is text or a `[[link]]` to another note. The Quest type uses it so a step can be another quest, building quest lines (a Big Quest made of smaller quests).
  - Types can be saved for reuse in other campaigns from the Edit > Types dialog, as `<Name>.type.yaml` files in the config folder's `types` folder. A saved type holds every field it inherits, so it works on its own.
- Phase 5: campaign templates, see [Campaign templates](#campaign-templates).

### Campaign templates

Phase 5. Templates put the power in the user's hands: the app ships a few, and users build their own for their systems (or for things that aren't TTRPGs at all, like a novel).

- A template is an ordinary `.zip` file of folders and their `_type.yaml` files, and nothing else: no notes, so it is small, has no spoilers, and can be made or changed with any zip tool (a folder zipped by the file manager works too; the one folder around everything is left out). The app's starter folders (`templates/default/`) are the built-in one.
- Templates live in the config folder's `templates` folder, and every `.zip` there is offered by New Campaign, by its file name. Import... (in New Campaign and Edit > Types) checks a template and copies it there.
- Later, maybe: note templates, dice presets and a suggested game system (`system: Shadowdark`), as extra files in the zip that older versions ignore.
- New Campaign offers a template. Opening a folder (such as an Obsidian vault) that Dungeon Elements has never opened offers one too, without changing existing notes.
- A campaign's current setup can be saved as a template from the app, to reuse or give to friends (Edit > Types > Export Campaign Structure).
- Open Campaign lists the campaigns opened or made in the app (`campaigns.yaml` in the config folder), so they don't have to be found in the file browser.
- Templates and manuals stay separate. A template's `system:` only decides which manuals the [PDF library](#pdf-library) shows first, and the user can change it or leave it out.
- The format isn't fixed until real templates for a few systems have been built and tried.

### Settings

Phase 5. For writers, and for GMs who run several games in one world: a world built once can be reused without copying notes by hand. A spin-off book or a new campaign with a different cast starts with the setting ready.

- A campaign marks which folders are its **setting** (for example lore, locations, factions, history) and which belong to this story only (players, quests, journal, the cast). The setting can also be all of it.
- **Export setting** saves those folders, their types and their notes as one shareable folder or file, without the story-only parts. Links from setting notes to story-only notes are listed so the user can decide what to do with them.
- **New campaign from a setting** copies it in, so the two can change on their own. This works with Obsidian too, because the notes are ordinary files in the vault. It sits beside templates: a template gives the shape (folders and types), a setting gives the world (the notes).
- Maybe, only if people ask: a **shared setting** for interconnected campaigns (or a spin-off set at the same time as the main story), one setting folder that several campaigns point to. Edits show up in all of them, and links resolve across the campaign and its setting. Obsidian can't see folders outside the vault, so this needs care (a folder link inside the vault, or an Obsidian-friendly option to copy instead).

### PDF library

Phase 5. Easy access to the user's PDF rulebooks and other documents. It is a library and reader, not a PDF manager: the app never changes a PDF (no marking up, editing, or form filling).

- **One library for the whole app**, not inside campaigns, because the same rulebooks serve many campaigns. The user adds library folders in Settings and nothing is copied. For each folder they choose how it's organized:
  - "Each subfolder is a game system" (e.g. `Shadowdark/`, `D&D/`, `Cosmere/` become systems)
  - "Just a pile of PDFs": manuals start in an _Unsorted_ group and can be given a system from a dropdown; sorting is optional
- **Systems are tags**, starting from the subfolder name and editable. A manual can have several, or _Any system_ (oracles, random tables, GM advice). Manuals are recognized by their contents, not their path, so moving or renaming a PDF keeps its tags and bookmarks.
- **Campaigns list the systems they use** (saved in the campaign folder, so it travels with the notes). Their manual panel shows those books first, with a "show all" switch.
- The library (folder paths, tags, bookmarks) is saved in the app's config folder, like themes, because paths belong to one computer.
- **Links** use Obsidian's form, `[[Player's Handbook.pdf#page=42]]`, matched against the library by name. When two systems have a book with the same name, the campaign's own system wins. Links can use the page number printed in the book or the PDF's own page number.
- **Showing PDFs**: first in the system's PDF viewer (some viewers can't jump to a page), then in a built-in reader using the `pdfium-render` crate (PDFium has a permissive license that fits MIT; its library ships next to the app on Windows and Linux). MuPDF and Poppler are ruled out because their licenses (AGPL and GPL) don't fit. The pure-Rust `hayro` is worth checking again when this phase starts.
- **Search** uses the text inside the PDFs, saved in a cache so it's fast. Scanned PDFs without text can't be searched; no OCR planned.
- Making PDFs (handouts, table maps) is a separate feature that needs a different crate; see [Handouts](#handouts).

### Handouts

Phase 5. The GM shares parts of the campaign with players without sharing their own notes. This is a one-way export, not a sync.

- A field or `##` section can be marked GM-only in the type, and a note can be marked as a whole. Handouts leave those out.
- Markdown handouts are a folder of ordinary notes with working links between the notes included; links to notes left out become plain text, so nothing points at secrets.
- PDF handouts and table maps need a PDF-writing crate (separate from the reader's `pdfium-render`), chosen when this phase starts. Table maps print the SVG at a chosen scale (such as one inch per square) across as many pages as needed, with the overlay drawn on top.

### Languages

After 1.0 (a stretch goal once Phase 5 is done). Ammon has international ties, and the app should work for GMs in many languages, not only English.

- Everything the app shows comes from a translation file per language, so translators edit text files, not Rust. [Fluent](https://projectfluent.org/) (the `fluent` crates, also used by Firefox) handles plurals and grammar that differ between languages, and is the first choice to look at. Plain YAML files, like themes, would be simpler but can't do plurals well.
- Users can add or fix a translation by dropping a file in the app's config folder, like a theme, so a language doesn't have to wait for a release.
- The help guides are already one folder per language (`help/en/`), so translated guides go in `help/de/`, `help/es/` and so on; a guide not translated yet shows in English.
- Notes are the user's own words, so nothing in a campaign is translated. Field names in the starter types and campaign templates could ship translated, so a new campaign in Spanish starts with Spanish folders and fields.
- iced needs fonts with the right characters (CJK fonts are large, so maybe downloaded or picked from the system) and right-to-left layout, which should be checked when this starts.
- Dates, numbers and dice (`2d6` is `2W6` in German) may need local forms; dice expressions should accept both.
- Until then, new code keeps what it shows in plain strings in one place where it can (like `help::SHORTCUTS`), which makes moving them to translation files easier later.

## Planned features

- Focus mode for writers
  - Full screen app with just what looks like a piece of paper and a background of the user's choosing
- Timer for specific sprints for writing or for in game timed things like Shadowdark's torch timer
- Do a check for updated notes, tags, and other file changes and offer fixes
- More to come as I have more ideas!
