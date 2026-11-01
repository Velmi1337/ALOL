//! A Russian Wine environment without changing the host's language or locale archive.
use crate::core::data_dir;
use std::{ffi::CStr, fs, path::PathBuf, process::Command, sync::OnceLock};

const RUSSIAN: &str = "ru_RU.UTF-8";
static PRIVATE: OnceLock<Option<PathBuf>> = OnceLock::new();

unsafe extern "C" {
    fn gnu_get_libc_version() -> *const std::ffi::c_char;
}

fn usable(path: Option<&std::path::Path>) -> bool {
    let mut command = Command::new("locale");
    command.env("LC_ALL", RUSSIAN).arg("charmap");
    if let Some(path) = path {
        command.env("LOCPATH", path);
    }
    command.output().is_ok_and(|output| {
        output.status.success()
            && output.stderr.is_empty()
            && String::from_utf8_lossy(&output.stdout)
                .trim()
                .eq_ignore_ascii_case("UTF-8")
    })
}

fn private_locale() -> Option<PathBuf> {
    if usable(None) {
        return None;
    }
    // Compiled locales belong to the libc version that generated them. Regenerate
    // after libc upgrades instead of reusing potentially incompatible data.
    let version = unsafe { CStr::from_ptr(gnu_get_libc_version()) }.to_string_lossy();
    let root = data_dir().join("locales").join(format!("glibc-{version}"));
    if usable(Some(&root)) {
        return Some(root);
    }
    let generated = (|| -> std::io::Result<()> {
        fs::create_dir_all(&root)?;
        let temporary = root.join(format!(".ru-{}", std::process::id()));
        // An interrupted build is never published as a complete locale.
        let _ = fs::remove_dir_all(&temporary);
        let result = Command::new("localedef")
            .env("LC_ALL", "C")
            .env_remove("LOCPATH")
            .args(["--no-archive", "-i", "ru_RU", "-f", "UTF-8"])
            .arg(&temporary)
            .output();
        let target = root.join(RUSSIAN);
        let complete = result.is_ok_and(|output| output.status.success())
            && temporary.join("LC_CTYPE").is_file()
            && temporary.join("LC_MESSAGES/SYS_LC_MESSAGES").is_file();
        if complete {
            // The prefix lock serializes a profile; another profile may already
            // have published this shared cache. Keep its valid copy in that case.
            if fs::rename(&temporary, &target).is_err() && !usable(Some(&root)) {
                let _ = fs::remove_dir_all(&target);
                fs::rename(&temporary, &target)?;
            }
        }
        let _ = fs::remove_dir_all(&temporary);
        Ok(())
    })();
    if generated.is_ok() && usable(Some(&root)) {
        println!("Русская локаль создана только для ALOL: {}", root.display());
        Some(root)
    } else {
        // Wine's unix_to_win_locale explicitly uses LC_ALL when setlocale falls
        // back to C. Its own NLS files still provide ru-RU and Windows-1251.
        // Minimal hosts without localedef/locale sources can therefore continue.
        eprintln!(
            "Локальная локаль glibc недоступна; используется встроенная ru-RU Wine. Системные локали не изменены."
        );
        None
    }
}

pub fn configure(command: &mut Command) {
    command
        .env("LANG", RUSSIAN)
        .env("LC_ALL", RUSSIAN)
        .env("LANGUAGE", "ru_RU:ru");
    if let Some(path) = PRIVATE.get_or_init(private_locale) {
        command.env("LOCPATH", path);
    }
    // All changes are Command-local. Never set environment variables in the GUI
    // process or write to /etc/locale.gen, /etc/default/locale or locale-archive.
}

pub fn description() -> String {
    let version = unsafe { CStr::from_ptr(gnu_get_libc_version()) }.to_string_lossy();
    format!(
        "ru-RU только для Wine; кэш glibc {version}: {}",
        data_dir().join("locales").display()
    )
}
