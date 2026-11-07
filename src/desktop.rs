//! Desktop identity for installed packages and standalone/source executables.
use crate::{
    core::{self, Result},
    mods,
    project::Project,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub const ICON: &[u8] = include_bytes!("../assets/alol.png");
pub const ARIZONA_ICON: &[u8] = include_bytes!("../assets/arizona.png");

pub fn application_id() -> &'static str {
    if development_copy() {
        "alol-dev"
    } else {
        "alol"
    }
}

pub fn arizona_game_id() -> &'static str {
    if development_copy() {
        "umu-alolarizonadev"
    } else {
        "umu-alolarizona"
    }
}

pub fn game_id(project: Project) -> &'static str {
    match project {
        Project::Arizona => arizona_game_id(),
        Project::Rodina if development_copy() => "umu-alolrodinadev",
        Project::Rodina => "umu-alolrodina",
    }
}

pub fn integrate_game(project: Project) -> Result<()> {
    if project == Project::Arizona {
        return integrate_arizona();
    }
    let executable = development_root()
        .map(|p| p.join("run-dev.sh"))
        .or_else(|| env::current_exe().ok())
        .ok_or("Не найден ALOL")?;
    let icon_path = metadata_root().join("icons/hicolor/256x256/apps/alol-rodina.png");
    let bytes: &[u8] = include_bytes!("../assets/rodina.png");
    if fs::read(&icon_path).ok().as_deref() != Some(bytes) {
        mods::atomic_bytes(&icon_path, bytes)?;
    }
    let class = format!("steam_app_{}", game_id(project).trim_start_matches("umu-"));
    let entry = metadata_root()
        .join("applications")
        .join(format!("{class}.desktop"));
    let previous = fs::read_to_string(&entry).ok();
    if previous.as_ref().is_some_and(|text| {
        !text
            .lines()
            .any(|line| line == "X-ALOL-WindowMetadata=true")
    }) {
        return Ok(());
    }
    let text = desktop_entry(&executable)?
        .replace("Name=ALOL\n", "Name=Родина RP · ALOL\n")
        .replace(
            "Icon=alol\n",
            &format!(
                "Icon={}\n",
                icon_path.to_string_lossy().replace('\\', "\\\\")
            ),
        )
        .replace(
            "StartupWMClass=alol\n",
            &format!("StartupWMClass={class}\n"),
        );
    let text = text.replace("\nIcon=", " --project rodina launch\nIcon=")
        + "NoDisplay=true\nX-ALOL-WindowMetadata=true\n";
    if previous.as_deref() != Some(&text) {
        mods::atomic_bytes(&entry, text.as_bytes())?;
    }
    Ok(())
}

fn metadata_root() -> PathBuf {
    env::var_os("ALOL_DESKTOP_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| core::data_dir().parent().unwrap().to_path_buf())
}

pub fn integrate_arizona() -> Result<()> {
    let executable = development_root()
        .map(|p| p.join("run-dev.sh"))
        .or_else(|| env::current_exe().ok())
        .ok_or("Не найден ALOL")?;
    let icon_path = metadata_root().join("icons/hicolor/256x256/apps/alol-arizona.png");
    if fs::read(&icon_path).ok().as_deref() != Some(ARIZONA_ICON) {
        mods::atomic_bytes(&icon_path, ARIZONA_ICON)?;
    }
    let icon = icon_path.to_string_lossy().replace('\\', "\\\\");
    // Migrate the icon of our older Arizona entries without changing their
    // Exec/profile. In particular a checkout must not redirect installed games.
    for class in [
        "steam_app_alolarizona",
        "steam_app_alolarizonadev",
        "arizona games launcher.exe",
        "arizona launcher.exe",
        "arizonalauncher.exe",
    ] {
        let entry = metadata_root()
            .join("applications")
            .join(format!("{class}.desktop"));
        if let Ok(previous) = fs::read_to_string(&entry)
            && previous
                .lines()
                .any(|line| line == "X-ALOL-WindowMetadata=true")
        {
            let text = previous
                .lines()
                .map(|line| {
                    if line.starts_with("Icon=") {
                        format!("Icon={icon}")
                    } else {
                        line.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            if text != previous {
                mods::atomic_bytes(&entry, text.as_bytes())?;
            }
        }
    }
    // A dedicated UMU identity avoids assigning Arizona's icon to every other
    // game using Proton's generic steam_app_default class.
    let proton_class = format!("steam_app_{}", arizona_game_id().trim_start_matches("umu-"));
    for class in [
        proton_class.as_str(),
        "arizona games launcher.exe",
        "arizona launcher.exe",
        "arizonalauncher.exe",
    ] {
        let entry = metadata_root()
            .join("applications")
            .join(format!("{class}.desktop"));
        let previous = fs::read_to_string(&entry).ok();
        if development_copy() && class != proton_class && previous.is_some() {
            continue;
        }
        let mut text = desktop_entry(&executable)?
            .replace("Name=ALOL\n", "Name=Arizona · ALOL\n")
            .replace("Icon=alol\n", &format!("Icon={icon}\n"))
            .replace(
                "StartupWMClass=alol\n",
                &format!("StartupWMClass={class}\n"),
            );
        // Opening the associated executable is delegated to ALOL's launcher action.
        let start = text.find("Exec=").unwrap();
        let end = start + text[start..].find('\n').unwrap();
        text.insert_str(end, " launch");
        text.push_str("NoDisplay=true\nX-ALOL-WindowMetadata=true\n");
        if previous
            .as_ref()
            .is_none_or(|s| s.lines().any(|line| line == "X-ALOL-WindowMetadata=true"))
            && previous.as_deref() != Some(&text)
        {
            mods::atomic_bytes(&entry, text.as_bytes())?;
        }
    }
    Ok(())
}

fn development_root() -> Option<PathBuf> {
    env::current_exe()
        .ok()?
        .ancestors()
        .find(|directory| {
            directory.join("Cargo.toml").is_file() && directory.join("src/desktop.rs").is_file()
        })
        .map(Path::to_path_buf)
}

pub fn development_copy() -> bool {
    env::var_os("ALOL_DEVELOPMENT").is_some_and(|value| value == "1")
        || development_root().is_some()
}

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
    // Wayland shells look up window icons by desktop ID, not _NET_WM_ICON.
    // A hidden development identity supplies that metadata without a menu item
    // or an installed executable, and cannot shadow the installed alol.desktop.
    if development_copy() {
        let Some(project) = development_root() else {
            return Ok(());
        };
        let root = metadata_root();
        let icon = project.join("assets/alol.png");
        let mut text = desktop_entry(&project.join("run-dev.sh"))?
            .replace("Name=ALOL\n", "Name=ALOL · Разработка\n")
            .replace("StartupWMClass=alol\n", "StartupWMClass=alol-dev\n")
            .replace(
                "Icon=alol\n",
                &format!("Icon={}\n", icon.to_string_lossy().replace('\\', "\\\\")),
            );
        text.push_str("NoDisplay=true\nX-ALOL-Development=true\n");
        let entry = root.join("applications/alol-dev.desktop");
        let previous = fs::read_to_string(&entry).ok();
        if previous
            .as_ref()
            .is_none_or(|s| s.lines().any(|line| line == "X-ALOL-Development=true"))
            && previous.as_deref() != Some(&text)
        {
            mods::atomic_bytes(&entry, text.as_bytes())?;
        }
        return Ok(());
    }
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
