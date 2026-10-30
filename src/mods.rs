//! Official Arizona API adapter and reversible local mod installation.
use crate::{
    archive,
    core::{self, Result, Settings},
    json::{self, Value, object, string},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::Command,
};

const APIS: [&str; 2] = [
    "https://api-samp.arizona-five.com/launcher/mods",
    "https://arz-api-samp-reserve.s-amp.com/launcher/mods",
];
const CDNS: [&str; 2] = [
    "https://pc.az-ins.com/resource/mods",
    "https://arz-pc.react-group.tech/resource/mods",
];
const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct CatalogMod {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub category: String,
    pub meta: Value,
}
#[derive(Clone, Debug)]
pub struct InstalledMod {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub version: String,
    pub source: String,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn safe_relative(text: &str) -> Result<PathBuf> {
    if text.is_empty()
        || text.len() > 4096
        || text.starts_with(['/', '\\'])
        || text.contains(':')
        || text.chars().any(char::is_control)
    {
        return Err(format!("Недопустимый путь мода: {text:?}"));
    }
    let normalized = text.replace('\\', "/");
    let mut out = PathBuf::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => (),
            ".." => return Err("Путь мода выходит из папки игры".into()),
            p => out.push(p),
        }
    }
    if out.as_os_str().is_empty() {
        return Err("Пустой путь мода".into());
    }
    Ok(out)
}
fn valid_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 100
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("Некорректный ID мода".into());
    }
    Ok(())
}
fn fnv(s: &str) -> String {
    let mut n = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        n ^= b as u64;
        n = n.wrapping_mul(0x100000001b3);
    }
    format!("{n:016x}")
}
fn catalog_path() -> PathBuf {
    core::data_dir().join("mods/catalog.json")
}
fn read_json(p: &Path) -> Result<Value> {
    let bytes = fs::read(p).map_err(err)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("JSON больше 16 МБ".into());
    }
    json::parse(std::str::from_utf8(&bytes).map_err(err)?)
}
fn write_json(p: &Path, v: &Value) -> Result<()> {
    atomic_bytes(p, v.dump().as_bytes())
}
pub fn atomic_bytes(p: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(p.parent().ok_or("Нет папки назначения")?).map_err(err)?;
    let tmp = p.with_file_name(format!(
        ".alol-{}-{}.tmp",
        std::process::id(),
        core::stamp()
    ));
    let result = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(err)?;
        f.write_all(bytes).map_err(err)?;
        f.sync_all().map_err(err)?;
        fs::rename(&tmp, p).map_err(err)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
fn copy_atomic(source: &Path, target: &Path) -> Result<()> {
    let bytes = fs::metadata(source).map_err(err)?.len();
    if bytes > MAX_FILE {
        return Err("Файл больше 2 ГБ".into());
    }
    fs::create_dir_all(target.parent().ok_or("Нет папки назначения")?).map_err(err)?;
    let tmp = target.with_file_name(format!(
        ".alol-{}-{}.tmp",
        std::process::id(),
        core::stamp()
    ));
    let result = (|| {
        fs::copy(source, &tmp).map_err(err)?;
        File::open(&tmp).map_err(err)?.sync_all().map_err(err)?;
        fs::rename(&tmp, target).map_err(err)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
pub(crate) fn official_settings(s: &Settings) -> Option<PathBuf> {
    let users = s.prefix().join("drive_c/users");
    let preferred = users.join("steamuser/AppData/Roaming/arizona-launcher/settings.json");
    if preferred.is_file() {
        return Some(preferred);
    }
    for e in fs::read_dir(users).ok()?.flatten() {
        let p = e
            .path()
            .join("AppData/Roaming/arizona-launcher/settings.json");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}
pub fn game_dir(s: &Settings) -> Result<PathBuf> {
    let exe = s
        .find_executable()
        .ok_or("Сначала установите официальный лаунчер Arizona")?;
    let mut game = exe
        .parent()
        .ok_or("Не найдена папка лаунчера")?
        .join("bin/arizona");
    if let Some(path) = official_settings(s)
        && let Ok(settings) = read_json(&path)
    {
        let location = settings
            .get("arizona")
            .get("gamePath")
            .text()
            .replace('\\', "/");
        if location
            .get(..3)
            .is_some_and(|v| v.eq_ignore_ascii_case("C:/"))
        {
            game = s
                .prefix()
                .join("drive_c")
                .join(safe_relative(&location[3..])?);
        } else if location
            .get(..3)
            .is_some_and(|v| v.eq_ignore_ascii_case("Z:/"))
        {
            game = PathBuf::from(format!("/{}", &location[3..]));
        }
    }
    if !game.is_dir() || game.is_symlink() {
        return Err("Сначала скачайте игру через официальный лаунчер".into());
    }
    game.canonicalize().map_err(err)
}
fn store(s: &Settings) -> Result<PathBuf> {
    Ok(s.prefix()
        .join(".alol-mods")
        .join(fnv(&game_dir(s)?.to_string_lossy())))
}
fn mod_dir(s: &Settings, id: &str) -> Result<PathBuf> {
    valid_id(id)?;
    Ok(store(s)?.join(id))
}
fn target(root: &Path, relative: &Path) -> Result<PathBuf> {
    let mut out = root.to_path_buf();
    for part in relative.components() {
        let part = part.as_os_str();
        let name = part.to_string_lossy();
        if out.is_dir() {
            let matches: Vec<_> = fs::read_dir(&out)
                .map_err(err)?
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(&name))
                .map(|e| e.path())
                .collect();
            if matches.len() > 1 {
                return Err(format!(
                    "Конфликт регистра имён: {}",
                    out.join(part).display()
                ));
            }
            if let Some(p) = matches.first() {
                out = p.clone();
            } else {
                out.push(part);
            }
        } else {
            if out.exists() {
                return Err(format!("Вместо папки найден файл: {}", out.display()));
            }
            out.push(part);
        }
        if out.is_symlink() {
            return Err(format!(
                "Назначение мода — символическая ссылка: {}",
                out.display()
            ));
        }
    }
    Ok(out)
}
fn files(root: &Path) -> Result<Vec<PathBuf>> {
    fn visit(base: &Path, p: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
        if depth > 32 {
            return Err("Слишком глубокая структура мода".into());
        }
        for e in fs::read_dir(p).map_err(err)? {
            let e = e.map_err(err)?;
            let kind = e.file_type().map_err(err)?;
            if kind.is_symlink() {
                return Err("Символические ссылки в модах не поддерживаются".into());
            }
            if kind.is_dir() {
                visit(base, &e.path(), out, depth + 1)?;
            } else if kind.is_file() {
                out.push(e.path().strip_prefix(base).map_err(err)?.to_path_buf());
                if out.len() > 20000 {
                    return Err("Больше 20000 файлов в моде".into());
                }
            } else {
                return Err("Неподдерживаемый тип файла".into());
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(root, root, &mut out, 0)?;
    out.sort();
    let mut seen = BTreeSet::new();
    for p in &out {
        safe_relative(&p.to_string_lossy())?;
        if !seen.insert(p.to_string_lossy().to_lowercase()) {
            return Err("Мод содержит имена, отличающиеся только регистром".into());
        }
    }
    Ok(out)
}
fn hash(p: &Path, algorithm: &str) -> Result<String> {
    let (tool, len) = match algorithm {
        "md5" => ("md5sum", 32),
        "sha256" => ("sha256sum", 64),
        "sha512" => ("sha512sum", 128),
        _ => return Err("Неизвестный алгоритм хеша".into()),
    };
    let o = Command::new(tool).arg("--").arg(p).output().map_err(err)?;
    let text = String::from_utf8_lossy(&o.stdout);
    let h = text.get(..len).unwrap_or("");
    if !o.status.success() || !h.bytes().all(|b| b.is_ascii_hexdigit()) || h.len() != len {
        return Err(format!("Не вычислить хеш {}", p.display()));
    }
    Ok(h.to_ascii_lowercase())
}
pub(crate) fn fetch(url: &str, out: &Path, limit: u64) -> Result<()> {
    if !url.starts_with("https://") {
        return Err("Адрес загрузки должен использовать HTTPS".into());
    }
    fs::create_dir_all(out.parent().ok_or("Нет папки загрузки")?).map_err(err)?;
    let status = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--connect-timeout",
            "8",
            "--max-time",
            "180",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-filesize",
            &limit.to_string(),
            "--output",
        ])
        .arg(out)
        .arg(url)
        .status()
        .map_err(|e| format!("Не запустить curl: {e}"))?;
    if !status.success() {
        let _ = fs::remove_file(out);
        return Err(format!("Не удалось скачать {url}: {status}"));
    }
    if fs::metadata(out).map_err(err)?.len() > limit {
        return Err("Загрузка превышает лимит размера".into());
    }
    Ok(())
}
fn fetch_json(path: &str, out: &Path) -> Result<Value> {
    let mut last = String::new();
    for api in APIS {
        match fetch(&format!("{api}{path}"), out, 16 * 1024 * 1024).and_then(|_| read_json(out)) {
            Ok(v) => return Ok(v),
            Err(e) => {
                println!("{e}");
                last = e;
            }
        }
    }
    Err(last)
}
pub fn catalog() -> Result<Vec<CatalogMod>> {
    if !catalog_path().exists() {
        return Ok(Vec::new());
    }
    parse_catalog(&read_json(&catalog_path())?)
}
fn parse_catalog(v: &Value) -> Result<Vec<CatalogMod>> {
    let mut mods = BTreeMap::new();
    let arrays: Vec<&[Value]> = match v {
        Value::Array(a) => vec![a],
        Value::Object(_) => vec![v.get("topOfMonth").array(), v.get("mods").array()],
        _ => return Err("Официальный каталог имеет неизвестный формат".into()),
    };
    for entry in arrays.into_iter().flatten() {
        if matches!(entry.get("visible"), Value::Bool(false)) {
            continue;
        }
        let id = entry.get("id").text();
        valid_id(&id)?;
        let mut name = entry.get("name").text();
        if name.is_empty() {
            name = entry.get("title").text();
        }
        if name.is_empty() {
            name = format!("Мод {id}");
        }
        mods.entry(id.clone()).or_insert(CatalogMod {
            id,
            name,
            description: entry.get("description").text(),
            version: entry.get("version").text(),
            category: entry.get("category").text(),
            meta: entry.clone(),
        });
    }
    let mut out: Vec<_> = mods.into_values().collect();
    out.sort_by_key(|m| m.name.to_lowercase());
    Ok(out)
}
pub fn installed(s: &Settings) -> Result<Vec<InstalledMod>> {
    let base = store(s)?;
    if !base.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for e in fs::read_dir(base).map_err(err)?.flatten() {
        let path = e.path();
        if path.is_symlink() || !path.join("state.json").is_file() {
            continue;
        }
        let v = read_json(&path.join("state.json"))?;
        out.push(InstalledMod {
            id: v.get("id").text(),
            name: v.get("name").text(),
            active: v.get("active").boolean(),
            version: v.get("version").text(),
            source: v.get("source").text(),
        });
    }
    out.sort_by_key(|m| m.name.to_lowercase());
    Ok(out)
}
fn state(s: &Settings, id: &str) -> Result<Value> {
    let d = mod_dir(s, id)?;
    if d.is_symlink() {
        return Err("Папка мода является ссылкой".into());
    }
    read_json(&d.join("state.json"))
}
fn write_state(s: &Settings, v: &Value) -> Result<()> {
    write_json(&mod_dir(s, &v.get("id").text())?.join("state.json"), v)
}
fn sync_official(s: &Settings, v: &Value, enabled: bool) -> Result<()> {
    let Some(path) = official_settings(s) else {
        return Ok(());
    };
    let mut settings = read_json(&path)?;
    let mut all = settings.get("installedMods").clone();
    let mut arizona = all.get("arizona").clone();
    let id = v.get("native_id").text();
    if enabled {
        let mut meta = v.get("meta").clone();
        if matches!(meta.get("id"), Value::Null) {
            meta.put("id", string(&id));
        }
        meta.put("name", v.get("name").clone());
        if matches!(meta.get("version"), Value::Null) {
            meta.put("version", v.get("version").clone());
        }
        meta.put(
            "installedFilesRelativePaths",
            Value::Array(
                v.get("files")
                    .array()
                    .iter()
                    .map(|f| f.get("path").clone())
                    .collect(),
            ),
        );
        meta.put(
            "replacedFilesPaths",
            Value::Array(
                v.get("files")
                    .array()
                    .iter()
                    .filter(|f| !matches!(f.get("original"), Value::Null))
                    .map(|f| f.get("path").clone())
                    .collect(),
            ),
        );
        arizona.put(&id, meta);
    } else {
        arizona.remove(&id);
    }
    all.put("arizona", arizona);
    settings.put("installedMods", all);
    write_json(&path, &settings)
}
fn enable(s: &Settings, id: &str) -> Result<()> {
    let root = game_dir(s)?;
    let d = mod_dir(s, id)?;
    let mut v = state(s, id)?;
    if v.get("active").boolean() {
        return Err("Мод уже включён".into());
    }
    let source = d.join("payload");
    let payload = files(&source)?;
    if payload.is_empty() {
        return Err("В моде нет файлов".into());
    }
    let mut owned = BTreeSet::new();
    for m in installed(s)? {
        if m.active && m.id != id {
            for f in state(s, &m.id)?.get("files").array() {
                owned.insert(f.get("path").text().to_lowercase());
            }
        }
    }
    let mut records = Vec::new();
    let mut total = 0;
    for p in payload {
        let dest = target(&root, &p)?;
        if dest.is_dir() {
            return Err(format!("Вместо файла существует папка: {}", dest.display()));
        }
        let rel = dest
            .strip_prefix(&root)
            .map_err(err)?
            .to_string_lossy()
            .replace('\\', "/");
        if owned.contains(&rel.to_lowercase()) {
            return Err(format!(
                "Другой включённый мод уже меняет {rel}. Сначала отключите его."
            ));
        }
        total += fs::metadata(source.join(&p)).map_err(err)?.len();
        if total > 8 * 1024 * 1024 * 1024 {
            return Err("Мод больше 8 ГБ".into());
        }
        let original = if dest.is_file() {
            let backup = d.join("originals").join(&rel);
            fs::create_dir_all(backup.parent().unwrap()).map_err(err)?;
            fs::copy(&dest, &backup).map_err(err)?;
            string(hash(&dest, "sha256")?)
        } else {
            Value::Null
        };
        records.push(object([
            ("path", string(rel)),
            ("payload", string(p.to_string_lossy())),
            ("sha256", string(hash(&source.join(p), "sha256")?)),
            ("original", original),
        ]));
    }
    v.put("files", Value::Array(records));
    v.put("status", string("installing"));
    write_state(s, &v)?;
    let result = (|| {
        for f in v.get("files").array() {
            let p = safe_relative(&f.get("path").text())?;
            let src = source.join(safe_relative(&f.get("payload").text())?);
            copy_atomic(&src, &target(&root, &p)?)?;
        }
        sync_official(s, &v, true)?;
        v.put("active", Value::Bool(true));
        v.put("status", string("ready"));
        write_state(s, &v)
    })();
    if let Err(e) = result {
        let rollback = restore(s, &v);
        return Err(format!(
            "Установка не завершена: {e}. Откат: {}",
            rollback.err().unwrap_or_else(|| "выполнен".into())
        ));
    }
    println!(
        "Включён мод: {} · файлов: {}",
        v.get("name").text(),
        v.get("files").array().len()
    );
    Ok(())
}
fn restore(s: &Settings, v: &Value) -> Result<()> {
    let root = game_dir(s)?;
    let d = mod_dir(s, &v.get("id").text())?;
    // Check every file before changing any; preserve edits made after installation.
    for f in v.get("files").array() {
        let dest = target(&root, &safe_relative(&f.get("path").text())?)?;
        if dest.exists() {
            let h = hash(&dest, "sha256")?;
            if h != f.get("sha256").text() && h != f.get("original").text() {
                return Err(format!(
                    "Файл изменён после установки мода: {}. Он сохранён, откат остановлен.",
                    dest.display()
                ));
            }
        }
        if !matches!(f.get("original"), Value::Null) {
            let backup = d
                .join("originals")
                .join(safe_relative(&f.get("path").text())?);
            if hash(&backup, "sha256")? != f.get("original").text() {
                return Err("Повреждена резервная копия мода".into());
            }
        }
    }
    for f in v.get("files").array() {
        let rel = safe_relative(&f.get("path").text())?;
        let dest = target(&root, &rel)?;
        if matches!(f.get("original"), Value::Null) {
            if dest.is_file() {
                fs::remove_file(&dest).map_err(err)?;
            }
        } else {
            copy_atomic(&d.join("originals").join(rel), &dest)?;
        }
    }
    sync_official(s, v, false)?;
    let mut v = v.clone();
    v.put("active", Value::Bool(false));
    v.put("status", string("ready"));
    write_state(s, &v)
}
fn add_payload(
    s: &Settings,
    id: &str,
    name: &str,
    source_name: &str,
    version: &str,
    meta: Value,
    payload: &Path,
) -> Result<()> {
    let d = mod_dir(s, id)?;
    if d.join("state.json").exists() {
        return Err("Мод уже добавлен. Используйте включение или удалите старую версию.".into());
    }
    if d.is_symlink() {
        return Err("Папка мода является ссылкой".into());
    }
    if d.exists() {
        fs::remove_dir_all(&d).map_err(err)?;
    }
    for p in files(payload)? {
        let dest = d.join("payload").join(&p);
        fs::create_dir_all(dest.parent().unwrap()).map_err(err)?;
        fs::copy(payload.join(p), dest).map_err(err)?;
    }
    let native = if let Some(v) = id.strip_prefix("official-") {
        v.to_string()
    } else {
        id.to_string()
    };
    let v = object([
        ("id", string(id)),
        ("native_id", string(native)),
        ("name", string(name)),
        ("version", string(version)),
        ("source", string(source_name)),
        ("meta", meta),
        ("active", Value::Bool(false)),
        ("status", string("ready")),
        ("files", Value::Array(Vec::new())),
    ]);
    write_state(s, &v)?;
    enable(s, id)
}
struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn staging() -> Result<Staging> {
    let base = core::data_dir().join("mods/staging");
    fs::create_dir_all(&base).map_err(err)?;
    let p = base.join(format!("{}-{}", core::stamp(), std::process::id()));
    fs::create_dir(&p).map_err(err)?;
    Ok(Staging(p))
}
fn url_path(p: &Path) -> String {
    p.to_string_lossy()
        .replace('\\', "/")
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn manifest(
    v: &Value,
    parent: &Path,
    out: &mut Vec<(PathBuf, u64, String)>,
    depth: usize,
) -> Result<()> {
    if depth > 32 {
        return Err("Слишком глубокий манифест".into());
    }
    if let Value::Array(a) = v {
        for x in a {
            manifest(x, parent, out, depth + 1)?;
        }
        return Ok(());
    }
    let kind = v.get("type").text();
    let mut here = parent.to_path_buf();
    if !kind.is_empty() {
        here.push(safe_relative(&v.get("name").text())?);
        if kind != "dir" {
            if kind != "file" {
                return Err(format!("Неизвестный тип в манифесте: {kind}"));
            }
            let size = v.get("size").text().parse::<u64>().map_err(err)?;
            if size > MAX_FILE {
                return Err("Файл официального мода больше 2 ГБ".into());
            }
            let h = v.get("hash").text();
            if ![32, 64, 128].contains(&h.len()) || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("В официальном манифесте нет корректного хеша файла".into());
            }
            out.push((here.clone(), size, h.to_lowercase()));
        }
    }
    if out.len() > 20000 {
        return Err("В манифесте слишком много файлов".into());
    }
    for x in v.get("data").array() {
        manifest(x, &here, out, depth + 1)?;
    }
    Ok(())
}
fn official_install(s: &Settings, id: &str) -> Result<()> {
    valid_id(id)?;
    if !mod_dir(s, &format!("official-{id}"))?
        .join("state.json")
        .exists()
        && let Some(p) = official_settings(s)
        && !matches!(
            read_json(&p)?.get("installedMods").get("arizona").get(id),
            Value::Null
        )
    {
        return Err("Этот мод установлен через Arizona. Удалите его там перед установкой через ALOL, чтобы сохранить корректный откат файлов.".into());
    }
    let entry = catalog()?
        .into_iter()
        .find(|m| m.id == id)
        .ok_or("Сначала обновите каталог и выберите официальный мод")?;
    let stage = staging()?;
    let v = fetch_json(&format!("/arizona/{id}"), &stage.0.join("manifest.json"))?;
    let mut list = Vec::new();
    manifest(&v, Path::new(""), &mut list, 0)?;
    if list.is_empty() {
        return Err("Официальный манифест пуст".into());
    }
    let mut sum = 0;
    let mut seen = BTreeSet::new();
    let payload = stage.0.join("payload");
    for (p, size, expected) in list {
        if !seen.insert(p.to_string_lossy().to_lowercase()) {
            return Err("Повторный путь в манифесте".into());
        }
        sum += size;
        if sum > 8 * 1024 * 1024 * 1024 {
            return Err("Мод больше 8 ГБ".into());
        }
        let out = payload.join(&p);
        let mut ok = false;
        for cdn in CDNS {
            let url = format!("{cdn}/{id}/{}", url_path(&p));
            println!("Загрузка: {}", p.display());
            if fetch(&url, &out, MAX_FILE).is_ok()
                && fs::metadata(&out).is_ok_and(|m| m.len() == size)
            {
                let algorithm = match expected.len() {
                    32 => "md5",
                    64 => "sha256",
                    _ => "sha512",
                };
                if hash(&out, algorithm)? == expected {
                    ok = true;
                    break;
                }
            }
        }
        if !ok {
            return Err(format!(
                "Не скачать или не проверить {}. Файлы игры не изменены.",
                p.display()
            ));
        }
    }
    add_payload(
        s,
        &format!("official-{id}"),
        &entry.name,
        "official",
        &entry.version,
        entry.meta,
        &payload,
    )
}
fn local_install(s: &Settings, path: &Path, mode: &str) -> Result<()> {
    if !path.is_absolute() || !path.is_file() || path.is_symlink() {
        return Err("Выберите обычный файл мода по абсолютному пути".into());
    }
    if !["auto", "root", "modloader", "cleo", "moonloader"].contains(&mode) {
        return Err("Неизвестная папка установки мода".into());
    }
    let stage = staging()?;
    let raw = stage.0.join("raw");
    fs::create_dir(&raw).map_err(err)?;
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if ["zip", "rar", "7z", "tar", "gz", "xz", "bz2"].contains(&ext.as_str()) {
        archive::extract(path, &raw)?;
    } else if [
        "asi", "cs", "cleo", "lua", "luac", "dll", "ini", "cfg", "dff", "txd", "col", "ifp",
    ]
    .contains(&ext.as_str())
    {
        if fs::metadata(path).map_err(err)?.len() > MAX_FILE {
            return Err("Файл больше 2 ГБ".into());
        }
        let name = path.file_name().ok_or("Нет имени файла")?;
        safe_relative(&name.to_string_lossy())?;
        fs::copy(path, raw.join(name)).map_err(err)?;
    } else {
        return Err(
            "Поддерживаются ZIP, RAR, 7z, TAR и файлы ASI, CLEO, Lua, DLL, DFF, TXD, IFP".into(),
        );
    }
    let folders = [
        "modloader",
        "cleo",
        "moonloader",
        "models",
        "data",
        "audio",
        "scripts",
        "fontcustom",
        "samp",
        "cef",
        "libs",
    ];
    let mut base = raw;
    for _ in 0..4 {
        let entries: Vec<_> = fs::read_dir(&base)
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        if entries.len() == 1
            && entries[0].file_type().map_err(err)?.is_dir()
            && !folders.iter().any(|n| {
                entries[0]
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(n)
            })
        {
            base = entries[0].path();
        } else {
            break;
        }
    }
    let paths = files(&base)?;
    if paths.is_empty() {
        return Err("В архиве нет файлов мода".into());
    }
    let name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let slug = format!("alol-{}", fnv(&name));
    let structured = paths.iter().any(|p| {
        p.components().next().is_some_and(|c| {
            folders
                .iter()
                .any(|n| c.as_os_str().to_string_lossy().eq_ignore_ascii_case(n))
        })
    });
    let chosen = if mode != "auto" {
        mode
    } else if structured {
        "root"
    } else if paths.iter().any(|p| {
        p.extension()
            .is_some_and(|e| ["lua", "luac"].iter().any(|n| e.eq_ignore_ascii_case(n)))
    }) {
        "moonloader"
    } else if paths.iter().any(|p| {
        p.extension()
            .is_some_and(|e| ["cs", "cleo"].iter().any(|n| e.eq_ignore_ascii_case(n)))
    }) {
        "cleo"
    } else if paths.iter().all(|p| {
        p.extension().is_some_and(|e| {
            ["dff", "txd", "col", "ifp", "txt", "png", "jpg"]
                .iter()
                .any(|n| e.eq_ignore_ascii_case(n))
        })
    }) {
        "modloader"
    } else {
        "root"
    };
    let prefix = match chosen {
        "root" => PathBuf::new(),
        "modloader" => PathBuf::from("modloader").join(slug),
        other => PathBuf::from(other),
    };
    let payload = stage.0.join("payload");
    for p in paths {
        let dest = payload.join(&prefix).join(&p);
        fs::create_dir_all(dest.parent().unwrap()).map_err(err)?;
        fs::copy(base.join(p), dest).map_err(err)?;
    }
    let id = format!("local-{}-{}", core::stamp(), std::process::id());
    add_payload(s, &id, &name, "local", "", object([]), &payload)
}
pub fn execute(s: &Settings, action: &str, args: &[String]) -> Result<()> {
    // Finish or undo an interrupted transaction before another file operation.
    if action != "mods-refresh" {
        for m in installed(s)? {
            let v = state(s, &m.id)?;
            if v.get("status").text() == "installing" {
                restore(s, &v)?;
            }
        }
    }
    match action {
        "mods-refresh" => {
            let stage = staging()?;
            let v = fetch_json("/arizona", &stage.0.join("catalog.json"))?;
            let entries = parse_catalog(&v)?;
            write_json(&catalog_path(), &v)?;
            println!("Официальный каталог обновлён: {} модов", entries.len());
            Ok(())
        }
        "mods-install" => official_install(s, args.first().ok_or("Выберите мод")?),
        "mods-add" => local_install(
            s,
            Path::new(args.first().ok_or("Выберите файл мода")?),
            args.get(1).map(String::as_str).unwrap_or("auto"),
        ),
        "mods-enable" => enable(s, args.first().ok_or("Выберите мод")?),
        "mods-disable" | "mods-remove" => {
            let id = args.first().ok_or("Выберите мод")?;
            let v = state(s, id)?;
            if v.get("active").boolean() {
                restore(s, &v)?;
            }
            if action == "mods-remove" {
                fs::remove_dir_all(mod_dir(s, id)?).map_err(err)?;
            }
            println!(
                "Мод {}: {}",
                v.get("name").text(),
                if action == "mods-remove" {
                    "удалён"
                } else {
                    "отключён"
                }
            );
            Ok(())
        }
        _ => Err("Неизвестная операция модов".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_escape_paths() {
        for p in [
            "../x",
            "C:\\x",
            "/tmp/x",
            "a/../../x",
            "x:stream",
            "a\n.txt",
        ] {
            assert!(safe_relative(p).is_err());
        }
        assert_eq!(
            safe_relative("./cleo\\x.cs").unwrap(),
            PathBuf::from("cleo/x.cs")
        );
    }
    #[test]
    fn official_tree_manifest() {
        let v=json::parse(r#"{"data":[{"type":"dir","name":"cleo","data":[{"type":"file","name":"мод.cs","size":3,"hash":"900150983cd24fb0d6963f7d28e17f72"}]}]}"#).unwrap();
        let mut out = Vec::new();
        manifest(&v, Path::new(""), &mut out, 0).unwrap();
        assert_eq!(out[0].0, PathBuf::from("cleo/мод.cs"));
    }
    #[test]
    fn official_catalog_deduplicates_and_hides() {
        let v=json::parse(r#"{"topOfMonth":[{"id":1,"name":"Мод"}],"mods":[{"id":1,"name":"Мод"},{"id":2,"visible":false}]}"#).unwrap();
        assert_eq!(parse_catalog(&v).unwrap().len(), 1);
    }
}
