# Changelog

All notable changes to Dungeon Elements are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Undo and Redo

- **Ctrl+Z** takes back the last change to a note, and **Ctrl+Y** (or **Ctrl+Shift+Z**) does it again. They are at the top of the **Edit** menu and the right-click menus, and as arrows at the left of the formatting toolbar, and they say what they would do, like **Undo Typing** or **Undo Roll**.
- Typing is undone a burst at a time: a pause of a second, Enter or a click starts the next step. A field filled from a roll, a choice picked, Bold, a list, a ticked task, a field added or removed and a table saved from the table editor are each a step of their own.
- Each note keeps about a thousand steps while the app is open, after saving too. Undoing back to the last save shows the note as saved. A note closed without saving can be opened again and its changes brought back with Ctrl+Z.
- Changes to the campaign can be undone and redone too: a new note or folder, a rename, a move, a delete, Note Types, the walkthrough questions and answers, link fixes and ticked quest steps. A rename or move is undone with the links that followed it, and a deleted folder comes back with its pictures. Ctrl+Z takes back whichever came last, the open note's change or the campaign's, and the menu names it, like **Undo Rename Saltmarsh**. About two hundred campaign steps are kept while the app is open.
- If a file changed since, maybe in another app, Undo asks before putting it back.
- Undo keeps only the words that changed in each step, not a copy of the note, and works them out when typing pauses rather than on every key, so it doesn't slow typing down.

### Menus

- The menu bar is sorted so each menu holds one kind of thing. A new **Campaign** menu has the walkthrough, the walkthrough questions, **Note Types...** (once Edit > Types...), **Export Campaign Structure...** and **Show Campaign Folder**. A new **Tables** menu has the Roll Tables list, the dice roller, **New Roll Table...** and **Add Roll Table Here**. **Maps** is there, greyed out, for the map editor to come.
- **Edit** has **Cut**, **Copy**, **Paste** and **Select All**, and **Search Notes** is now **Find in Notes**.
- **View** is shorter: the themes are in a **Theme** submenu and the file tree's Refresh, Expand All and Collapse All in a **File Tree** one.
- **File > New Campaign...** and **Open Campaign...** work while a campaign is open, closing it first.
- The dice roller opens and closes with **Ctrl+Shift+D**, or from **View** or **Tables**.
- Hold **Alt** and press a menu's first letter to open it (**Alt+F** for File), then move about with the arrow keys and pick with **Enter**.
- The built-in roll tables are in four groups, **Names**, **Story Prompts**, **World** and **Adventure**, in the Roll Tables list (click a group to open or fold it) and at the bottom of the **Tables** menu, where picking one rolls it.

### Inspire Me dropdown

- A field or walkthrough question that rolls on several tables has one **Inspire Me** button with a dropdown beside it, in place of a button for each table. For names, the dropdown lists **Any**, **Mixed** and each culture, for women and men: **English - Female**, **Norse - Male** and so on.
- The table picked is remembered for that field in the campaign, so after picking Norse - Male every NPC name rolls Norse men's names until you pick another.
- **Mixed female names** and **Mixed male names** take the first name from one culture and the family name from another, like Isaac Akinaka. A link in a roll table's column now rolls only the column of the same name in the table it links to, which is how they work, and your own tables can do the same.
- Every table button reads **Inspire Me**, and can have a word after it, like **Inspire Me - Names**.
- **Edit > Types...** and **Edit > Walkthrough Questions...** pick a field's or question's tables from a list, with no table names to type: whole tables, a part under a heading, like English - Female, or all its parts at once. A box sets the word on the button.

### Roll table editor

