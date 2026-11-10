//! Keep AppImage libraries in ALOL, and host tools in the host environment.
use std::{env, ffi::OsStr, path::PathBuf, process::Command};

// Keep in sync with packaging/appimage/AppRun.sh.
const HOST_VARIABLES: &[&str] = &[
    "PATH",
    "LD_LIBRARY_PATH",
    "LD_PRELOAD",
    "XDG_DATA_DIRS",
    "GSETTINGS_SCHEMA_DIR",
    "GDK_PIXBUF_MODULE_FILE",
    "GDK_PIXBUF_MODULEDIR",
    "GTK_EXE_PREFIX",
    "GTK_DATA_PREFIX",
    "GTK_PATH",
    "GTK_IM_MODULE_FILE",
    "GTK_THEME",
    "GI_TYPELIB_PATH",
    "GDK_BACKEND",
    "GIO_MODULE_DIR",
    "GIO_EXTRA_MODULES",
];

pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    if env::var_os("ALOL_APPIMAGE_ACTIVE").as_deref() != Some(OsStr::new("1")) {
        return command;
    }
    for name in HOST_VARIABLES {
        let saved = format!("ALOL_APPIMAGE_HOST_{name}");
        if let Some(value) = env::var_os(&saved) {
            use std::os::unix::ffi::{OsStrExt, OsStringExt};
            let bytes = value.as_bytes();
            if bytes.first() == Some(&b's') {
                command.env(name, std::ffi::OsString::from_vec(bytes[1..].to_vec()));
            } else {
                command.env_remove(name);
            }
        }
        command.env_remove(saved);
    }
    for name in [
        "ALOL_APPIMAGE_ACTIVE",
        "APPIMAGE",
        "APPDIR",
        "ARGV0",
        "OWD",
        "APPIMAGE_EXTRACT_AND_RUN",
    ] {
        command.env_remove(name);
    }
    command
}

pub fn appimage() -> Option<PathBuf> {
    if env::var_os("ALOL_APPIMAGE_ACTIVE").as_deref() != Some(OsStr::new("1")) {
        return None;
    }
    let directory = PathBuf::from(env::var_os("APPDIR")?).canonicalize().ok()?;
    let current = env::current_exe().ok()?.canonicalize().ok()?;
    if current != directory.join("usr/bin/alol").canonicalize().ok()? {
        return None;
    }
    let image = PathBuf::from(env::var_os("APPIMAGE")?);
    if !image.is_absolute() || !image.is_file() {
        return None;
    }
    image.canonicalize().ok()
}

pub fn executable() -> std::io::Result<PathBuf> {
    appimage().map_or_else(env::current_exe, Ok)
}
