# Templates and sharing types

The folders and fields you build for one game can start your next campaign, and you can share them with other people who use Dungeon Elements. A GM who runs D&D, Cairn and Cyberpunk can keep a template for each.

## Save your campaign's folders as a template

1. Open the campaign whose folders and fields you want to reuse, and choose **Edit > Types...**.
2. Choose **Export Campaign Structure...** at the bottom.
3. Save it. The save window starts in your templates folder: save it there to start new campaigns from it, or anywhere else to share it.

A template is an ordinary **.zip** file holding the campaign's folders and the **_type.yaml** file of each folder that has fields, subfolders and all. It never holds your notes, so it's small and has no spoilers in it.

## Start a campaign from a template

In **New Campaign...**, pick the template under **Start from**. The line under it lists the folders it makes. Every template in your templates folder is listed, by its file name. **Show my templates folder** opens that folder, to rename or delete templates.

## Share a single type

1. In **Edit > Types...**, pick the folder whose fields you want to share, like Weapons.
2. Choose **Export Type...** and save it. The file is named after the type, like **Weapon.type.yaml**, and holds every field the folder's notes have, including the ones it gets from the folder it's in.

Types saved in your types folder (where the save window starts) can be used in any campaign:

- In **Edit > Types...**, a folder with no fields can **start from a type saved for every campaign**.
- The **+** at the bottom of the file tree offers them for a new folder, marked **(saved type)**.

Save your changes in **Edit > Types...** before exporting: the export buttons wait until there are none.

## Use a template or type someone shared

Choose **Import...** in **Edit > Types...** or in **New Campaign...**, and pick the file.

- A **.zip** becomes a template for new campaigns. A folder zipped with your file manager works too, even with notes in it: only its folders and **_type.yaml** files are used.
- A **.yaml** type becomes a saved type.

Each is checked first, so a broken file is refused with a message saying what's wrong with it.

## Where they are kept

Templates, saved types and the list of campaigns in **Open Campaign...** are kept in the app's config folder, beside your themes: `%APPDATA%\dungeon_elements` on Windows, and `~/.config/dungeon_elements` on Linux. The templates are in **templates**, the types in **types**, and the campaign list is **campaigns.yaml**.
