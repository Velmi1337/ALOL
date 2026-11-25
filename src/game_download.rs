//! Verified, incremental downloads from the two official game CDNs.
use crate::{
    core::{self, Result, Settings},
    game_options::GameOptions,
    json::{self, Value},
    mods,
    project::Project,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, FileTimes},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, UNIX_EPOCH},
};

const PENDING: &str = ".alol-game-update";
const INSTALLED_RELEASE: &str = ".alol-game-release";
const MANIFEST_LIMIT: u64 = 8 * 1024 * 1024;
const MAX_FILE: u64 = 16 * 1024 * 1024 * 1024;
pub const EXECUTABLES: &[&str] = &["enb.exe", "gta-sa.exe", "gtasa.exe", "gta_sa.exe"];

pub fn unfinished(root: &Path) -> bool {
    root.join(PENDING).symlink_metadata().is_ok()
}

struct Entry {
    remote: PathBuf,
    local: PathBuf,
    size: u64,
    hash: String,
    modified: Option<u64>,
    resource: bool,
}

fn component(value: &Value) -> Result<String> {
    let name = value.text();
    let path = mods::safe_relative(&name)?;
    let stem = name.split('.').next().unwrap_or("").to_ascii_lowercase();
    let device = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ["com", "lpt"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|suffix| suffix.len() == 1 && suffix.as_bytes()[0].is_ascii_digit())
        });
    if path.as_os_str() != name.as_str()
        || name.contains(['/', '\\', '<', '>', '"', '|', '?', '*'])
        || name.ends_with(['.', ' '])
        || name.to_ascii_lowercase().starts_with(".alol")
        || device
    {
        return Err(format!("Недопустимое имя в списке файлов игры: {name:?}"));
    }
    Ok(name)
}

fn parse_manifest(text: &str, variant: &str) -> Result<Vec<Entry>> {
    let root = json::parse(text)?;
    let mut entries = Vec::new();
    let mut paths = BTreeMap::new();
    let mut queue = vec![(root.get("data").array(), PathBuf::new(), PathBuf::new())];
    let mut count = 0;
    while let Some((nodes, remote_parent, local_parent)) = queue.pop() {
        for node in nodes {
            count += 1;
            if count > 30_000 {
                return Err("Список файлов игры слишком большой".into());
            }
            let tag = node.get("variant_tag").text();
            if !tag.is_empty() && tag != variant {
                if !["default", "future"].contains(&tag.as_str()) {
                    return Err("Список файлов содержит неизвестный вариант игры".into());
                }
                continue;
            }
            let kind = node.get("type").text();
            if kind == "delete" {
                continue;
            }
            let name = component(node.get("name"))?;
            let local_name = if matches!(node.get("install_as"), Value::Null) {
                name.clone()
            } else {
                component(node.get("install_as"))?
            };
            let remote = remote_parent.join(name);
            let local = local_parent.join(local_name);
            let key = local.to_string_lossy().to_lowercase();
            if paths.insert(key, kind == "dir").is_some() {
                return Err("В списке файлов игры повторяется один и тот же путь".into());
            }
            if kind == "dir" {
                queue.push((node.get("data").array(), remote, local));
                continue;
            }
            if !matches!(kind.as_str(), "file" | "check" | "res") {
                return Err(format!("Неизвестный тип игрового файла: {kind}"));
            }
            let size = node
                .get("size")
                .text()
                .parse::<u64>()
                .map_err(|_| "В списке файлов игры указан неверный размер")?;
            if size > MAX_FILE {
                return Err("Размер игрового файла превышает 16 ГиБ".into());
            }
            let hash = node.get("hash").text().to_ascii_lowercase();
            if hash.len() != 32 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err("В списке файлов игры нет контрольной суммы MD5".into());
            }
            entries.push(Entry {
                remote,
                local,
                size,
                hash,
                modified: node.get("date_change").text().parse().ok(),
                resource: kind == "res",
            });
        }
    }
    if !EXECUTABLES
        .iter()
        .any(|name| paths.get(*name) == Some(&false))
    {
        return Err("В списке файлов нет исполняемого файла игры".into());
    }
    Ok(entries)
}

