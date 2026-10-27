# Help pages

The how-to guides in **Help > How-To Guides** are the Markdown files in this folder, built into the app when it is compiled.

- Each language has its own folder, named by its language code (`en` is English). English is the only one for now; see the i18n stretch goal in [PLAN.md](../PLAN.md).
- Each guide is one `.md` file. Its first `# Heading` is its title in the list, and the number at the start of the file name sets its place (`01-getting-started.md` comes first). The rest of the name is the guide's id.
- To add a guide, add a file. Nothing else needs changing: `build.rs` finds it.
- Link to another guide with its id, like `[Links](guide:links)`, or to a keyboard shortcut list with `[shortcuts](help:shortcuts)`. Web links open in the browser. The tests check that every `guide:` link points at a guide that exists.
- Write for someone who has never used the app: short steps, menu names in **bold** (**Edit > Types**), and keyboard shortcuts in brackets after them.
