# Default campaign template

Each `<folder>.yaml` here becomes `<folder>/_type.yaml` in a new campaign when
"Starter folders" is picked under "Start from" in New Campaign. A file in a folder
here is a subfolder's type: `items/weapons.yaml` becomes `items/weapons/_type.yaml`,
a kind of Item that lists only the fields it adds (and `removed_fields`). Each file
must also be listed in `STARTER_TYPES` in `src/types.rs`. They define
the fields each note in that folder has. See PLAN.md (Content types) for the
format. Templates users make or import are `.zip` files of folders and their
`_type.yaml` files (see `src/templates.rs`).
