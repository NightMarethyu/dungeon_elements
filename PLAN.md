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
- Friendly for non-technical people and creatives who don't want to deal with anything coding-adjacent. The files stay plain Markdown and YAML, but everything kept in them can be done from the app, without ever editing Markdown or YAML by hand

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

**Phase 2 is done.** The menu bar review and Undo and Redo were its last two items. Anything new goes into Phase 3 or later.

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
- [x] Adjust the default folders and types: go over the starter folders (`templates/default/`) and each type's fields now that fields can do much more (dates, tags, links to notes, steps, dice, subfolders as kinds), and change what a new campaign starts with to match
  - Nine system-neutral folders built to answer the common world-building questions: Campaign, Sessions (prep and recap, replacing Journal), Players, Quests, NPCs (Enemies, Deities), Factions (Religions), Locations (Settlements, Sites), Lore (History, Peoples, Magic and Technology, Creatures) and Items (Weapons, Armor, Gear). World notes have Known to players and Secrets; factions have a Plan of steps. Existing campaigns are unchanged

**Player Management Excluded**
The items in Player Management are all things that will just be built as templates, these can be based off character sheets for different TTRPGs. These items are all already included as fields and types

- [x] ~~Player management~~
  - [x] ~~The players type holds everything about a player and their character: player notes, backstory, and a character sheet built from the type's fields (numbers, choices, tags), so any system's sheet can be made in Edit > Types~~
  - [x] ~~Fields with dice in them (like `1d8+2` or `+3`) can be rolled from the note (builds on Phase 1's dice expressions)~~
- [x] NPC management
  - [x] Link fields for where an NPC lives, who they serve, and who they know; backlinks show where they appear
    - This was completed when we ensured links and backlinks were working properly
  - [x] A status field (alive, dead, missing...) and filtering the folder by a field, so the GM can find "every NPC in this town"
    - NPCs got their Status field with the new starter folders. The filtering is a simple note search: a box at the top of the file tree (Ctrl+Shift+F) that matches names and aliases first, then text, with `field:value` filters (`status:alive location:Saltmarsh`), and **Search in Folder** on a folder's right-click menu. It reads the notes each time, with no index (`src/search.rs`)
    - Maybe later, not decided: Boolean searches (and, or, not), regular expressions, and sorting the results (by a field, or by when notes changed). Worth considering if the simple search feels short, but not planned yet
- [x] Quest tracker
  - [x] The Quest type uses `steps`, so a step can be another quest, building quest lines (a big quest made of smaller quests)
  - [x] A quest view: open quests and their progress, ticking off steps, and the quest line as a tree
    - A Quests list in the sidebar (View > Quests, Ctrl+Shift+Q) in place of the file tree, grouped by Status, with finished groups collapsed. Steps are ticked from the list, and a step that is a Complete quest counts as done. Built as its own piece (`src/quests.rs`) so Layouts can move it into any pane
    - Left out for now: changing a quest's Status from the list, writing a sub-quest's completion into its parent's file, sorting or filtering the list, and adding or reordering steps from it
- [x] Generators
  - [x] Roll tables as notes (a Markdown table with a die column), rolled from the dice pane or a link, with results that can roll on other tables
    - Rolled from a Roll button above the table in the reading view, the Roll Tables list in the sidebar (View > Roll Tables, Ctrl+Shift+T), or by typing the table's name in the dice roller. Results show in the dice roller with Copy and Create New Note
    - A result that links to another table (`[[Storms]]`, `[[Weather#Winter]]`) rolls it too, and dice in a result (`1d4 days`) are rolled. A table with several result columns gives a whole row, or rolls each column on its own (a toggle, kept in `.dungeon_elements/tables.yaml`)
    - File > New Roll Table... makes an empty table (die, columns, rows split evenly) or fills one from a pasted list. Problems lists rows the die can't pick and numbers no row has
  - [x] Name generators, built on roll tables, with a few built-in lists to start from
    - Built-in tables (in the app, read only, copied into the campaign to change): Names, Female names and Male names, names by culture (English, Norse, Celtic, Latin, Greek, Arabic, Slavic, Japanese, each with Female and Male tables; Norse built from parts), Fantasy names, Place names, Inn and shop names, Group names. A campaign note with the same name is used in their place
    - `table:` on a text or long text field (Rolls on in Edit > Types) gives it a die button that fills it in, asking before replacing. The default template sets it on NPC (Female names, Male names), location and faction names
    - Several tables on one field or question show as one **Inspire Me** button with a dropdown (like Norse - Male), the pick remembered per field in the campaign; Edit > Types and the walkthrough editor pick tables from a list, and **Mixed** names take first and family names from different cultures
  - [x] Story prompts: suggestions for inspiration when building quests and places (drawn from roll tables, never writing the story for the GM)
    - Built-in tables of a few words each: Wants, Trouble, Twists, What's here, Feel of a place, and Action and Theme (two words to read into). A long text field with `table:` gets an **Inspire me** button; the default template sets it on quest hooks, NPC and faction wants, and location descriptions
- [x] World-building walkthrough: an optional guide that asks the GM questions one at a time and turns the answers into linked starter notes
  - Offered when a new campaign is made (a _Start the walkthrough_ box in New Campaign) and any time from **File > Campaign Walkthrough...**
  - Opens as a tab: chapters on the left (the campaign, the world, power, the extraordinary, where it starts, the heroes, the first session), one question at a time on the right with Back, Skip and Next. Every question can be skipped, and chapters can be done in any order
  - Each answer uses the input of the field it fills (a calendar for a date, the dice picker for dice) and is saved into its note right away, so nothing is lost and the notes stay the truth. A list answer ("name the factions") makes a note for each, and follow-up questions go through them; links fill themselves (a region is Part of the world, the starting town is the campaign's Home base)
  - Remembers where the GM left off in `.dungeon_elements/walkthrough.yaml` in the campaign: which note each answer went to, what was skipped and where the GM was. Whether a question is answered is read from the note itself (hidden, so the file tree, templates and Obsidian ignore it)
  - The questions are data: the default set is built in, and a campaign or template can have its own in `_walkthrough.yaml` at the campaign root (so the built-in system templates can ask their own questions)
  - Each question gets an **Inspire me** button that rolls on a story-prompt table (builds on Generators): a question names its table with `table:`, like a field does, falling back to its field's own table, then Action and Theme
  - A walkthrough editor (**Edit > Walkthrough Questions...**), so no one has to write YAML: chapters and question cards with pick lists for the note, field and table, the notes it makes, Start from the default questions, and Try it
  - Built in four steps: the question format, progress file and writing answers into notes; the walkthrough tab with Inspire me, menu items and a help guide; templates' own questions; the walkthrough editor
- [x] More generators, so every walkthrough question has a table of its own to roll on (in theory, a GM could fill in the whole walkthrough from the tables alone; no "fill it all in" button is planned)
  - Religions and faiths, and deities (names and what they hold sway over)
  - Rumors, plot hooks and quest names, for "What rumors are going around?" and quest notes
  - Peoples and races, historic events, magic and strange technology (name, source, cost), and anything else the walkthrough asks that falls back to Action and Theme today
  - Not for questions about real-world things, like the game system or who plays a hero: those have no Inspire me button
  - Built-in tables stay genre-light, so they fit fantasy, sci-fi and modern campaigns alike. They are a starting point and an example: the hope is that people make their own tables and share them
  - Built in two steps: 13 tables for the campaign, the world, power and the extraordinary (Campaign names, Threats, Tone, World names, Peoples, Historic events, Aftermath, Faiths, Deity names, Domains, Wonders, Sources of power, Costs and limits), then 6 for the adventure (Dangerous places, Quest names, Rumors, Plot hooks, Opening scenes, Secrets). Each is set on its walkthrough question and on the matching text field in the default template; a test checks that every built-in question names a table of its own
- [x] A roll table editor, so people can make and change their own tables without touching the Markdown table: the die, the columns and the rows in a form, with rows that can be added, moved and removed, and ranges kept in step with the die
  - Opens in place of the table in the reading view, for a table anywhere in any note (a chapter of a sourcebook can have several), and from **Edit Table** on a long text field in the form. The Markdown view stays one click away
  - **Calculate ranges** is ticked to start with: each row takes some of the die's numbers and the ranges are worked out. Unticked, people type their own ranges, like 2-3 or 12+. A table with gaps or ranges of its own opens unticked, as written
  - Each row shows its chance as a percent, worked out from the real dice (so 2d6 rows are weighted right)
  - Saved as an ordinary Markdown table that Obsidian reads; a line break in a result is saved as `<br>`
  - Built in two steps (done): the editor, then the rest (linking a result to another table with a **Rolls on** box for each row, a **Roll table** field kind for types, Paste a List, Add Roll Table Here, opening it from the Roll Tables list and New Roll Table, renaming the table's heading)
- [x] Menu bar review: work out the most user-friendly menu bar for everything the earlier items added, before Phase 3 adds more
  - A **Tables** menu (new roll table, the Roll Tables list, the built-in tables)
  - The built-in tables grouped in the Roll Tables list (names, story prompts, world, adventure) instead of one long Built in group
  - Seven menus, each holding one kind of thing: File, Edit, View, Campaign (the walkthrough, Note Types, Export Campaign Structure), Tables (the dice roller and roll tables), Maps (greyed out until the map editor) and Help. View's themes and file tree actions go in submenus, Edit gets Cut, Copy, Paste and Select All, and menus open from the keyboard with Alt and their first letter
  - Every menu item is a named command (`src/commands.rs`) with an id, a label and its shortcut, so the Phase 3 tool pane and custom shortcuts can offer "everything the menus can do"
  - Shortcuts: the dice roller is Ctrl+Shift+D; Ctrl+D is kept free for Duplicate (notes, and the map editor later)
  - Built in two steps (done): the commands and the new menus, then the Tables menu's built-in tables and the grouped Roll Tables list
- [x] **Undo** and **Redo** for everything changed in the app, not only typing: a field filled from a roll, a table changed in the table editor, a note moved or renamed
  - Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z, at the top of the Edit menu and the right-click menus, and as arrows at the left of the formatting toolbar, each saying what it would do ("Undo Rename Saltmarsh")
  - Notes keep about a thousand steps each, kept as only the words that changed; the campaign keeps about two hundred steps of files and folders (new, rename, move, delete, Note Types, walkthrough questions and answers, link fixes, quest ticks), and asks first if a file changed in another app since. Ctrl+Z takes back whichever came last
  - The table editor has its own Undo and Redo while it is open
  - History is kept while the app is open, not between runs
  - Built in three steps (done): notes, then files and folders, then the table editor

### Phase 2.1: long rest

A short stop to patch things up before the climax: the fixes and tweaks that go in before v0.3.0. Once they are done, we release v0.3.0 and move on to Phase 3. Each item is built on its own, in its own PR. An item that says "after" waits for that one; the others can start at any time.

- [x] Ammon's built-in tables
  - Replace the built-in tables in `assets/tables/` with the ones in `assets/tables/Ammons_Tables/`, then delete that folder
  - Add Ammon's new **Tropes** (d50) and **Verbs** (d100) tables to the Roll Tables list (Story prompts group). Replace the Walkthrough's **Action and Theme** rolls with a roll on the new tropes and verbs table the table can look something like this, it should match the Linked Tables items below so don't take this table as doctrine:

  | d1 | Tropes and Verbs | |
  | 1 | [[Tropes]] | [[Verbs]] |
  - Rolling a culture's names still gives a full name ("Alice Ashby", not "Alice"): each culture's top table gets a First and a Family column that link to its own given name and family name sections
  - Norse family names: picking a Norse name (Norse names, or Norse - Female / Male in the dropdown) gives a male given name and the matching ending (-sdottir for a woman, -sson for a man). Random rolls (Names and the Mixed tables) roll the ending on the Norse surname table, so it can come out either way. All of this is written in the tables, no code
  - Fix the links whose headings were renamed (like `[[Norse names#Female]]`, now "Female Given Name"), the Mixed tables' Family column (which points at given names), and every place that names a heading part: the name dropdown's choices and saved picks, and the `table:` lists in the default template and walkthrough
  - A test that every link in every built-in table finds its table, so a renamed heading can't break a roll again

- [x] Dice pane fixes
  - The roll history on the left keeps its width however many single dice are rolled, and scrolls (today it gets narrow and looks odd)
  - A **–** in the pane's top right corner hides it ("Hide dice roller (Ctrl+Shift+D)"), as well as the d20 button and the Tables menu
- [x] **Inspire me** replaces its own result without asking
  - Clicking Inspire me (or a table button on a field) again replaces the result it just put there, with no "Replace?" question: someone clicking again wasn't inspired by it
  - It still asks when the field holds anything else: text the person typed, or a rolled result they have changed since. That is the rule for "in a row", so it doesn't depend on how fast someone clicks
  - Each replaced roll is still one Undo step
- [x] Table result history, after the dice pane fixes
  - The dice pane gets two lists: **Dice** (the roll history today) and **Tables** (the results of roll table rolls, newest first, each with its table's name), from everywhere a table is rolled: Inspire me, field table buttons, the reading view, the Tables menu and the Roll Tables list
  - Each result has **Copy** and **Make Note** buttons. Make Note asks for the note's name (the result to start with) and where it goes: the campaign's folders with their types, plus **Plain note (no type)**
  - Like the dice history, it is kept while the app is open
- [x] Linked tables in the table editor
  - **Also roll**: a table can name one or more tables to roll after it, picked in a couple of clicks with no change to each row (Norse given name, then Norse surname). The results are joined into one, with no space before an ending like -sson
    - When joining results of multiple tables we need to ensure that the default format is to add a space, but we should allow a user to mark a column as needing no space (what we currently do when a column's results start with '-'), or needing commas. We can leave the current table formats, but I want to ensure that a user who wants to have their results come back as a comma separated list, regular spaces between results, or no space, they can do that without needing to know/remember to type special characters before each item in a column.
  - **Rolled after**: a table can be marked as needing another table's roll first (Norse surnames need a father's name). Rolling it on its own rolls that table first, and the editor and the Roll Tables list show the link
  - Kept as an ordinary roll table: a d1 table with one link per column, in a hidden file in the campaign's `.dungeon_elements/` folder. Today's rolling already handles it (each column, the -sson joins, the history, Undo), and the table editor shows it as Also roll picks, so nobody sees or edits the file. To check: that d1 counts as a die, and that fixing links after a rename covers that folder
  - The per-row **Rolls on** boxes stay, for rows that lead somewhere different
- [x] Roll tables and information tables
  - Each table is a **roll table** (Roll buttons, shown in the Roll Tables list and the link pickers) or an **information table** (a plain table, like prices, a calendar or a party list: no Roll button, not listed)
  - A table whose first column is a die (d6, d20, 2d6 …) starts as a roll table, any other as an information table. A switch in the table editor and a right-click on the table change it
  - The table editor works for information tables too: columns and rows, no die and no ranges
- [x] A **Table** view, after roll and information tables
  - Beside **Edit** and **Read** at the top right, for notes that hold only tables: tables, their headings, and short lines of text between them (like the line under Mixed names), with no type fields
  - It shows every table in the note in the table editor, one under the other, each headed by its heading; the text between them is edited in place, as in the reading view
  - Such notes open in the Table view (a View setting, like Open Notes in Reading View). Other notes keep Edit and Read, and their tables keep the Edit Table button
- [x] A built-in **Light** theme, and following the system
  - Light is cream, like old parchment, with dark ink text, its colors checked for contrast. The built-in themes become Light, Dark and High Contrast
  - A new **System** choice under View > Theme, the new default: Light or Dark to match the computer, checked again when the app gets focus. Picking a theme overrides it, and people who already picked one keep it
- [x] The walkthrough's left menu shows each section's steps
  - Each section has a triangle (like the file tree) that opens it to list its steps; the section being worked on starts open
  - Clicking a section or a step goes straight to it
  - A step shows a short title: its own, or the start of its question. The walkthrough question editor gets an optional **Short title** box, and the built-in questions get short titles
- [ ] Release v0.3.0 (see `RELEASING.md`), then the project website

### Phase 3: climax

Play at the table, solo play, and writing. This needs panes the user can arrange, so layouts come before the tools that live in panes. The steps are in build order: everything in a step can run at the same time, each item in its own PR, unless it says "after". v0.4.0 is released at the end of Step 3, and v0.4.1 adds pop-out windows (Step 4).

#### Step 1: quick wins (all in parallel, start any time)

- [ ] Custom roll buttons that roll two or more tables that aren't linked (like "Tavern": Inn and shop names, Feel of a place and What's here)
  - Made from the Roll Tables list (**New Roll Button**): a name, then tables picked from a list. Shown in their own group in the Roll Tables list and the Tables menu, and can be set on a field like a table; the results show together in the Tables history
  - Built like linked tables (Phase 2.1): a named d1 table with one link per column, in a hidden file, so rolling it needs almost nothing new
- [ ] Undo and Redo inside the Note Types and walkthrough question editors while they are open (their Save is already one step to undo)
- [ ] A progress bar for the walkthrough (moved from Planned features)
- [ ] A theme editor (View > Theme > Edit Themes..., see [Themes](#themes)), so nobody has to write a theme's YAML by hand. Its first theme: dark with neon green accents
- [ ] Opening an Obsidian vault (or any folder Dungeon Elements hasn't opened before) offers a template, without changing existing notes (moved from Phase 5)
- [ ] **Add to note** on each result in the dice pane's Dice and Tables history: puts the result at the cursor in the open note. The first piece of the journal: a roll goes straight into the entry

#### Step 2: layouts, one at a time (see [Layouts](#layouts))

Every pane is the user's to arrange, like Visual Studio Code: the file tree, the notes, the dice tray, the tool pane, and later the journal, maps and roll tables. Each item reworks how the main window is drawn, so they are built one after another, in this order.

- [ ] Resize any pane by dragging its edges, and collapse any pane (the file tree, the dice tray, an extra note) to an arrow that pulls it back
- [ ] Move panes: choose where each pane goes (left, right, bottom, or a tab), including the file tree and the dice tray, and split the main pane to show two notes side by side
- [ ] Tool pane
  - A tool pane instead of a ribbon (an idea from Adobe's apps): buttons for the tools, under the menu bar by default, moved like any pane
  - Users choose which buttons the tool pane shows, picked from a list of everything the menus can do, with no file to edit
- [ ] Save arrangements as named layouts ("Prep", "At the table", "Writing") and switch from the View menu
- [ ] A History pane listing the Undo steps of the note and the campaign, to go back several at once

#### Step 2, beside the layouts: the journal

For solo play, for GMs, and for authors writing a book: one quick way to keep dated entries.

- [ ] A **Journal** folder whose notes are dated entries, added quickly with **New Entry** (a button and a shortcut) that makes today's note and opens it ready to type. New Entry works on any folder whose type has a date, so GMs get the same on Sessions (this covers session notes for GMs: the Sessions type already holds what happened at the table, linked to NPCs, quests and places)
- [ ] Oracle questions in an entry: ask a yes/no question, roll a built-in oracle table (system-neutral), and the question and answer go into the entry, after Add to note

#### Step 3: play and writing tools (after resize and collapse)

These can run at the same time as each other and the later layout items.

- [ ] The journal side by side with lore, a roll table, or (once Phase 5 has the reader) the rules: an **Open Beside** right-click, after the main pane can split
- [ ] Timer (moved from Planned features): writing sprints, or in-game timers like Shadowdark's torch timer, as a small pane with start and pause and a sound or flash at zero. Several can run at once
- [ ] Focus mode for writers (moved from Planned features), inspired by tools like FocusWriter: full screen, every pane hidden, just the note on what looks like a page over a background the user picks, with a word count and an optional daily word goal. The timer can show for sprints. Moving the mouse to an edge shows the hidden panes
- [ ] Release v0.4.0 (see `RELEASING.md`)

#### Step 4: pop-out windows (v0.4.1)

- [ ] Pop panes out into their own windows, and put them back. A floating tool pane stays above the main window. This needs iced's multi-window support, so it is the largest layout piece and gets its own release
- [ ] Release v0.4.1

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
- [ ] Export for sharing finished work (see [Export](#export)): one export engine with two uses. Not critical to the app's goals, so it waits for 1.0, but it could come sooner
  - [ ] **Writing export**: export the Journal, or any folder marked as a manuscript (it doesn't have to be named Journal), as one document, for solo campaigns, books, short story collections and so on
  - [ ] Formats: `.txt` and `.md` for plain text, `.docx` for Microsoft Word and other word processors, `.epub` for ebooks, and `.pdf` for formatted documents
  - [ ] Creative options, set in the export dialog: scene breaks, chapter headings and chapter breaks, drop capitals, and similar book touches
  - [ ] **World-bible export**: pick notes, group them and put them in order to share a world (for GMs and writers), in the same formats and with the same options

## Design

The layout plan is in `reference/dungeon_elements_layout_plan.png` and the logo in `reference/dungeon_elements.svg`. The reference is drawn in black and white for contrast only; the app is themed. The colored logo, `reference/dungeon_elements_colors.svg` (on a light grey disc, so it shows on dark taskbars), is the app icon: the window, the taskbar, the Windows `.exe` (from `assets/dungeon_elements.ico`), the Linux app menu and the macOS app. The colored logo without the disc, `assets/die_icons/dungeon_elements_colors.svg`, is kept as artwork; inside the app, the menu bar and start screen show the outline logo in the theme's colors.

### Themes

- A theme is a YAML file that sets a color for each named role (background, sidebar, menu bar, tabs, borders, text, fields, accent, status bar, and so on). Any role left out falls back to the theme's `base` (dark or light).
- Built-in themes ship with the app (the YAML lives in `themes/`): Dark (the default) and High Contrast (the reference drawing). More built-in themes later.
- User themes are YAML files in the app's config folder (`%APPDATA%\dungeon_elements\themes` on Windows, `~/.config/dungeon_elements/themes` on Linux) and appear under View > Theme. A broken theme file is skipped with a warning, never a crash.
- The logo is recolored to match the theme (black becomes the text color, white the background).
- Phase 3: a theme editor (View > Theme > Edit Themes...). The user picks the main colors and accents with a color picker, sees the app update live, and saves the result as a theme file in the user themes folder, ready to use or share. Fine-tuning single roles is optional, because anything left out is worked out from the main colors or the `base`.
  - The first theme to make with it: dark with neon green accents

### Layouts

Phase 3. Customization is a big part of the app, so the window's layout should be the user's to arrange too.

- Like Visual Studio Code, every pane can be moved, not only the notes: the file tree, the editor, the dice tray, the tool pane, and later the journal, maps and roll tables.
- The user chooses where each pane appears in the main window (left, right, bottom, or as a tab), resizes it by dragging its edges, and collapses it to an arrow that brings it back.
- Panes can pop out into their own windows, and be put back (v0.4.1).
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

### Export

After 1.0, maybe sooner. For sharing creative work made in the app: a solo campaign written up as a story, a book, a short story collection, or a world bible for players, co-writers or readers. It goes with the Journal and Focus mode (Phase 3), which serve authors as well as GMs and solo players.

- **One engine, two uses.** The writing export and the world-bible export are the same feature with different intentions: both take an ordered list of notes, grouped into parts or chapters, and write them out as one document. Only the starting point differs.
  - **Writing export** starts from a folder marked as a manuscript (the Journal, or any folder; the name doesn't matter), in the folder's order or by date.
  - **World-bible export** starts empty: the user picks notes (or whole folders), puts them in groups ("Factions", "The North"), and drags them into order.
- **Formats**: `.txt` and `.md` for plain text, `.docx` for Microsoft Word and other word processors, `.epub` for ebooks, and `.pdf` for formatted documents. Every format gets the same content and as many of the creative options as it can show (plain text has no drop capitals, for example).
- **Creative options**, especially for books:
  - Scene breaks: what a break inside a chapter looks like (a blank line, `* * *`, a small ornament)
  - Chapter headings and chapter breaks: what starts a chapter (each note, or each group), how the heading reads ("Chapter 3", "Three", or the note's name), and whether each chapter starts on a new page
  - Drop capitals at the start of each chapter
  - Room for more, like a title page and table of contents, fonts, or page size for PDF
- **No Markdown or YAML to touch.** Everything is set in an export dialog with a preview, and the choices are saved with the folder or the world bible (in the campaign folder), so exporting the next draft is one click. Settings can be saved as a named style to reuse in other campaigns, like a theme.
- What goes in: the note's text, and its fields only when the user asks (a world bible likely wants an NPC's fields; a novel chapter doesn't). `[[Links]]` become plain text, or links within the document when the linked note is included too.
- Related: [Handouts](#handouts) and [Settings](#settings) export (Phase 5) also leave the campaign folder. Handouts could later be one more use of this engine, with GM-only parts left out, and the PDF writer can be the same one. Crates are chosen when this starts, with licenses that fit MIT, as for the [PDF library](#pdf-library).
- A smaller first version, if it helps: the writing export to `.md` and `.txt` with scene breaks and chapter headings, then `.docx` and `.epub`, then `.pdf` with the page options and drop capitals, then the world-bible picker on the same engine.

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

- More to come as I have more ideas!
