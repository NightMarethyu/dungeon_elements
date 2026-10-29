# Searching your notes

The search box at the top of the file tree finds notes as you type. Click in it, or press **Ctrl+Shift+F** (**Edit > Search Notes**).

## Find a note

- Type part of a name. Notes whose name or alias matches come first, then notes that mention it anywhere in their text.
- Each note found shows its folder and the line that matched. Click it to open it, or press **Enter** to open the first one.
- Every word has to be in the note, so `orc cook` finds the half-orc cook and not every orc.
- Put quotes around words that go together: `"black dragon"`.
- Clear the box with **✕**, or press **Escape**, to see the file tree again.

## Find notes by a field

Type a field's name, a colon, and what it should hold:

- `status:alive` finds everyone whose Status is Alive.
- `location:Saltmarsh` finds notes whose Location links to Saltmarsh.
- `factions:"Thieves Guild"` finds notes with the Thieves Guild among their Factions. Use quotes when a value has spaces.
- `known_to_players:no` finds what the players haven't learned yet. Checkboxes are `yes` or `no`, and a field can go by its label or its key.
- `secrets:dragon` looks in a long text field, the text under its heading.

Words and fields can be mixed: `cook status:alive location:Saltmarsh`.

## Search one folder

Right-click a folder in the file tree and pick **Search in Folder**. Only that folder and its subfolders are searched (an NPC search includes Enemies and Deities), and with the box empty it lists every note in the folder. Click **Search everywhere** to search the whole campaign again.

So to find every NPC in a town, right-click **NPCs**, pick **Search in Folder**, and type `location:` and the town's name.