- Roll tables can be changed without touching their Markdown. Click a roll table in the reading view, or **Edit Table** above it, and it opens in an editor: the die, the columns and a line for each row, with a box for each result.
- **Calculate ranges** works out which numbers each row covers. Each row's **−** and **+** say how many of the die's numbers it takes, and the rows below make room. Untick it to type your own ranges, like 2-3 or 12+; anything missing or in two rows is pointed out as you type. A table written elsewhere with gaps or ranges of its own opens unticked, just as it was written.
- Each row shows its chance of being rolled, worked out from the real dice, so on 2d6 a row for 7 shows 17% and one for 2 shows 3%.
- Changing the die keeps each row's share of it: a d6 table turned into a d20 table keeps its weighting. **Spread Evenly** gives every row the same share.
- **Enter** adds a row, **Shift+Enter** starts a new line in a result, **Tab** moves to the next box, and rows and columns can be added, moved and removed. **Done** puts the table back into the note; **Cancel** leaves it as it was.
- It works on any roll table in any note, like the encounter tables in a chapter of a sourcebook, and only that table changes. A long text field in the form with a roll table in it has **Edit Table** too.
- Tables are still ordinary Markdown tables that Obsidian and other apps read. A line break in a result is saved as `<br>`, and shows as a line break in the reading view.
- **Link** on a row, or typing `[[`, links a result to another note or table, with the usual suggestions (Ctrl+K works too). A result that links to a roll table gets a **Rolls on** box, ticked to start with: untick it and the link stays a link when the table is rolled. Which links don't roll is kept in `.dungeon_elements/tables.yaml`.
- **Name** renames the heading a table is under, and the links to it in the note and in your other notes, like `[[Encounters#Marsh]]`, follow.
- **Paste a List...** adds a row for each line of a list copied from a web page or a PDF, and **Roll** tries the table as it is in the editor.
- **Add Roll Table Here**, in the reading view's right-click menu, adds a d6 table after the paragraph you clicked and opens it in the editor. **New Roll Table** opens the new table in the editor too.
- Right-click a table in the Roll Tables list for **Roll**, **Edit Table** and **Open Note**. A built-in table has **Copy into Campaign and Edit**.
- A new **Roll table** field kind in **Edit > Types...**: each note has the table editor in its form, always open, starting with a d6 table, saved as a `## Label` section that steps and other tables can roll with `[[Note#Label]]`.
- A quest step that links to a roll table, like `Cross the marsh: [[Encounters#Marsh]]`, has a button that rolls it.

### More generators

- New built-in roll tables for building a world: **Campaign names**, **Threats**, **Tone**, **World names**, **Peoples**, **Historic events**, **Aftermath**, **Faiths**, **Deity names**, **Domains**, **Wonders**, **Sources of power** and **Costs and limits**. They're written to fit any genre.
- And for adventures: **Dangerous places**, **Quest names**, **Rumors**, **Plot hooks**, **Opening scenes** and **Secrets**.
- Every question in the walkthrough rolls on a table made for it, so **Inspire me** gives ideas that fit the question: a nearby danger rolls a dangerous place, a rumor a quest name, and the first session an opening scene and secrets. A hero's bonds roll a name for a new NPC. "What game or rules will you use?" has no Inspire me button, since that's for you to choose.
- In new campaigns, the matching fields roll on them too: the campaign's Name, Big picture and Themes and tone, a people's, event's, religion's or deity's Name, an event's Consequences, a deity's Domains, a magic note's Name, Source and Cost and limits, a quest's Name and Hook (now Rumors and Plot hooks, in place of Trouble), a site's Name, a session's Strong start, and Secrets on NPCs, factions, locations and lore.

### App icon

- A new app icon for the taskbar, the window, the Windows executable, the Linux app menu and the macOS app: the colored logo on a light grey disc, so it stands out on dark taskbars. The logo inside the app is unchanged.

### Campaign walkthrough

