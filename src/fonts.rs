//! File-based font fallbacks for game interfaces that bypass Wine's font mapper.
use crate::{core, mods};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

pub const GOOGLE_SANS: &[u8] = include_bytes!("../assets/fonts/GoogleSans.ttf");
const LICENSE: &[u8] = include_bytes!("../assets/fonts/OFL.md");
const README: &[u8] = b"ALOL game font fallbacks\n\n\
segoeui.ttf and seguisb.ttf, when missing, are supplied as file aliases for\n\
unmodified Google Sans. These are not Microsoft Segoe UI fonts.\n\
Internal font names and bytes are unchanged. See OFL.md for copyright and license.\n\
Existing font files and links are preserved. No host fonts are changed.\n";

fn publish_font(path: &Path) -> core::Result<bool> {
    let temporary = path.with_file_name(format!(
        ".alol-font-{}-{}-{}.tmp",
        std::process::id(),
        core::stamp(),
        path.file_name().unwrap().to_string_lossy()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(&temporary)
        .map_err(|e| format!("Не создать временный файл шрифта: {e}"))?;
    let result = (|| -> std::io::Result<bool> {
        file.write_all(GOOGLE_SANS)?;
        file.sync_all()?;
        // Publish a complete file without overwriting an existing font. Each
        // alias has its own inode; editing one must not alter the other one.
        match fs::hard_link(&temporary, path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(e),
        }
    })();
    drop(file);
    let _ = fs::remove_file(temporary);
    result.map_err(|e| format!("Не подготовить шрифт {}: {e}", path.display()))
}

pub fn ensure_game_fonts(prefix: &Path) -> core::Result<()> {
    let drive = prefix.join("drive_c");
    let directory = mods::target(&drive, Path::new("windows/Fonts"))?;
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let present: Vec<_> = fs::read_dir(&directory)
        .map_err(|e| e.to_string())?
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|entry| entry.file_name().to_string_lossy().to_ascii_lowercase())
        .collect();
    let missing: Vec<_> = ["segoeui.ttf", "seguisb.ttf"]
        .into_iter()
        .filter(|name| !present.iter().any(|p| p == name))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let notices = mods::target(&drive, Path::new("alol/fonts"))?;
    for (name, bytes) in [("OFL.md", LICENSE), ("README.txt", README)] {
        let path = mods::target(&notices, Path::new(name))?;
        if fs::read(&path).ok().as_deref() != Some(bytes) {
            mods::atomic_bytes(&path, bytes)?;
        }
    }
    let mut added = 0;
    for name in missing {
        added += usize::from(publish_font(&directory.join(name))?);
    }
    if added > 0 {
        println!(
            "Шрифты меню игры: добавлен Google Sans с кириллицей ({added} файла только в префиксе Wine)."
        );
    }
    Ok(())
}
