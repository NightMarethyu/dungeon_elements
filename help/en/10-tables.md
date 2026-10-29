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

**File > New Roll Table...** makes one in a few clicks. You can also right-click a folder and pick **New Roll Table...**, or click **+** in the Roll Tables list.

- Give it a **Name**, and pick its **Folder** (a **tables** folder to start with, made when you make your first table).
- Pick the **Die**: d4, d6, d8, d10, d12, d20, d100 or 2d6.
- **Rows** is how many rows it has, one for each number on the die if you leave it empty. Fewer rows split the numbers evenly, so a d100 with 20 rows gets 01-05, 06-10 and so on.
- **Result columns** are the headers after the die. Add more for tables like Name and Description, or First and Last.
- **Paste a list** to copy a table from a web page or a PDF: one result a line. Numbers or ranges at the start of the lines, like `1.` or `3-4`, are kept. Otherwise the rows are numbered for you, and the die is picked to fit.

The note opens ready to fill in. You can also type a table into any note yourself.

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

- A result that links to another roll table, like `[[Storms]]`, rolls that table and puts its result in place. `[[Weather#Winter]]` rolls the table under the Winter heading of the Weather note. The tables it rolled on are listed under the result.
- A link to a note that isn't a table stays its name.
- Dice in a result are rolled: `1d4 days of rain` might give "3 days of rain".

A table that rolls on itself stops after 10 rolls.

## Tables with several columns

A table with more than one result column, like `| d20 | Name | Description |`, gives a whole row: each column's header and result.

Tick **Roll each column**, above the table in the reading view or under it in the Roll Tables list, to roll the die again for every column instead. A name table with First, Middle and Last columns then makes any mix of names, like "Wren Ash Halloway". Which tables roll each column is kept in the campaign, in `.dungeon_elements/tables.yaml`, so the notes stay plain tables.
