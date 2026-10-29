# Rolling dice

## The dice roller

Click the **d20** button at the right of the tab bar to open the dice roller under your note.

- Click a die to roll it. Set **Count** to roll more than one, and press **Enter** in the box to roll again.
- Type an expression and press **Enter**: `2d6+3`, `d%` (a d100), or `4d6kh3` (roll four, keep the highest three). A bare modifier like `+5` rolls a d20 and adds 5.
- Type the name of a roll table, like `Weather`, and press **Enter** to roll on it. See [Roll tables](guide:tables).
- Pick **Advantage** or **Disadvantage** to roll a d20 twice and keep the higher or lower one. Tick **Any die** for systems that allow advantage on other dice.

The last few rolls stay on the left, so you can look back.

## Rolling from a note

Fields with dice in them, like a weapon's Damage `1d8 + 3 slashing`, get a roll button next to them in the form and in the reading view.

## Dice fields

A **dice** field holds one roll, like a weapon's Damage or a monster's Attack, picked in parts instead of typed:

- How many dice, and which die (d4, d6, d8, d10, d12, d20, d100, and d2 and d3).
- A modifier, like `+2` or `-1`.
- Something special: **Advantage** or **Disadvantage** (roll one more die and keep the best or worst, so a d20 is rolled twice), or **Keep highest** or **Keep lowest**, with how many to keep.

The roll button next to the field rolls it with its own advantage or disadvantage, whatever the dice roller is set to. Add a dice field to a folder in **Edit > Types...**, or to one note with the **+** button.

A dice field is saved as text that reads the way you'd say it: `2d6+3`, `4d6kh3` (keep the highest three) or `1d20+5 advantage`. Obsidian and other editors show that text, and you can type it there too. Text with more than one kind of die, like `1d8+1d6`, is kept as written and shown as a text box; empty the box to pick the dice in parts.