- A walkthrough that builds your world with you, one question at a time, in seven chapters: the campaign, the world, power, the extraordinary, where it starts, the heroes and the first session. Each answer goes into a note, and the notes it needs are made for you and linked: name the factions and each gets a note, then questions about each one; name a faction's leader and an NPC is made.
- **Start the walkthrough** in New Campaign opens it once the campaign is made. **File > Campaign Walkthrough...** and **Build your world with the walkthrough**, shown when no note is open, open it later, at the question you were on.
- **Next**, **Back**, **Skip** and **Skip Chapter** move through it, and **Enter** (or **Ctrl+Enter** in a long answer) goes to the next question. The chapters are listed on the side with how many questions are answered, and each ends with a summary of the notes it made.
- **Inspire me** rolls on a roll table for an idea: names from the name generators, and wants, trouble and twists from the story prompts.
- Questions that ask for a list of names, like the regions or the factions, have a box for each name and **Add Another**. Pasting a list (one name a line, as name generator websites give them, or with commas between) fills a box for each, with capital letters added.
- Answers that link to other notes show the notes you've made to pick from, and a new name can be made there and then. Who rules the starting town can be a faction or an NPC, and who can use magic any people, faction or NPC.
- Notes the walkthrough makes show in the file tree, with their folders opened.
- **Add Characters from Files...** on the heroes question (or dragging files onto the window) copies characters your players wrote up in Markdown into the players folder and adds them to the heroes.
- Answers are ordinary fields, so changing them in a note changes them in the walkthrough. A note open with unsaved changes gets the answer in its tab.
- Questions about folders or fields a campaign doesn't have are left out.
- A template can have walkthrough questions of its own, for its folders. **Export Campaign Structure...** puts the campaign's `_walkthrough.yaml` in the template, importing a template checks it, and a campaign made from the template gets it. New Campaign says when a template has its own questions, and shows **Start the walkthrough** only for a template the walkthrough has questions for. Campaigns made from the starter folders keep using the built-in questions, so they get better ones as the app updates. A `_walkthrough.yaml` the app can't read is listed in **Problems**.
- **Edit > Walkthrough Questions...** changes the questions a campaign asks, with no YAML to write. Start from the default questions, then change, add, move or remove chapters and questions. Each question picks the note it goes into, the field it fills and the table Inspire me rolls on from lists, and the notes it makes have their folder, how they're named, values to fill in and links from other notes. Mistakes are shown in plain words before saving, and **Save and Try It** opens the walkthrough with the new questions.
- New guide in **Help > How-To Guides**: The campaign walkthrough.

### Story prompts

- Built-in story prompts, a few words each to spark an idea: **Wants**, **Trouble**, **Twists**, **What's here** and **Feel of a place**, plus **Action and Theme**, two words to read into, like *Betray* and *Harvest*.
- A long text field that rolls on a table has an **Inspire me** button. New campaigns set it on a quest's Hook, a location's First impression and Description, and an NPC's or faction's Wants and Goal.

### Name generators

- Built-in roll tables, under **Built in** in the Roll Tables list: **Names**, **Female names** and **Male names** (any of the cultures), names by culture (**English**, **Norse**, **Celtic**, **Latin**, **Greek**, **Arabic**, **Slavic** and **Japanese**, each with Female and Male tables), **Fantasy names**, **Place names**, **Inn and shop names** and **Group names**. Norse family names are built from the father's name and -sdottir or -sson, and fantasy names from a start and an ending. Link to them from your own tables or type their names in the dice roller. **Copy** puts one in your **tables** folder to change it, and your copy is used in its place.
- A text or long text field can roll on a table: set its **Rolls on** in **Edit > Types...** (several tables with commas between them), and it gets a die button for each in the form and the reading view. An empty field is filled in; a filled one asks before replacing what's there (or adding the roll on a new line). New campaigns roll NPC names on Female names or Male names, locations on Place names and factions on Group names.
- Rolling each column joins a result that starts with a hyphen to the one before it, so `Ash` and `-ford` make "Ashford".

### Roll tables

