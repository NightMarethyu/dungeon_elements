# Roll tables

A roll table is an ordinary Markdown table whose first column is a die. Rolling the die picks a row:

```markdown
## Weather

| d6  | Weather          |
| --- | ---------------- |
| 1   | Clear and cold   |
| 2-3 | Grey drizzle     |
| 4-5 | 1d4 days of rain |
| 6   | [[Storms]]       |
```

It's plain Markdown, so it shows as a table in Obsidian and anywhere else, and sharing a table is sharing its note.

## Making a table

**Tables > New Roll Table...** makes one in a few clicks. You can also right-click a folder and pick **New Roll Table...**, or click **+** in the Roll Tables list.

- Give it a **Name**, and pick its **Folder** (a **tables** folder to start with, made when you make your first table).
- Pick the **Die**: d4, d6, d8, d10, d12, d20, d100 or 2d6.
- **Rows** is how many rows it has, one for each number on the die if you leave it empty. Fewer rows split the numbers evenly, so a d100 with 20 rows gets 01-05, 06-10 and so on.
- **Result columns** are the headers after the die. Add more for tables like Name and Description, or First and Last.
- **Paste a list** to copy a table from a web page or a PDF: one result a line. Numbers or ranges at the start of the lines, like `1.` or `3-4`, are kept. Otherwise the rows are numbered for you, and the die is picked to fit.

The note opens in the table editor, ready to fill in. You can also type a table into any note yourself, or right-click in the reading view and pick **Add Roll Table Here** for a d6 table after the paragraph you clicked, open in the table editor.

## Changing a table

Click a roll table in the reading view, or **Edit Table** above it, and it opens in the table editor. Right-click a table in the Roll Tables list and pick **Edit Table** to open it there. It works on any roll table in any note, like the encounter tables in a chapter of a sourcebook, and only that table changes.

- **Name** is the heading the table is under, for a table under one. Renaming it renames the heading, and the links to it in your other notes, like `[[Encounters#Marsh]]`, are changed to match.
- **Die** picks the die. Changing it keeps each row's share: a d6 table turned into a d20 table keeps its weighting. **Spread Evenly** gives every row the same share.
- **Calculate ranges** works out which numbers each row covers. Each row's **−** and **+** say how many of the die's numbers it takes. Untick it to type your own ranges, like 2-3 or 12+. A table with gaps or ranges of its own opens unticked, just as it was written.
- Each row shows its **chance** of being rolled, worked out from the real dice, so on 2d6 a row for 7 shows 17% and one for 2 shows 3%.
- Each column has a box in every row. **Enter** adds a row, **Shift+Enter** starts a new line in a result, and **Tab** moves to the next box. Rows and columns can be added, moved and removed.
- **Link** on a row, or typing `[[`, links a result to another note or table. A result that links to a roll table gets a **Rolls on** box: ticked, the table is rolled and its result put in place; unticked, the link stays a link.
- **Paste a List...** adds a row for each line of a list copied from a web page or a PDF. Numbers at the start of every line, like `3-4`, are kept.
- **Roll** rolls the table as it is in the editor, to try it.
- **Done** puts the table back into the note. **Cancel** leaves it as it was.

Tables are still ordinary Markdown tables that Obsidian and other apps read. A line break in a result is saved as `<br>`. Which links don't roll is kept in `.dungeon_elements/tables.yaml`, so the notes stay plain tables.

## The die and the rows

The first header is the die: `d6`, `2d6`, `d100` or `d%`. Each row's first cell is the numbers it covers:

- One number, like `4`, or a range, like `2-3` or `01-05`.
- `00` is 100 on a d100, and `5+` is 5 and up.

**Problems** lists rows that don't start with a number, and numbers the die can roll that no row has.

## Rolling

- In the reading view, click the **Roll** button above the table.
- **View > Roll Tables** (Ctrl+Shift+T) lists every roll table in the campaign in the sidebar, in place of the file tree. Click a table's die to roll it, or its name to open it. Close the list with **✕** or **Escape**.
- In the dice roller, type the table's name, like `Weather`, and press **Enter**. A note with several tables takes its heading too: `Weather#Winter`.

The result shows in the dice roller, with what the die rolled. **Copy** copies it, and **Create New Note** makes a note named after it, in the folder you pick: roll a name, and make the NPC.

## Results that roll more

- A result that links to another roll table, like `[[Storms]]`, rolls that table and puts its result in place, unless its **Rolls on** box is unticked in the table editor. `[[Weather#Winter]]` rolls the table under the Winter heading of the Weather note. The tables it rolled on are listed under the result.
- A link to a note that isn't a table stays its name.
- Dice in a result are rolled: `1d4 days of rain` might give "3 days of rain".

A table that rolls on itself stops after 10 rolls.

## Tables with several columns

A table with more than one result column, like `| d20 | Name | Description |`, gives a whole row: each column's header and result.

Tick **Roll each column**, above the table in the reading view or under it in the Roll Tables list, to roll the die again for every column instead. A name table with First, Middle and Last columns then makes any mix of names, like "Wren Ash Halloway".

A link in a column rolls just the column of the same name in the table it links to, when that table has one. So a table with a First and a Family column, each listing `[[English names#Male]]`, `[[Japanese names#Male]]` and so on, takes a first name from one culture and a family name from another, like Isaac Akinaka. Columns after it whose results start with a hyphen, like a Norse name's -sson, come along too. Which tables roll each column is kept in the campaign, in `.dungeon_elements/tables.yaml`, so the notes stay plain tables.

