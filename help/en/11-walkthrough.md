# The campaign walkthrough

The walkthrough builds your world with you, one question at a time: what the campaign is about, the world and its regions, who holds power, where the heroes start, who the heroes are, and how the first session opens. Each answer goes into a note, and the notes it needs are made for you, linked to each other.

## Starting it

- When you make a campaign, **Start the walkthrough** is ticked. Leave it ticked, and the walkthrough opens once the campaign is made.
- **Campaign > Campaign Walkthrough...** opens it any time, at the question you were on.
- With no note open, click **Build your world with the walkthrough**.

The walkthrough opens in a tab of its own. Close it like any other tab and open it again later: your answers are already in their notes, and the walkthrough remembers where you were.

## Answering

The chapters are listed on the left, with how many of their questions are answered. Click one to go to it.

On the right is one question, with the box you answer in. Under it, **Goes into** names the note the answer is written into, and the field it fills. Click the note's name to open it.

- **Next** writes the answer and goes on. In a one-line answer, **Enter** does the same, and in a long one, **Ctrl+Enter**.
- **Back** goes to the question before.
- **Skip** leaves the question for later, and **Skip Chapter** leaves the rest of the chapter.
- **Inspire Me** rolls on a roll table for an idea and puts it in the answer, like a name for a faction or an opening scene for the first session. Every question has a table of its own, apart from the ones only you can answer, like which game you'll play. Roll again for another. A question with several tables has a dropdown beside the button to pick one, like **Norse - Male** for a name. Your pick is remembered for that field, here and in the note's form. If the answer already has something in it, you're asked whether to replace it or add the roll on a new line.

An empty answer writes nothing, so pressing **Next** without typing is the same as skipping.

## Names and lists

Some questions ask for names: the campaign's, the world's, or a list like the regions or factions. A list has a box for each name:

- Type a name and press **Enter** for a box for the next one, or click **Add Another**. Pressing **Enter** in an empty box at the end goes on to the next question.
- Click **✕** to take a name out.
- **Paste a list** to fill a box for each name: names on separate lines, like a list copied from a name generator website, or between commas. Names copied all in lower case get capital letters.
- **Inspire Me - Names** rolls a name into the next empty box, from the culture picked in the dropdown beside it.

Your players can write up their own characters. Click **Add Characters from Files...** under "Who are the players' characters?" and pick their Markdown files, or drag the files onto the window. Each is copied into the players folder as a player character, and added to the list. The file itself stays where it was. When you press **Next**, the character is added to the campaign's Players, and fields it left empty, like Status and Home, are filled in. Fields the player filled in are kept.

Each name makes a note in its folder, and the questions after it are asked about each one: "What do the Iron Guild want?", then the same for the next faction.

Changing a name you gave before renames its note, and its links with it.

## Links

Answers that name other notes, like a faction's leader or a hero's bonds, link to them.

- The notes you've made that fit are shown under the answer. Click one to pick it.
- Or type a name: notes that match are offered as you type (**Tab** takes the first). For a name with no note yet, click **+ New** to make it there and then, or press **Next** and it is made for you. Some questions can make more than one kind of note: who rules a town can be a new faction or a new NPC.
- In a long answer or a step, type `[[` to start a link.

So answering "Who leads the Iron Guild?" with a name makes an NPC with that name, and the faction links to them.

Notes the walkthrough makes show in the file tree, with their folders opened. **▾** and **▸** at the top of the file tree open and close every folder at once.

## Changing your answers

Every answer is an ordinary field in an ordinary note. Change it in the note, and the walkthrough shows the new answer next time. A note you rename or move is still found.

If a note is open with changes you haven't saved, the walkthrough's answer goes into its tab instead, so your changes aren't written over. Save the tab to keep both.

At the end of each chapter, a summary lists the notes it made. Click one to open it. When you reach the end, open any chapter to change answers or fill in ones you skipped.

## Your own folders

The walkthrough asks about the starter folders. If a campaign doesn't have a folder or a field a question is about, that question is left out, so the walkthrough fits campaigns whose folders you've changed.

A template can bring questions of its own, made for its folders, and a campaign made from it asks those instead. They go with the campaign's folders when you export them as a template (see the Templates guide).

## Your own questions

**Campaign > Walkthrough Questions...** changes the questions this campaign asks.

1. A campaign starts out asking the built-in questions, which get better as the app updates. Click **Start from the Default Questions** to make a copy of them for this campaign to change.
2. The chapters are listed on the left. Click one to see its questions, or **Add Chapter** for a new one. A chapter has a title, a line shown at its top, and **↑**, **↓** and **Remove**.
3. Each question has its text, a hint under it if you like, and two choices:
   - **Goes into**: the note the answer is written into, like *factions: several new Factions*.
   - **Field**: which of that note's fields the answer fills. **Its name** (or **Their names**, for several notes) asks for the note's name instead.

   Under them, **Inspire Me rolls on the field's tables** is ticked to use the tables the field has in Campaign > Note Types. Untick it to pick the question's own: **Add a table...** adds a table, a part of one, like **English - Female**, or all its parts at once, and clicking a table takes it out. With more than one, the person answering picks from a dropdown; the first is picked to start with. **Button says** adds a word to the button, like Names for **Inspire Me - Names**. With no tables, the question has no Inspire Me button.

   Write `{name}` in a question about several notes, like "What do {name} want?", and it is asked about each one. A question whose field links to other notes can offer notes from other folders too: add them under **Pick from**. A question that asks for the names of several notes can have a button that adds notes from Markdown files, like the heroes question does.
4. **Notes it makes** lists the notes the questions write into. Each has a short name the questions use, its folder, whether a list of names makes several, and how a single note is named: asked for, after the campaign, or always the same name, like Session 1. **Fill In a Field** gives a field a value when the note is made. A value that is the short name of another of these notes links to it. **Also link to it from** adds a link to it in a field of another note, like the campaign's Players for each hero.

Mistakes show at the bottom of the dialog in plain words, like a question with no text, and must be fixed before saving. Questions the campaign can't ask, like one about a field its type no longer has, are listed there too. They can still be saved, and the walkthrough leaves them out.

**Save** keeps the questions for this campaign. **Save and Try It** opens the walkthrough with them. **Use the Built-In Questions** goes back to the built-in ones.
