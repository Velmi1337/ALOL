//! Extract only regular files/directories with libarchive; never follow archive links.
use crate::{core::Result, mods::safe_relative};
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
#[link(name = "archive")]
unsafe extern "C" {
    fn archive_read_new() -> *mut c_void;
    fn archive_read_support_filter_all(a: *mut c_void) -> c_int;
    fn archive_read_support_format_all(a: *mut c_void) -> c_int;
    fn archive_read_open_filename(a: *mut c_void, p: *const c_char, block: usize) -> c_int;
    fn archive_read_next_header(a: *mut c_void, entry: *mut *mut c_void) -> c_int;
    fn archive_entry_pathname_utf8(e: *mut c_void) -> *const c_char;
    fn archive_entry_pathname(e: *mut c_void) -> *const c_char;
    fn archive_entry_filetype(e: *mut c_void) -> u32;
    fn archive_entry_symlink(e: *mut c_void) -> *const c_char;
    fn archive_entry_hardlink(e: *mut c_void) -> *const c_char;
    fn archive_entry_size(e: *mut c_void) -> i64;
    fn archive_read_data(a: *mut c_void, b: *mut c_void, len: usize) -> isize;
    fn archive_error_string(a: *mut c_void) -> *const c_char;
    fn archive_read_free(a: *mut c_void) -> c_int;
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
}
struct Reader(*mut c_void);
impl Drop for Reader {
    fn drop(&mut self) {
        unsafe {
            archive_read_free(self.0);
        }
    }
}
fn error(a: *mut c_void) -> String {
    unsafe {
        let p = archive_error_string(a);
        if p.is_null() {
            "Ошибка чтения архива".into()
        } else {
            CStr::from_ptr(p).to_string_lossy().into()
        }
    }
}
pub fn extract(source: &Path, destination: &Path) -> Result<()> {
    let name = CString::new(source.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
    // SAFETY: Reader owns the archive, entries/data are used only before the next header.
    unsafe {
        // Rust workers do not initialize libc's locale like GTK does. Without
        // this, libarchive rejects Cyrillic filenames even under LANG=*.UTF-8.
        if setlocale(0, c"C.UTF-8".as_ptr()).is_null() {
            return Err("Не включить UTF-8 для имён файлов архива".into());
        }
        let r = Reader(archive_read_new());
        if r.0.is_null() {
            return Err("Не создать libarchive reader".into());
        }
        archive_read_support_filter_all(r.0);
        archive_read_support_format_all(r.0);
        if archive_read_open_filename(r.0, name.as_ptr(), 65536) < 0 {
            return Err(error(r.0));
        }
        let mut count = 0;
        let mut total = 0u64;
        loop {
            let mut entry = std::ptr::null_mut();
            let status = archive_read_next_header(r.0, &mut entry);
            if status == 1 {
                break;
            }
            if status < 0 {
                return Err(error(r.0));
            }
            if entry.is_null() {
                return Err("Пустой заголовок архива".into());
            }
            count += 1;
            if count > 20000 {
                return Err("В архиве больше 20000 файлов".into());
            }
            let mut p = archive_entry_pathname_utf8(entry);
            if p.is_null() {
                p = archive_entry_pathname(entry);
            }
            if p.is_null() {
                return Err("В архиве нет имени файла".into());
            }
            let text = CStr::from_ptr(p).to_str().map_err(
                |_| "В архиве имя файла не в UTF-8. Перепакуйте архив в ZIP с Unicode именами.",
            )?;
            if text == "." || text == "./" {
                continue;
            }
            if text.to_ascii_lowercase().ends_with(":zone.identifier") {
                continue;
            }
            let relative = safe_relative(text)?;
            if !archive_entry_symlink(entry).is_null() || !archive_entry_hardlink(entry).is_null() {
                return Err(format!("Ссылки в архиве не поддерживаются: {text}"));
            }
            let path = destination.join(relative);
            match archive_entry_filetype(entry) {
                0o040000 => fs::create_dir_all(path).map_err(|e| e.to_string())?,
                0o100000 => {
                    let size = archive_entry_size(entry);
                    if !(0..=2_147_483_648).contains(&size) {
                        return Err("Файл мода больше 2 ГБ".into());
                    }
                    fs::create_dir_all(path.parent().ok_or("Нет родительского каталога")?)
                        .map_err(|e| e.to_string())?;
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)
                        .map_err(|e| format!("{}: {e}", path.display()))?;
                    let mut buf = [0u8; 65536];
                    let mut file_bytes = 0u64;
                    loop {
                        let n = archive_read_data(r.0, buf.as_mut_ptr().cast(), buf.len());
                        if n < 0 {
                            return Err(error(r.0));
                        }
                        if n == 0 {
                            break;
                        }
                        total += n as u64;
                        file_bytes += n as u64;
                        if total > 8 * 1024 * 1024 * 1024 || file_bytes > 2_147_483_648 {
                            return Err("Распакованный мод превышает лимит размера".into());
                        }
                        file.write_all(&buf[..n as usize])
                            .map_err(|e| e.to_string())?;
                    }
                    if file_bytes != size as u64 {
                        return Err(format!("Неполный файл в архиве: {text}"));
                    }
                }
                _ => return Err(format!("Неподдерживаемый тип файла в архиве: {text}")),
            }
        }
    }
    Ok(())
}
