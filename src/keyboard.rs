//! An optional, per-game XKB indicator bridge for the explicitly patched runtime.
use crate::{core, json, mods};
use std::{
    env, fs,
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MONITOR: &str = include_str!("../runtime/xkb_layout.py");
const PROTOCOL: &str = "xkb-state-v1";
pub const RUNTIME_NAME: &str = "ALOL-Proton-0.1.0-kbd1-x86_64";

pub fn installed() -> core::Result<PathBuf> {
    let directory = core::data_dir().join("runtimes").join(RUNTIME_NAME);
    if !directory.join("proton").is_file() {
        return Err("ALOL-Proton-Keyboard ещё не собран. Инструкция: docs/proton.md, раздел исправления раскладки. Прежний ALOL-Proton сохранён.".into());
    }
    Ok(directory)
}

pub struct LayoutSync {
    child: Child,
    state: File,
    path: PathBuf,
    next_check: Instant,
    exited: bool,
}

impl LayoutSync {
    pub fn start(settings: &core::Settings, command: &mut Command) -> core::Result<Option<Self>> {
        // A normal GE, Wine or the existing experimental Proton remains untouched.
        // Only the new distribution advertises this protocol and records hashes
        // for both rebuilt user32 DLLs in its recipe.
        if settings.runner != "proton" {
            return Ok(None);
        }
        let runtime = if settings.proton == "ALOL-Proton" {
            crate::alol_proton::installed()?
        } else if settings.proton == "ALOL-Proton-Keyboard" {
            installed()?
        } else if Path::new(&settings.proton).is_absolute() {
            PathBuf::from(&settings.proton)
        } else {
            return Ok(None);
        };
        let Ok(marker) = fs::read_to_string(runtime.join("alol-runtime.json")) else {
            if settings.proton == "ALOL-Proton-Keyboard" {
                return Err("Нет манифеста Proton с исправлением раскладки".into());
            }
            return Ok(None);
        };
        let marker = json::parse(&marker)?;
        if marker.get("keyboard_patch").get("protocol").text() != PROTOCOL {
            if settings.proton == "ALOL-Proton-Keyboard" {
                return Err("Proton не поддерживает протокол индикатора раскладки".into());
            }
            return Ok(None);
        }
        for arch in ["i386", "x86_64"] {
            let relative = format!("files/lib/wine/{arch}-windows/user32.dll");
            let expected = marker
                .get("recipe")
                .get("engine_files")
                .get(&relative)
                .text();
            if expected.len() != 64 || mods::hash(&runtime.join(relative), "sha256")? != expected {
                return Err("Proton с исправлением раскладки повреждён; выберите сохранённый ALOL-Proton или пересоберите отдельную версию".into());
            }
        }
        if !settings.input_compatibility
            || !env::var_os("DISPLAY").is_some_and(|display| !display.is_empty())
        {
            eprintln!(
                "Индикатор раскладки: нужен режим совместимости клавиатуры и X11/XWayland; используется обычное определение Wine."
            );
            return Ok(None);
        }
        if !core::available("python3") {
            return Err("Для синхронизации индикатора раскладки нужен Python 3".into());
        }
        let directory = settings.prefix().join("drive_c/alol");
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let name = format!("keyboard-state-{}-{nonce}.bin", std::process::id());
        let path = directory.join(&name);
        let mut state = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|error| format!("Индикатор раскладки: {error}"))?;
        if let Err(error) = state.write_all(&0u32.to_le_bytes()) {
            let _ = fs::remove_file(&path);
            return Err(error.to_string());
        }
        let helpers = core::data_dir().join("helpers");
        let script = helpers.join("xkb-layout-v1.py");
        let child = (|| -> core::Result<Child> {
            fs::create_dir_all(&helpers).map_err(|error| error.to_string())?;
            if fs::read(&script).ok().as_deref() != Some(MONITOR.as_bytes()) {
                let temporary = helpers.join(format!(".xkb-layout-{}.py", std::process::id()));
                fs::write(&temporary, MONITOR).map_err(|error| error.to_string())?;
                fs::rename(&temporary, &script).map_err(|error| error.to_string())?;
            }
            crate::portable::command("python3")
                .arg(&script)
                .arg(&path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .spawn()
                .map_err(|error| error.to_string())
        })();
        let child = match child {
            Ok(child) => child,
            Err(error) => {
                let _ = fs::remove_file(&path);
                return Err(error);
            }
        };
        command.env("ALOL_KEYBOARD_STATE", format!(r"C:\alol\{name}"));
        println!(
            "Индикатор раскладки: синхронизация XKB включена; локаль ru-RU и обработка клавиш сохранены."
        );
        Ok(Some(Self {
            child,
            state,
            path,
            next_check: Instant::now(),
            exited: false,
        }))
    }

    fn clear(&mut self) {
        let _ = self.state.seek(SeekFrom::Start(0));
        let _ = self.state.write_all(&0u32.to_le_bytes());
    }

    pub fn check(&mut self) {
        if !self.exited && Instant::now() >= self.next_check {
            self.next_check = Instant::now() + Duration::from_secs(1);
            if self.child.try_wait().is_ok_and(|status| status.is_some()) {
                self.exited = true;
                self.clear();
                eprintln!(
                    "Синхронизация индикатора остановилась; возвращено обычное определение Wine."
                );
            }
        }
    }
}

impl Drop for LayoutSync {
    fn drop(&mut self) {
        // End the lifetime pipe and reap only our helper, never a Wine/game process.
        self.child.stdin.take();
        if !self.exited {
            let deadline = Instant::now() + Duration::from_millis(500);
            while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if self.child.try_wait().ok().flatten().is_none() {
                let _ = self.child.kill();
            }
            let _ = self.child.wait();
        }
        self.clear();
        let _ = fs::remove_file(&self.path);
    }
}
