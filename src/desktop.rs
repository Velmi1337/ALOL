//! Desktop identity for installed packages and standalone/source executables.
use crate::{
    core::{self, Result},
    mods,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub const ICON: &[u8] = include_bytes!("../assets/alol.png");

fn desktop_entry(executable: &Path) -> Result<String> {
    let path = executable.to_str().ok_or("Путь ALOL не в UTF-8")?;
    if path.chars().any(char::is_control) {
        return Err("Недопустимый путь ALOL".into());
    }
    let mut escaped = path.replace('%', "%%");
    for character in ['\\', '"', '`', '$'] {
        escaped = escaped.replace(character, &format!("\\{character}"));
    }
    escaped = escaped.replace('\\', "\\\\");
    Ok(format!(
        "[Desktop Entry]\nX-ALOL-Managed=true\nType=Application\nName=ALOL\nGenericName=Arizona Launcher On Linux\nExec=\"{escaped}\"\nIcon=alol\nStartupWMClass=alol\nTerminal=false\nCategories=Game;\nStartupNotify=false\n"
    ))
}

pub fn integrate() -> Result<()> {
    let root = core::data_dir()
        .parent()
        .ok_or("Нет папки данных")?
        .to_path_buf();
    let mut directories: Vec<PathBuf> = env::split_paths(
        &env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
    )
    .filter(|p| p.is_absolute())
    .collect();
    directories.push(root.clone());
    let icon_relative = "icons/hicolor/256x256/apps/alol.png";
    if !directories.iter().any(|d| d.join(icon_relative).is_file()) {
        mods::atomic_bytes(&root.join(icon_relative), ICON)?;
    }
    // Only maintain entries created by ALOL. A package installation takes precedence
    // over our source/standalone fallback; unrelated custom entries remain untouched.
    let entry = root.join("applications/alol.desktop");
    let previous = fs::read_to_string(&entry).ok();
    let owned = previous
        .as_ref()
        .is_some_and(|text| text.lines().any(|l| l == "X-ALOL-Managed=true"));
    let packaged = directories
        .iter()
        .filter(|d| *d != &root)
        .any(|d| d.join("applications/alol.desktop").is_file());
    if packaged && owned {
        fs::remove_file(&entry).map_err(|e| e.to_string())?;
    } else if !packaged && (previous.is_none() || owned) {
        let executable = env::current_exe()
            .and_then(|p| p.canonicalize())
            .map_err(|e| e.to_string())?;
        let text = desktop_entry(&executable)?;
        if previous.as_deref() != Some(&text) {
            mods::atomic_bytes(&entry, text.as_bytes())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_identity_and_exec_escaping() {
        let text = desktop_entry(Path::new("/tmp/ALOL test/a%$\".bin")).unwrap();
        assert!(text.contains("StartupWMClass=alol\n"));
        assert!(text.contains("a%%\\\\$\\\\\".bin"));
        assert!(desktop_entry(Path::new("/tmp/a\nb")).is_err());
    }
}
