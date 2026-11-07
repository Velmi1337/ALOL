//! Bounded log presentation and cleanup of ALOL's own current/rotated logs.
use crate::core::{self, Result};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
    Error,
}

pub fn level(line: &str) -> Level {
    let text = line.to_lowercase();
    if [
        "[ошибка]",
        "ошибка:",
        "ошибка ",
        "err:",
        "error:",
        "error ",
        "fatal",
        "panic",
        "failed",
        "не удалось",
        "не выполнена",
    ]
    .iter()
    .any(|s| text.contains(s))
    {
        Level::Error
    } else if ["warning", "warn:", "предупреж"]
        .iter()
        .any(|s| text.contains(s))
    {
        Level::Warning
    } else {
        Level::Info
    }
}

/// Strip terminal control sequences and collapse overwritten progress lines.
pub fn normalize(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    let mut line_start = 0;
    let mut carriage = false;
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' || (c == '\u{1b}' && chars.next() == Some('\\')) {
                            break;
                        }
                    }
                }
                _ => (),
            }
            continue;
        }
        if ch == '\r' {
            carriage = true;
            continue;
        }
        if ch == '\n' {
            out.push('\n');
            line_start = out.len();
            carriage = false;
        } else if !ch.is_control() || ch == '\t' {
            if carriage {
                out.truncate(line_start);
                carriage = false;
            }
            out.push(ch);
        }
    }
    out
}

pub struct View {
    pub text: String,
    pub shown: usize,
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
}
pub fn view(source: &str, query: &str, severity: i32, last_operation: bool) -> View {
    let text = if last_operation {
        source
            .rfind("──────── ALOL · ")
            .map(|i| &source[i..])
            .unwrap_or(source)
    } else {
        source
    };
    let query = query.trim().to_lowercase();
    let mut result = View {
        text: String::new(),
        shown: 0,
        total: 0,
        errors: 0,
        warnings: 0,
    };
    for line in text.lines() {
        let level = level(line);
        if !line.trim().is_empty() {
            result.total += 1;
        }
        if level == Level::Error {
            result.errors += 1;
        }
        if level == Level::Warning {
            result.warnings += 1;
        }
        let matches = match severity {
            1 => level != Level::Info,
            2 => level == Level::Error,
            _ => true,
        };
        if matches && line.to_lowercase().contains(&query) {
            result.text.push_str(line);
            result.text.push('\n');
            if !line.trim().is_empty() {
                result.shown += 1;
            }
        }
    }
    result
}

pub fn operation_title(action: &str) -> &str {
    match action {
        "play" => "Запуск игры",
        "launch" => "Официальный лаунчер",
        "install" => "Установка лаунчера",
        "prepare" | "runtime-prepare" => "Подготовка окружения",
        "doctor" => "Диагностика",
        "servers-refresh" => "Обновление серверов",
        "mods-refresh" => "Обновление каталога модов",
        "mods-preview" => "Проверка архива",
        "mods-add" | "mods-install" => "Установка мода",
        "mods-enable" => "Включение мода",
        "mods-disable" => "Отключение мода",
        "mods-remove" => "Удаление мода",
        "repair" | "repair-game" | "repair-language" => "Восстановление",
        "libraries" => "Шрифты и библиотеки",
        "self-update" => "Обновление ALOL",
        "reset-game" => "Сброс графики",
        "clean-cache" => "Очистка кэша",
        "winecfg" => "Настройки Wine",
        _ => action,
    }
}
pub fn finish(action: &str, success: bool, stopped: bool) {
    let text = if stopped {
        "[Информация] Операция остановлена"
    } else if success {
        "[Готово] Операция завершена"
    } else {
        "[Ошибка] Операция не выполнена"
    };
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(core::log_file())
    {
        let _ = writeln!(file, "\n{text}: {}", operation_title(action));
    }
}
pub fn failure(action: &str, reason: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(core::log_file())
    {
        let _ = writeln!(file, "[Ошибка] {}: {reason}", operation_title(action));
    }
}

pub fn clear() -> Result<u64> {
    clear_at(
        core::log_file()
            .parent()
            .ok_or("Не найдена папка журналов")?,
    )
}
fn clear_at(directory: &Path) -> Result<u64> {
    let current = directory.join("alol.log");
    let previous = directory.join("alol.previous.log");
    // Validate both before changing either. Never follow a link or open a device.
    for path in [&current, &previous] {
        match fs::symlink_metadata(path) {
            Ok(m) if !m.is_file() || m.file_type().is_symlink() => {
                return Err("Журнал не является обычным файлом; очистка отменена".into());
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => (),
        }
    }
    let mut bytes = 0;
    // Truncate in place: an active O_APPEND writer keeps writing to the same inode.
    const O_NOFOLLOW: i32 = 0x20000;
    const O_NONBLOCK: i32 = 0x800;
    match OpenOptions::new()
        .write(true)
        .custom_flags(O_NOFOLLOW | O_NONBLOCK)
        .open(&current)
    {
        Ok(file) => {
            if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                return Err("Журнал не является обычным файлом".into());
            }
            bytes += file.metadata().map_err(|e| e.to_string())?.len();
            file.set_len(0).map_err(|e| e.to_string())?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    if let Ok(meta) = fs::symlink_metadata(&previous) {
        fs::remove_file(&previous).map_err(|e| e.to_string())?;
        bytes += meta.len();
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_output_progress_and_unicode_are_readable() {
        assert_eq!(
            normalize("\x1b[31mОшибка\x1b[0m\r\n10%\r50%\r100%\r\nГотово\0"),
            "Ошибка\n100%\nГотово"
        );
        assert_eq!(normalize("\x1b]0;title\x07ready\r"), "ready");
    }
    #[test]
    fn search_severity_and_operation_filters_combine() {
        let source = "──────── ALOL · first\nОшибка: старая\n──────── ALOL · last\nready\nWARNING: font\nwine:err: Нет DLL\n";
        let found = view(source, "dll", 2, true);
        assert_eq!(found.shown, 1);
        assert_eq!(found.errors, 1);
        assert_eq!(found.warnings, 1);
        assert_eq!(found.text, "wine:err: Нет DLL\n");
        assert_eq!(view(source, "", 2, false).errors, 2);
        assert!(!view(source, "", 0, true).text.contains("старая"));
        assert_eq!(view(source, "не найдётся", 0, false).shown, 0);
    }
    #[test]
    fn cleanup_keeps_active_writer_and_other_files() {
        let root = std::env::temp_dir().join(format!(
            "alol-log-clean-{}-{}",
            std::process::id(),
            core::stamp()
        ));
        fs::create_dir_all(&root).unwrap();
        let current = root.join("alol.log");
        let mut writer = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&current)
            .unwrap();
        writeln!(writer, "old").unwrap();
        fs::write(root.join("alol.previous.log"), "previous").unwrap();
        fs::write(root.join("keep.log"), "keep").unwrap();
        assert_eq!(clear_at(&root).unwrap(), 12);
        assert_eq!(fs::metadata(&current).unwrap().len(), 0);
        writeln!(writer, "new").unwrap();
        assert_eq!(fs::read_to_string(&current).unwrap(), "new\n");
        assert_eq!(fs::read_to_string(root.join("keep.log")).unwrap(), "keep");
        assert!(!root.join("alol.previous.log").exists());
        fs::remove_file(&current).unwrap();
        std::os::unix::fs::symlink(root.join("keep.log"), &current).unwrap();
        assert!(clear_at(&root).is_err());
        assert_eq!(fs::read_to_string(root.join("keep.log")).unwrap(), "keep");
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(clear_at(&root).unwrap(), 0);
    }
}