- Any Markdown table whose first column is a die, like `d6`, `2d6` or `d100`, is a roll table. Rows can be single numbers or ranges (`2-3`, `01-05`, `00` for 100, `5+`).
- In the reading view, each roll table has a **Roll** button above it. **View > Roll Tables** (Ctrl+Shift+T) lists every roll table in the campaign in the sidebar, to roll or open. You can also type a table's name in the dice roller's box.
- A result that links to another table, like `[[Storms]]` or `[[Weather#Winter]]`, rolls that table too, and dice in a result, like `1d4 days of rain`, are rolled. The result shows in the dice roller with the rolls behind it, a **Copy** button, and **Create New Note** to make a note named after it.
- A table with several result columns gives a whole row, or, with **Roll each column** ticked, rolls each column on its own to mix them: a First and a Last column make any mix of names.
- **File > New Roll Table...** (or a folder's right-click menu, or the **+** in the list) makes a table: pick the die, the columns and how many rows, or paste a list from a web page or PDF. Tables go in a **tables** folder to start with.
- **Problems** points out rows a table's die can't pick and numbers it can roll that no row has.
- New guide in **Help > How-To Guides**: Roll tables.

### Track your quests

- **View > Quests** (Ctrl+Shift+Q), or **Show Quests** on a Quests folder's right-click menu, shows the campaign's quests in the sidebar, grouped by Status. Complete, Failed and Abandoned start collapsed, so the quests still going come first.
- Each quest shows its progress, like `2/5`, with a bar. Click the arrow to see its steps and tick them off right there: in the quest's tab if it's open, or straight into the note if it isn't.
- Quest lines show as a tree: a step that links to another quest shows that quest under it, with its own steps. A step that is a quest counts as done when that quest is Complete.
- New guide in **Help > How-To Guides**: Tracking quests.

### Search your notes

- A search box at the top of the file tree (**Edit > Search Notes**, Ctrl+Shift+F) finds notes as you type. Notes whose name or alias matches come first, then notes that mention it, each with its folder and the line that matched. Click one to open it, or press Enter for the first. Clear the box or press Escape to see the file tree again.
- Type `field:value` to find notes by a field, like `status:alive`, `location:Saltmarsh` (a link field matches the note it links to) or `factions:"Thieves Guild"` (a list matches any of its items). Fields go by their key or label, checkboxes by `yes` and `no`, and long text fields by the text under their heading. Words and fields can be mixed: `cook status:alive`.
- Right-click a folder and pick **Search in Folder** to search only it and its subfolders, so `location:Saltmarsh` in NPCs finds every NPC in Saltmarsh. With the box empty, it lists every note in the folder.
- New guide in **Help > How-To Guides**: Searching your notes.

### New starter folders

- New campaigns start with nine folders built to answer the questions world-building asks: **Campaign** (the pitch, the big picture, arcs and house rules), **Sessions** (prep and recap, in place of Journal), **Players**, **Quests**, **NPCs**, **Factions**, **Locations**, **Lore** and **Items**. They aren't tied to any game system.
- Some come with subfolders that are kinds of their folder: Enemies and Deities in NPCs, Religions in Factions, Settlements and Sites in Locations, History, Peoples, Magic and Technology, and Creatures in Lore, and Weapons, Armor and Gear in Items.
- The fields use what fields can do now: links between notes (a faction's Leader and Members, a location's Inhabitants), dates (a session's real date and its in-world one), steps (a session's Scenes and Secrets and clues, a faction's Plan), dice (an enemy's Attack and Damage) and tags. World notes have **Known to players** and a **Secrets** section, and NPCs have a **Status** (alive, dead, missing...).
- Campaigns you already have keep their folders and fields.

### Templates, shared types and a list of your campaigns

- **Edit > Types...** can **Export Campaign Structure...**: the campaign's folders and their fields, subfolders and all, without any notes, as an ordinary `.zip` file. Saved in your templates folder, it's offered by **New Campaign...**; saved anywhere else, it's ready to share.
- **New Campaign...** starts from the starter folders, an empty folder, or any of your templates. **Import...** adds a template someone shared, including a folder zipped with your file manager (only its folders and types are used).
- **Add New** at the bottom of the list in **Edit > Types...** makes a new type, with a folder for its notes named after it.
- **Export Type...** saves one folder's fields as a file like `Weapon.type.yaml`. Saved types can start a new folder in any campaign, from the **+** at the bottom of the file tree or in **Edit > Types...**.
- **Open Campaign...** lists the campaigns you've opened or made, the latest first, with a box to find one by name. **Browse for a Folder...** opens any other.
- New guide in **Help > How-To Guides**: Templates and sharing types.

### Dates, tags and links to several notes

- A new kind of field, **Date**: type a date, pick one from a calendar, or click **Today**. New notes start with today's date, so a journal entry knows when its session was played. It is saved as `2026-10-28`, which Obsidian reads as a date. Untick **Pick the date from a calendar** in **Edit > Types...** for dates that aren't on one, like a Stardate: the field becomes a line of text. The Journal's Date is a date field in new campaigns.
- **Tags** are picked one at a time. The tags there are show under the field to click, so they aren't misspelled, and each tag has a note in the **tags** folder listing links to the notes tagged with it. Saving a note makes its new tags' notes and keeps their lists up to date. Tags are still saved as a list Obsidian reads as tags. When a note's tags are changed in another app, a dialog offers to update the tag notes to match.
- A new kind of field, **Links to notes**: any number of `[[links]]` in one field, like everyone who knows a spell. Each opens with a click, and **Open all** opens them all. Lore notes have a Related field of this kind in new campaigns.

### Dice fields

- A new kind of field, **Dice**: pick how many dice, which die, a modifier (`+2`), and something special (advantage, disadvantage, keep highest or keep lowest) instead of typing an expression. Add one to a folder in **Edit > Types...** or to one note with **+**.
- Its roll button rolls it with the field's own advantage or disadvantage, on any die, in the form and in the reading view.
- It is saved as readable text, like `2d6+3`, `4d6kh3` or `1d20+5 advantage`, so Obsidian shows it as it reads and it can be typed there too. Dice with more than one kind of die are kept as written.

### Steps

- A new kind of field, **Steps**: a checklist where each step is text, a `[[link]]` to another note, or both. A quest's Objectives are steps, so a step can be another quest, building quest lines out of smaller quests.
- Type a step and press Enter to add it, tick it when it's done, move it up or down, or remove it. Typing `[[` in a step suggests quests first, and a step's **Open** button opens the note it links to, making a new quest if there isn't one yet.
- Steps are saved as a Markdown task list under their heading, so they show as a checklist in Obsidian and can be ticked in the reading view.

### Forms

- **Tab** and **Shift+Tab** move between the fields of a form, scrolling to the field. In a link field, Tab inserts the suggested note.

### Subfolders

- Subfolders work like child classes: a subfolder of a folder with fields can be **a kind of** its type. Weapons in Items has every Item field (Value, Rarity...) and fields of its own (Damage); Enemies in NPCs starts out like any NPC and adds a Challenge Rating. Changes to the parent's fields show up in its subfolders, and subfolders can have subfolders of their own.
- Right-click a folder for **New Subfolder...**, which offers a kind of the folder's type (its fields are added next in **Edit > Types**) or the same fields as the folder. A subfolder with no fields of its own has its parent's fields.
- **Edit > Types** shows subfolders under their folder, and lists the fields a subfolder inherits: **Change** one for the subfolder only, **Remove** it from the subfolder's notes, or **Bring back** one that was removed.
- Right-clicking a folder offers a new note of each of its kinds (**New Item**, **New Weapon**), and the **+** next to a folder says what it makes when you point at it.
- A note moved into a subfolder becomes that kind of note and keeps what was filled in. A note that says it's an Item but is in Weapons is read as a Weapon.

### Fixes

- On Linux, the app kept reloading the campaign in the background, using the processor and closing menus as soon as they opened: reading the notes again after a change looked like another change.

### Help

- **Help > How-To Guides** (F1) opens a Help window with short guides: getting started, notes and fields, links, reading and formatting, dice, and your files. The start screen links to it too.
- **Help > Keyboard Shortcuts** (Ctrl+/) lists every shortcut, grouped by what it's for.
- **Help > What's New** shows what changed in this version.
- The Help window has a search box that narrows the guides and shortcuts to what you typed.
- **Help > Report a Problem...** opens a new GitHub issue in your browser, and **Dungeon Elements on GitHub** opens the project's page.

## [0.2.0] - 2026-10-27

Phase 1, _Inciting Incident_, of the [plan](PLAN.md): notes link together like a wiki, read like one, and can be written without knowing Markdown, with a dice roller close at hand. Campaigns from v0.1.0 open as they are. Dungeon Elements is still in early development, so keep backups of campaigns you care about.

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
- A note's name can hold characters a file name can't (`Chapter 1: The Inn`, `What?`, `The "Lucky" Inn`). The name field keeps the name as typed, and the file gets a name that works in Windows, macOS, Linux, and in links (`Chapter 1 - The Inn.md`, `What.md`, `The 'Lucky' Inn.md`). Links written with the full name still find the note. The file tree, tabs and dialogs show the name as typed (View > File Extensions shows the real file names).
- Aliases, the Obsidian way: list other names for a note under **Aliases** (add a field called Aliases, or `aliases:` in the front matter). The BBEG can go by Jerry: `[[Jerry]]` finds the note, and typing `[[Jer` suggests "Jerry (BBEG)", which links as `[[BBEG|Jerry]]` so it works in Obsidian too.
- A note whose name has characters its file name can't gets that name as an alias, so Obsidian finds it by its real name, and links to it show the real name: `[[Chapter 1 - The Inn|Chapter 1: The Inn]]`. Renaming it updates the alias and the name those links show (links showing words of their own keep them).
- Backlinks: the reading view ends with **Linked from**, the notes that link to the one you're reading, grouped by kind (Quest, Location, Journal Entry...). Each shows the line the link is on, or the field it's in (Quest giver: BBEG); click a name to open it. Links count however they name the note: by name, path, alias (`[[Jerry]]`), with shown text (`[[BBEG|Jerry]]`), to a heading, or as an embed. Unsaved changes and edits made in other apps count right away.
- Suggestions rank names, aliases and file names together: whatever starts with what you typed comes first, then names with a word starting with it ("inn" finds Old Inn before Spinning), so an alias like Jerry is never crowded out by other notes.

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
- Typing `[[` in a block you're editing in the reading view suggests notes right under it, by name, alias or file name; press Tab (or click one) to insert it. A link field in the box suggests notes the same way (Enter picks the first), with its folder's notes first and **+ New Location** (or whatever the folder holds) for a place that doesn't exist yet.
- Click a task box (`- [ ] Find the cave`) in the reading view to tick or untick it; the note changes as if you typed the `x`. Task boxes show in numbered lists (`1. [ ] Sneak in`) too.

### Editing

- A formatting toolbar shows over the note while you edit it, so you don't need to know Markdown: pick what a line is (Paragraph, Heading 1–4, Quote), make text bold, italic or crossed out, make bulleted, numbered or task lists, or add a link or picture. It writes the Markdown for you, and each button's tooltip shows what it writes. Press a button again to take the format away. In the reading view it shows while you edit a block; **View > Formatting Toolbar** hides it.
- Ctrl+B and Ctrl+I make the selected text (or the word at the cursor) bold or italic.
- Pressing Enter in a list starts the next item (numbered one more, or with an empty task box), and pressing it on an empty item ends the list. Quotes carry on the same way.
- The Markdown marks are dimmer than the words while you edit, so the words stand out: `#` before a heading, `**` and `*` around bold and italic text, `- [ ]` before a task, the brackets of links and the `(address)` after a link or picture, a table's `|`s, and the fences around code and front matter. This works in the Markdown text, in a form's long text fields, and in a block you're editing in the reading view.
- A note with a type has **Form**, **Markdown** and **Read** buttons at the right of the tab bar, so it's clear how to switch between its form, its Markdown text and the reading view (other notes have **Edit** and **Read**). **View > Edit as Form** and **View > Edit as Markdown** (Ctrl+E switches) do the same, and right-clicking the Markdown text offers **Edit as Form**.
- **File > Save All** (Ctrl+Alt+S) saves every open note with unsaved changes. New notes without a file yet ask where to go, one at a time, and the status bar says how many were saved and why any weren't.
- Typing in the Markdown source no longer loses the focus when the note changes in another app and the "changed outside" banner shows up.

### Dice roller

- The d20 button at the right of the tab bar opens a dice roller along the bottom of the window. Click a die (d20, d12, d10, d100, d8, d6, or d4) to roll it; the result shows on the left.
- Type a number in a die's **Count** box to roll several at once. Each result shows, with their total, and the last few rolls are listed under it.
- Type dice to roll in the box above the dice and press Enter (or **Roll**): `2d6+3`, `d20-1`, `1d8+2d6+4`, `d%`, or `4d6kh3` to keep the highest three (`kl` keeps the lowest). A modifier on its own, like `+5`, rolls a d20 with it. If it can't be read, the pane says so.
- **Advantage** and **Disadvantage** roll each single d20 twice and keep the higher or the lower; the other die is shown crossed out. They stay on until **Normal** is picked, and leave damage dice and rolls of several d20s alone. For systems that give advantage on any die, tick **Any die**: each set of dice then rolls one more and keeps the best (`1d6` rolls 2 and keeps 1, `3d6` rolls 4 and keeps 3), or the worst with disadvantage.
- Roll from a note's fields: a field with dice in it, like Damage `1d8 + 3 slashing` or Attack `+5 to hit`, gets a button for each roll, in the form and in the reading view's box of fields. A choice of dice, like a Hit die of `d10`, can be rolled too. The roll is named after the field ("Damage: 1d8+3") and opens the dice roller if it is closed.

### Known limitations

- A Number field drops a leading `+` when the note is saved (`+5` becomes `5`), so roll buttons won't see it as a modifier. Use a text field for bonuses like an attack's to-hit for now.
- `![[Note]]` embeds go one level deep: notes embedded inside an embedded note show as links.
- **Open Campaign...** on Linux uses the desktop's file chooser portal (or `zenity`); if neither is installed, open a campaign from the command line instead.
- Builds are not code-signed, so Windows SmartScreen may warn the first time you run the app ("More info" > "Run anyway"). The macOS app isn't signed with an Apple Developer ID: the first time, open **System Settings > Privacy & Security**, scroll down, and click **Open Anyway** (on macOS 14 and older, Control-click the app and choose **Open**).

## [0.1.0] - 2026-10-25

The first release: Phase 0, _Call to Action_, of the [plan](PLAN.md). Dungeon Elements is in early development, so expect rough edges, and keep backups of campaigns you care about.

### Campaigns

- Start on a splash screen that opens a campaign folder or creates a new one. New campaigns can start with folders for items, journal, locations, lore, NPCs, players, and quests.
- Open a campaign straight from the command line: `dungeon_elements path/to/my-campaign`.
- Browse the campaign in a collapsible file tree, with a tab for each open file and a status bar.
- Make folders with the round **+** under the tree, optionally with a note type.

### Notes

- Write notes in a text editor and save them as Markdown (`.md`) files, with **New Note** (Ctrl+N), **Open File...** (Ctrl+O), **Save** (Ctrl+S), **Save As...** (Ctrl+Shift+S), and **Close Tab** (Ctrl+W). Closing a tab or the window with unsaved changes asks first.
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

[Unreleased]: https://github.com/NightMarethyu/dungeon_elements/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/NightMarethyu/dungeon_elements/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/NightMarethyu/dungeon_elements/releases/tag/v0.1.0