fn manifest(s: &Settings, variant: &str) -> Result<(Vec<Entry>, [&'static str; 2])> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let mut cdns = s.project.game_cdns();
    let cache = core::data_dir().join("game-downloads");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let path = cache.join(format!(
        "manifest-{}-{}.json",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut last = String::new();
    for index in 0..cdns.len() {
        core::check_cancelled()?;
        let result = mods::fetch_with_timeout(
            &format!("{}/game.json", cdns[index]),
            &path,
            MANIFEST_LIMIT,
            20,
        )
        .and_then(|()| fs::read_to_string(&path).map_err(|e| e.to_string()))
        .and_then(|data| parse_manifest(&data, variant));
        let _ = fs::remove_file(&path);
        match result {
            Ok(entries) => {
                cdns.swap(0, index);
                return Ok((entries, cdns));
            }
            Err(error) => last = error,
        }
    }
    Err(format!(
        "Не удалось получить список файлов с серверов Arizona. {last}"
    ))
}

fn variant(new_graphics: bool) -> &'static str {
    if new_graphics { "future" } else { "default" }
}

fn release_index(entries: &[Entry]) -> String {
    let mut index: Vec<_> = entries
        .iter()
        .filter(|entry| !entry.resource)
        .map(|entry| format!("{}\t{}\t{}", entry.local.display(), entry.size, entry.hash))
        .collect();
    index.sort_unstable();
    index.join("\n")
}

/// Check for a new release without opening the large game files for hashing.
pub(crate) fn update_available(s: &Settings, root: &Path, new_graphics: bool) -> Result<bool> {
    let (entries, _) = manifest(s, variant(new_graphics))?;
    let release = mods::target(root, Path::new(INSTALLED_RELEASE))?;
    if release
        .metadata()
        .is_ok_and(|m| m.is_file() && m.len() <= MANIFEST_LIMIT)
    {
        let saved = fs::read_to_string(release).map_err(|error| error.to_string())?;
        return Ok(saved != release_index(&entries));
    }

    // Existing installs have no ALOL index. Use the size and timestamp that
    // the official updater sets. A manual update verifies all checksums.
    let protected = mods::protected_game_files(s)?;
    for entry in entries {
        if entry.resource || protected.contains(&entry.local.to_string_lossy().to_lowercase()) {
            continue;
        }
        let metadata = match root.join(&entry.local).symlink_metadata() {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                mods::target(root, &entry.local)?.symlink_metadata()
            }
            found => found,
        };
        let matches = metadata.is_ok_and(|metadata| {
            metadata.is_file()
                && metadata.len() == entry.size
                && entry.modified.is_none_or(|date| {
                    metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                        .is_some_and(|time| time.as_secs() >= date)
                })
        });
        if !matches {
            return Ok(true);
        }
    }
    Ok(false)
}

struct Download {
    entry: Entry,
    target: PathBuf,
}

fn inspect(
    root: &Path,
    entries: Vec<Entry>,
    protected: &BTreeSet<String>,
) -> Result<Vec<Download>> {
    let total = entries.len();
    let mut downloads = Vec::new();
    let mut tick = Instant::now() - Duration::from_secs(1);
    for (index, entry) in entries.into_iter().enumerate() {
        core::check_cancelled()?;
        if tick.elapsed() >= Duration::from_millis(500) {
            core::progress(&format!("Проверка файлов · {} из {total}", index + 1));
            tick = Instant::now();
        }
        let target = mods::target(root, &entry.local)?;
        let managed_by_mod = protected.contains(&entry.local.to_string_lossy().to_lowercase());
        match target.symlink_metadata() {
            Ok(meta) if meta.is_file() => {
                if managed_by_mod
                    || entry.resource
                    || (meta.len() == entry.size && mods::hash(&target, "md5")? == entry.hash)
                {
                    continue;
                }
            }
            Ok(_) => return Err(format!("Нужен обычный файл: {}", entry.local.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if managed_by_mod {
                    return Err(format!(
                        "Файл мода отсутствует: {}. Переустановите или отключите этот мод.",
                        entry.local.display()
                    ));
                }
            }
            Err(error) => return Err(error.to_string()),
        }
        downloads.push(Download { entry, target });
    }
    Ok(downloads)
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn amount(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} ГиБ", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.1} МиБ", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn download(cdns: [&str; 2], pending: &Path, item: Download, progress: impl Fn(u64)) -> Result<()> {
    let entry = item.entry;
    let path_on_disk = pending.join("file.part");
    if path_on_disk
        .symlink_metadata()
        .is_ok_and(|meta| !meta.is_file())
    {
        return Err("Временный файл загрузки должен быть обычным файлом".into());
    }
    let temp = Temporary(path_on_disk);
    let path = mods::url_path(&entry.remote);
    for cdn in cdns {
        core::check_cancelled()?;
        if temp.0.symlink_metadata().is_ok_and(|meta| !meta.is_file()) {
            return Err("Временный файл загрузки должен быть обычным файлом".into());
        }
        if temp.0.exists() {
            fs::remove_file(&temp.0).map_err(|e| e.to_string())?;
        }
        let mut child = mods::download_command(
            &format!("{cdn}/game/{path}"),
            &temp.0,
            entry.size.max(1),
            7200,
        )?
        .spawn()
        .map_err(|e| format!("Не запустить curl: {e}"))?;
        let mut tick = Instant::now();
        let status = loop {
            if let Err(error) = core::check_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            if tick.elapsed() >= Duration::from_millis(700) {
                progress(fs::metadata(&temp.0).map_or(0, |m| m.len()).min(entry.size));
                tick = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        if !status.success()
            || !fs::metadata(&temp.0).is_ok_and(|m| m.len() == entry.size)
            || mods::hash(&temp.0, "md5")? != entry.hash
        {
            eprintln!("Сервер не подтвердил файл: {}", entry.local.display());
            continue;
        }
        core::check_cancelled()?;
        let file = fs::File::open(&temp.0).map_err(|e| e.to_string())?;
        if let Some(time) = entry
            .modified
            .and_then(|time| UNIX_EPOCH.checked_add(Duration::from_secs(time)))
        {
            file.set_times(FileTimes::new().set_modified(time))
                .map_err(|e| e.to_string())?;
        }
        file.sync_all().map_err(|e| e.to_string())?;
        fs::create_dir_all(item.target.parent().ok_or("Нет каталога игрового файла")?)
            .map_err(|e| e.to_string())?;
        fs::rename(&temp.0, &item.target).map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err(format!(
        "Не удалось скачать или проверить {}. Повторите обновление.",
        entry.local.display()
    ))
}

pub fn run(s: &Settings, check_only: bool, prepare: impl FnOnce() -> Result<()>) -> Result<()> {
    core::progress("Получаем список файлов с серверов Arizona");
    let (entries, cdns) = manifest(s, variant(GameOptions::load(s)?.enabled("newGraphics")))?;
    let root = mods::game_path(s)?;
    let mut other = s.clone();
    other.project = match s.project {
        Project::Arizona => Project::Rodina,
        Project::Rodina => Project::Arizona,
    };
    if mods::game_path(&other).is_ok_and(|path| {
        path == root
            || path
                .canonicalize()
                .ok()
                .zip(root.canonicalize().ok())
                .is_some_and(|(a, b)| a == b)
    }) {
        return Err(
            "Для Arizona и Родины указана одна папка. Выберите отдельную папку для каждой игры."
                .into(),
        );
    }
    let protected = if root.is_dir() {
        mods::protected_game_files(s)?
    } else {
        BTreeSet::new()
    };
    let release_index = release_index(&entries);
    let downloads = inspect(&root, entries, &protected)?;
    let total = downloads.len();
    let bytes: u64 = downloads.iter().map(|item| item.entry.size).sum();
    println!("К загрузке: {total} файлов · {}", amount(bytes));
    if check_only {
        return Ok(());
    }
    core::check_cancelled()?;
    let release = mods::target(&root, Path::new(INSTALLED_RELEASE))?;
    let record_release = || mods::atomic_bytes(&release, release_index.as_bytes());
    if total == 0 && !unfinished(&root) && s.prefix_ready() {
        return record_release();
    }
    if total > 0 || unfinished(&root) {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let pending = mods::target(&root, Path::new(PENDING))?;
        fs::create_dir_all(&pending).map_err(|e| e.to_string())?;
        let mut completed = 0;
        for (index, item) in downloads.into_iter().enumerate() {
            let size = item.entry.size;
            let report = |current| {
                core::progress(&format!(
                    "Загрузка · файл {} из {total} · {} / {}",
                    index + 1,
                    amount(completed + current),
                    amount(bytes)
                ))
            };
            report(0);
            download(cdns, &pending, item, report)?;
            completed += size;
        }
        core::check_cancelled()?;
        prepare()?;
        core::progress("Завершаем установку игры");
        record_release()?;
        // Only this updater's temporary file may remain after a hard stop.
        let part = pending.join("file.part");
        if part.symlink_metadata().is_ok_and(|m| m.is_file()) {
            fs::remove_file(part).map_err(|e| e.to_string())?;
        }
        fs::remove_dir(pending).map_err(|e| e.to_string())?;
    } else {
        prepare()?;
        record_release()?;
    }
    Ok(())
}
