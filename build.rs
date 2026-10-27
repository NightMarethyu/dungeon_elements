//! Gives the Windows executable the app icon, so it shows in Explorer and the taskbar,
//! and builds the how-to guides in `help/` into the app.

use std::fmt::Write;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=assets/dungeon_elements.ico");
    guides();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/dungeon_elements.ico");
        resource
            .compile()
            .expect("could not add the icon to the Windows executable");
    }
}

/// Writes `guides.rs` for `src/help.rs`: every `help/<language>/*.md` as
/// `(language, file name, text)`, sorted, so adding a guide only takes adding its file.
fn guides() {
    println!("cargo:rerun-if-changed=help");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("help");

    let mut guides = Vec::new();
    for language in std::fs::read_dir(&root).expect("could not read the help folder") {
        let language = language.expect("could not read the help folder").path();
        if !language.is_dir() {
            continue;
        }
        println!("cargo:rerun-if-changed={}", language.display());
        for file in std::fs::read_dir(&language).expect("could not read a help folder") {
            let file = file.expect("could not read a help folder").path();
            if file.extension().is_some_and(|extension| extension == "md") {
                guides.push(file);
            }
        }
    }
    guides.sort();

    let mut code = String::from("&[\n");
    for file in &guides {
        let name = |path: &Path| path.file_name().unwrap().to_string_lossy().into_owned();
        let language = name(file.parent().unwrap());
        writeln!(
            code,
            "    ({language:?}, {:?}, include_str!({:?})),",
            name(file),
            file.display().to_string(),
        )
        .unwrap();
    }
    code.push(']');

    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("guides.rs");
    std::fs::write(out, code).expect("could not write the list of guides");
}