Rolling each column puts the results together with spaces. A result that starts with a hyphen joins the one before it instead: `Ash` and `-ford` make "Ashford".

## Built-in tables

Some tables come with the app, listed under **Built in** in the Roll Tables list:

- **Names**, which picks one of the name tables below at random, and **Female names** and **Male names**, which do the same for women's or men's names.
- **Mixed female names** and **Mixed male names**, which roll the first name and the family name on cultures of their own, so they can differ, like a family with roots in more than one place.
- Names by culture: **English**, **Norse**, **Celtic**, **Latin**, **Greek**, **Arabic**, **Slavic** and **Japanese names**. Each has a **Female** and a **Male** table, with a first and a family name, so `[[Celtic names#Female]]` rolls a woman's name. Norse family names are built from the father's name and -sdottir or -sson, like Sigrid Haraldsdottir.
- **Fantasy names**, made-up names built from a start and an ending, like Aelwen or Kaeldor.
- **Place names** (like Ashford or Ravenmoor), **Inn and shop names** (The Crooked Lantern) and **Group names**, for factions, crews and ships (The Silent Hand).
- Story prompts, a few words to spark an idea, never the whole story: **Wants** (what someone is after), **Trouble** (what's going wrong), **Twists**, **What's here** (details for a room or site) and **Feel of a place**.
- World building, for the walkthrough's questions and your own notes: **Campaign names** (The Drowned Crown), **Threats** (a danger as big as the campaign), **Tone** (one word, like Grim or Cozy), **World names**, **Peoples** (the Ashborn, River folk), **Historic events** (The Long Winter) and their **Aftermath**, **Faiths** (The Order of the Open Hand), **Deity names** and **Domains** (what a god holds sway over), and for magic or strange technology, **Wonders** (its name, like the Weave), **Sources of power** and **Costs and limits**.
- Adventure, for the places and quests near the heroes: **Dangerous places** (The Sunken Crypt), **Quest names** (The Missing Miller), **Rumors**, **Plot hooks**, **Opening scenes** (how a session starts) and **Secrets**.

The world-building and adventure tables are kept genre-light, so they fit a fantasy, sci-fi or modern world.
- **Action and Theme**, two words to read into when you're stuck, like *Betray* and *Harvest*. It rolls each column, so you get one of each.

Roll them like any other table, link to them from your own (`[[Norse names]]`, `[[Norse names#Male]]`), or type their names in the dice roller. To change one, click **Copy** beside it: it's copied into your **tables** folder, and your copy is used in its place from then on, everywhere it's rolled. Right-click it and pick **Copy into Campaign and Edit** to open your copy in the table editor straight away.

## Fields that roll on a table

A text or long text field can roll on a table to fill itself in. In **Campaign > Note Types...**, pick the field's tables from **Add a table...**: a whole table, a part of one under a heading, like **English - Female**, or all its parts at once. Click a table to take it out. The first one is picked to start with. The field then has an **Inspire Me** button, in the form, in the reading view and in the walkthrough. Type a word in **Button says**, like Names, and it reads **Inspire Me - Names**.

With several tables, a dropdown beside the button picks which one it rolls on: **Any - Female**, **Mixed - Male**, **Norse - Female** and so on. The table picked is remembered for that field in this campaign, in `.dungeon_elements/tables.yaml`, until you pick another. In the type's file the tables are written with commas between them, like `table: Female names, Norse names#Male`.

- An empty field is filled with the result.
- A field with something in it asks first: **Replace** what's there, or **Cancel**. A long text field can also **Add on a New Line**.

The result shows in the dice roller too, so you can copy it or roll again. In new campaigns:

- An NPC's Name has **Inspire Me - Names**, with a dropdown of any, mixed or one culture's women's or men's names, and Wants rolls on **Wants**.
- A location's Name rolls on **Place names**, First impression on **Feel of a place**, and Description on **What's here**.
- A faction's Name rolls on **Group names**, and Goal on **Wants**.
- A quest's Name rolls on **Quest names**, and its Hook has a dropdown of **Rumors** and **Plot hooks**.
- A site's Name rolls on **Dangerous places**, and a session's Strong start on **Opening scenes**.
- Secrets on NPCs, factions, locations and lore notes roll on **Secrets**.
- The campaign note's Name rolls on **Campaign names**, Big picture on **Threats**, and Themes and tone on **Tone**.
- A people's Name rolls on **Peoples**, an event's Name on **Historic events** and Consequences on **Aftermath**, a religion's Name on **Faiths**, and a deity's Name on **Deity names** and Domains on **Domains**.
- Magic and technology notes roll their Name on **Wonders**, Source on **Sources of power**, and Cost and limits on **Costs and limits**.

## Roll table fields

A field in **Campaign > Note Types...** can be a **Roll table**, like Encounters on a location or Loot on a dungeon room. Each note of that type has the table editor in its form, always open, starting with a d6 table. It's saved as a `## Encounters` section with the table under it, so a step or another table can roll it with `[[Old Mill#Encounters]]`.

If a note's section has more than just a table in it, the field shows it as long text instead, so nothing in it is lost, with **Edit Table** to change the table.
