# Notes, folders and fields

## Folders give notes their fields

A folder can say which fields its notes have, like Name, Location and Status for an NPC. Its notes then open as a **form**. The fields are saved as ordinary Markdown with the short ones at the top of the file, so the note still reads well in any text editor.

- **Campaign > Note Types...** lists every folder on the left and its fields on the right. Add, remove, rename and reorder fields, and choose what kind each one is: text, number, a choice from a list, a date, tags, a link to another note, links to several notes, a checkbox, long text, steps, [dice](guide:dice), or a [roll table](guide:tables).
- A folder without fields holds plain Markdown notes.
- To give a folder fields, pick it in **Campaign > Note Types...** and add some.
- **Add New**, at the bottom of the list in **Campaign > Note Types...**, makes a new type and a folder for its notes. Name the type (like Faction) and the folder is named after it (factions), or type another name for the folder. **Discard** leaves it out. The folder is made when you choose **Save**.

## Fill in a form from the keyboard

- **Tab** goes to the next field and **Shift+Tab** back to the one before, scrolling the form to show it. Choices and checkboxes are skipped: click them.
- In a link field, or while typing a `[[link]]`, Tab inserts the suggested note instead.

## Dates

A **date** field holds a date, like when a session was played. A new note starts with today's date.

- Type the date as year-month-day (`2026-10-28`), click **Calendar** to pick a day from a month (**‹** and **›** go to the month before or after), or click **Today**. The day of the week shows under the box.
- It's saved as `2026-10-28`, which Obsidian reads as a date too.
- Dates that aren't on a calendar, like a Stardate or the 3rd of Frostfall, are kept as written. For a field that only holds those, untick **Pick the date from a calendar** for it in **Campaign > Note Types...**: it becomes a plain line of text, and new notes start with it empty.

## Tags

A **tags** field groups notes. Each tag is also a note of its own, in the **tags** folder, holding a list of links to the notes tagged with it, so the tag's note shows every note with that tag, here and in Obsidian.

- Type a tag and press **Enter** (or type a comma) to add it. The tags there are already show under the box: click one to add it without typing it, so it's never misspelled. Typing narrows them down.
- Each tag is a button that opens its note. **×** takes a tag off.
- Saving the note makes the notes of new tags and adds a link to the note to each one. A tag taken off has the link taken out of its note. Renaming the note renames the links.
- Tags changed in another app, like Obsidian, are noticed: a dialog lists the notes whose tags changed and offers to update their tag notes to match.
- The tags are saved as a list under `tags:`, which Obsidian reads as tags. A tags field can keep its tag notes in another folder: set **Tag notes in** for it in **Campaign > Note Types...**.

## Links to several notes

A **links to notes** field holds any number of links, like the people who use a kind of magic, or an NPC's allies.

- Type a note's name and press **Enter** (or **Tab**) to add the note suggested. Each linked note is a button that opens it, and **Open all** opens them all at once.
- Like a link field, it can **Link to** a folder in **Campaign > Note Types...**, whose notes are suggested first and where a new note it names is made.
- The links are saved as a list of `[[links]]`, which Obsidian shows as links, and each linked note lists this one under **Linked from**.

## Steps: quests made of smaller quests

A **steps** field is a checklist. Each step is a line of text, a `[[link]]` to another note, or both, like `Talk to [[Mira]]`. A step that links to another quest makes quest lines: a big quest whose steps are smaller quests, each with steps of its own. The Objectives of a quest are steps.

- Type a step in the box at the bottom and press **Enter** to add it. Enter in a step adds a blank one after it.
- Tick a step's box when it's done. **↑** and **↓** move it up or down, and **Remove** takes it off.
- Typing `[[` suggests notes, with the notes of the folder the field **Links to** first (quests, for Objectives). A step with a link has an **Open** button, which makes the note if there isn't one yet (a new quest, in the quests folder).
- A step that links to a roll table, like `Cross the marsh: [[Encounters#Marsh]]`, has a die button with the table's name that rolls it.
- Steps are saved as a Markdown task list (`- [ ] Find the map`) under a `## Objectives` heading, so Obsidian shows them as a checklist and the reading view can tick them too.
- Campaigns made before steps existed have Objectives as long text. To use steps, pick the quests folder in **Campaign > Note Types...** and change Objectives to **Steps** that **Links to** quests. Notes whose objectives are already a task list show them as steps.

## Subfolders are kinds of their folder

A subfolder works like a child class: its notes have every field of the folder it's in, plus fields of their own. Weapons, Armor and Gear in Items are all items, with a Value and a Rarity, but a weapon also has Damage. Enemies in NPCs start out like any NPC, then add a Challenge Rating.

- **Right-click** a folder and pick **New Subfolder...**. In a folder with fields, choose **A kind of Item** (named after the folder), then add its own fields in the **Campaign > Note Types...** window that opens next. Choose **Item notes** instead for a subfolder that only sorts notes, like Waterdeep in NPCs.
- In **Campaign > Note Types...**, a subfolder lists the fields it inherits first. **Change** one to make it different in the subfolder only (like other choices for an Enemy's Disposition), or **Remove** it from the subfolder's notes. Fields added to the parent folder later show up in its subfolders too.
- A subfolder can have subfolders of its own, like Swords in Weapons: they have every Item field, every Weapon field, and theirs.
- Right-clicking a folder offers a new note of each of its kinds (**New Item**, **New Weapon**), and the **+** next to a folder says what it makes.
- Moving a note into a subfolder makes it that kind of note and keeps everything you filled in. A note that says it's an Item but sits in Weapons is read as a Weapon.

## Fields for one note only

Sometimes one note needs something its folder doesn't have, like a tavern with a menu.

- The round **+** at the bottom right of a form (or **Edit > Add Field to This Note...**) adds a field to that note only.
- Each field (except the name) has a **Remove** button. A removed field can be brought back with the same **+**.

## Form, Markdown or Read

The buttons at the top right of a note switch how you see it:

- **Form** shows the fields to fill in.
- **Markdown** shows the note's text as it is saved (Ctrl+E switches between the two).
- **Read** shows it formatted, like a wiki page (Ctrl+R). See [Reading and formatting notes](guide:reading-and-formatting).

## Undo and Redo

**Ctrl+Z** takes back the last change to the note, and **Ctrl+Y** (or Ctrl+Shift+Z) does it again. The arrows at the left of the formatting toolbar, the **Edit** menu and the right-click menus do the same, and say what they would undo, like **Undo Typing** or **Undo Roll**.

- Typing is taken back a burst at a time: a pause of a second, Enter, or a click starts the next one.
- Everything else that changes the note is a step of its own: a field filled from a roll, Bold, a list, a ticked task, a field added or removed, a table saved from the table editor.
- Each note remembers about a thousand steps while the app is open, even after you save, and even after you close its tab and open it again. Closing a note without saving can be undone too: open it again and press Ctrl+Z.
- Undoing back to the last save shows the note as saved again.
- In the table editor, **Cancel** puts the table back as it was.

## Organize notes

- The big **+** at the bottom of the file tree makes a new folder, and offers fields to start from.
- **Drag** a note onto another folder to move it. It takes that folder's fields and keeps everything you filled in.
- **Right-click** a note or folder to rename, move or delete it. Deleted notes go to the Recycle Bin (or the Trash), so they can be brought back. Links to a note follow it when it is renamed or moved.
