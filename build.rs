//! Gives the Windows executable the app icon, so it shows in Explorer and the taskbar.

fn main() {
    println!("cargo:rerun-if-changed=assets/dungeon_elements.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/dungeon_elements.ico");
        resource
            .compile()
            .expect("could not add the icon to the Windows executable");
    }
}
