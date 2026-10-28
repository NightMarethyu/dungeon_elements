# Notes, folders and fields

## Folders give notes their fields

A folder can say which fields its notes have, like Name, Location and Status for an NPC. Its notes then open as a **form**. The fields are saved as ordinary Markdown with the short ones at the top of the file, so the note still reads well in any text editor.

- **Edit > Types...** lists every folder on the left and its fields on the right. Add, remove, rename and reorder fields, and choose what kind each one is: text, number, a choice from a list, tags, a link to another note, a checkbox, long text, or steps.
- A folder without fields holds plain Markdown notes.
- To give a folder fields, pick it in **Edit > Types...** and add some.

## Fill in a form from the keyboard

- **Tab** goes to the next field and **Shift+Tab** back to the one before, scrolling the form to show it. Choices and checkboxes are skipped: click them.
- In a link field, or while typing a `[[link]]`, Tab inserts the suggested note instead.

## Steps: quests made of smaller quests

A **steps** field is a checklist. Each step is a line of text, a `[[link]]` to another note, or both, like `Talk to [[Mira]]`. A step that links to another quest makes quest lines: a big quest whose steps are smaller quests, each with steps of its own. The Objectives of a quest are steps.

- Type a step in the box at the bottom and press **Enter** to add it. Enter in a step adds a blank one after it.
- Tick a step's box when it's done. **↑** and **↓** move it up or down, and **Remove** takes it off.
- Typing `[[` suggests notes, with the notes of the folder the field **Links to** first (quests, for Objectives). A step with a link has an **Open** button, which makes the note if there isn't one yet (a new quest, in the quests folder).
- Steps are saved as a Markdown task list (`- [ ] Find the map`) under a `## Objectives` heading, so Obsidian shows them as a checklist and the reading view can tick them too.
- Campaigns made before steps existed have Objectives as long text. To use steps, pick the quests folder in **Edit > Types...** and change Objectives to **Steps** that **Links to** quests. Notes whose objectives are already a task list show them as steps.

## Subfolders are kinds of their folder

A subfolder works like a child class: its notes have every field of the folder it's in, plus fields of their own. Weapons, Armor and Gear in Items are all items, with a Value and a Rarity, but a weapon also has Damage. Enemies in NPCs start out like any NPC, then add a Challenge Rating.

- **Right-click** a folder and pick **New Subfolder...**. In a folder with fields, choose **A kind of Item** (named after the folder), then add its own fields in the **Edit > Types...** window that opens next. Choose **Item notes** instead for a subfolder that only sorts notes, like Waterdeep in NPCs.
- In **Edit > Types...**, a subfolder lists the fields it inherits first. **Change** one to make it different in the subfolder only (like other choices for an Enemy's Disposition), or **Remove** it from the subfolder's notes. Fields added to the parent folder later show up in its subfolders too.
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

## Organize notes

- The big **+** at the bottom of the file tree makes a new folder, and offers fields to start from.
- **Drag** a note onto another folder to move it. It takes that folder's fields and keeps everything you filled in.
- **Right-click** a note or folder to rename, move or delete it. Deleted notes go to the Recycle Bin (or the Trash), so they can be brought back. Links to a note follow it when it is renamed or moved.
