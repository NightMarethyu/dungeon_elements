# Releasing Dungeon Elements

How to build the release files and publish a version on GitHub. The examples use v0.1.0; change the version number for later releases.

## 1. Get the code ready

1. Merge everything that goes in the release into `main`, then update your copy:

   ```sh
   git checkout main
   git pull
   ```

2. Check that `version` in `Cargo.toml` is the version you are releasing (`0.1.0`), and that `CHANGELOG.md` has a section for it with today's date. For the next release, move the `[Unreleased]` notes into a new version section and bump `version` (then run `cargo build` once so `Cargo.lock` picks it up, and commit both).

3. Run the tests:

   ```sh
   cargo test
   ```

## 2. Build the Windows release (on your Windows PC)

You already have the Rust MSVC toolchain and the Visual Studio **Desktop development with C++** workload, which is all this needs.

1. In PowerShell, in the repository folder:

   ```powershell
   cargo build --release
   ```

   This makes `target\release\dungeon_elements.exe`. It is a single file: the themes, templates, and logo are built in, the icon is embedded (from `assets\dungeon_elements.ico`, see `build.rs`), and the C runtime is linked in (see `.cargo\config.toml`), so it runs on PCs without the Visual C++ Redistributable. Release builds open without a console window.

2. Try it: double-click `target\release\dungeon_elements.exe`, check that the window and taskbar show the colored logo, and open a campaign.

3. Zip it with the README, changelog, and license:

   ```powershell
   $v = "0.1.0"
   New-Item -ItemType Directory -Force dist | Out-Null
   Compress-Archive -Force -DestinationPath "dist\dungeon_elements-v$v-windows-x86_64.zip" `
     -Path target\release\dungeon_elements.exe, README.md, CHANGELOG.md, LICENSE
   ```

## 3. Build the Linux release (in WSL on the same PC)

The Linux build has to be made on Linux. On Windows the easiest way is WSL with Ubuntu 22.04, whose older system libraries let the binary run on most current distros (a binary built on a newer distro may not start on older ones).

1. Install WSL with Ubuntu 22.04, if you haven't (PowerShell as administrator, then restart and pick a username and password):

   ```powershell
   wsl --install -d Ubuntu-22.04
   ```

2. In the Ubuntu terminal, install a C compiler and Rust:

   ```sh
   sudo apt update
   sudo apt install -y build-essential pkg-config git
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
   source "$HOME/.cargo/env"
   ```

3. Clone the repository inside WSL (building from `/mnt/c/...` works but is much slower) and build:

   ```sh
   git clone https://github.com/NightMarethyu/dungeon_elements.git ~/dungeon_elements
   cd ~/dungeon_elements
   cargo build --release
   ```

   For later releases, `cd ~/dungeon_elements && git pull` instead of cloning again.

4. Optional: try it. Windows 11 shows Linux windows from WSL, so `./target/release/dungeon_elements` should open the app. If it complains about `libxkbcommon-x11`, run `sudo apt install -y libxkbcommon-x11-0` first.

5. Package it with the icon and a desktop entry, and copy the archive to your Windows `dist` folder:

   ```sh
   v=0.1.0
   name=dungeon_elements-v$v-linux-x86_64
   rm -rf "dist/$name" && mkdir -p "dist/$name"
   cp target/release/dungeon_elements README.md CHANGELOG.md LICENSE \
      assets/dungeon_elements.png assets/dungeon_elements.desktop "dist/$name/"
   tar -czf "dist/$name.tar.gz" -C dist "$name"
   cp "dist/$name.tar.gz" /mnt/c/CodeBase/dungeon_elements/dist/
   ```

   People run it with `./dungeon_elements` after unpacking. To add it to their app menu, they copy `dungeon_elements` to `~/.local/bin/`, `dungeon_elements.png` to `~/.local/share/icons/hicolor/256x256/apps/`, and `dungeon_elements.desktop` to `~/.local/share/applications/`.

## 4. Publish the release on GitHub

1. Tag the release commit and push the tag (from Windows or WSL, on an up-to-date `main`):

   ```sh
   git tag -a v0.1.0 -m "Dungeon Elements v0.1.0"
   git push origin v0.1.0
   ```

2. On GitHub, open **Releases** > **Draft a new release**.
3. Pick the tag `v0.1.0` and title it **Dungeon Elements v0.1.0**.
4. Paste the `0.1.0` section of `CHANGELOG.md` as the description.
5. Attach both files from `dist`:
   - `dungeon_elements-v0.1.0-windows-x86_64.zip`
   - `dungeon_elements-v0.1.0-linux-x86_64.tar.gz`
6. Tick **Set as a pre-release** if you want to signal that it's early (0.x), then **Publish release**.

## Notes

- If the Windows build stops with "could not add the icon to the Windows executable", the resource compiler (`rc.exe`) wasn't found: make sure a Windows 10/11 SDK is ticked in the Visual Studio Installer under **Desktop development with C++**.
- The builds aren't code-signed, so Windows SmartScreen may say it "protected your PC" the first time; **More info** > **Run anyway** opens the app. Signing needs a paid certificate and can come later.
- To regenerate the icon files after the logo changes, render `reference/dungeon_elements_colors.svg` to PNGs at 16, 24, 32, 48, 64, 128, and 256 pixels, save the 256 one as `assets/dungeon_elements.png`, and combine them all into `assets/dungeon_elements.ico` (for example with ImageMagick: `magick icon-*.png assets/dungeon_elements.ico`). The window icon is drawn from the SVG when the app starts, so it needs no extra step.
