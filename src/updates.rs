//! GitHub release updates. Package-owned executables always use their package manager.
use crate::{
    core::{self, Result},
    json::{self, Value, object},
    mods,
};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::Stdio,
    time::{SystemTime, UNIX_EPOCH},
};

const API: &str = "https://api.github.com/repos/Velmi1337/ALOL/releases/latest";
const DOWNLOADS: &str = "https://github.com/Velmi1337/ALOL/releases/download";
const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;
const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    assets: Vec<Asset>,
}
#[derive(Clone, Debug)]
struct Asset {
    name: String,
    sha256: String,
    size: u64,
}

fn version(text: &str) -> Result<[u32; 3]> {
    let parts = text.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|p| {
            p.is_empty()
                || !p.bytes().all(|b| b.is_ascii_digit())
                || (p.len() > 1 && p.starts_with('0'))
        })
    {
        return Err("Некорректная версия обновления ALOL".into());
    }
    let mut result = [0; 3];
    for (number, part) in result.iter_mut().zip(parts) {
        *number = part
            .parse()
            .map_err(|_| "Слишком большой номер версии ALOL")?;
    }
    Ok(result)
}

impl Release {
    fn parse(value: &Value) -> Result<Self> {
        if value.get("draft").boolean() || value.get("prerelease").boolean() {
            return Err("Предварительные версии автоматически не устанавливаются".into());
        }
        let tag = value.get("tag_name").text();
        let release_version = tag.strip_prefix('v').ok_or("Неизвестный тег релиза ALOL")?;
        version(release_version)?;
        let mut assets = Vec::new();
        for asset in value.get("assets").array() {
            let name = asset.get("name").text();
            // Construct downloads ourselves; never follow a URL supplied by local cache data.
            if name.is_empty()
                || name.len() > 160
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-._".contains(&b))
            {
                continue;
            }
            let digest = asset.get("digest").text();
            let Some(sha256) = digest.strip_prefix("sha256:") else {
                continue;
            };
            if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                continue;
            }
            let size = asset.get("size").text().parse::<u64>().unwrap_or(0);
            if !(1..=MAX_DOWNLOAD).contains(&size) || assets.iter().any(|a: &Asset| a.name == name)
            {
                continue;
            }
            assets.push(Asset {
                name,
                sha256: sha256.to_ascii_lowercase(),
                size,
            });
        }
        Ok(Self {
            version: release_version.into(),
            notes: value
                .get("body")
                .text()
                .chars()
                .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
                .take(16000)
                .collect(),
            assets,
        })
    }

    fn asset(&self, kind: &InstallKind) -> Result<&Asset> {
        let binary = format!("alol-{}-linux-x86_64", self.version);
        let image = format!("alol-{}-x86_64.AppImage", self.version);
        let arch = format!("alol-{}-1-x86_64.pkg.tar.zst", self.version);
        let rpm_prefix = format!("alol-{}-1.fc", self.version);
        self.assets.iter().find(|a| match kind {
            InstallKind::Local => a.name == binary,
            InstallKind::AppImage => a.name == image,
            InstallKind::Arch => a.name == arch,
            InstallKind::Fedora => a.name.strip_prefix(&rpm_prefix).and_then(|n| n.strip_suffix(".x86_64.rpm")).is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())),
        }).ok_or_else(|| "Файл обновления с SHA256 ещё не опубликован. Попробуйте проверить обновления позже.".into())
    }
}

fn cache_file() -> PathBuf {
    core::data_dir().join("updates/release.json")
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

struct Staging(PathBuf);
impl Staging {
    fn new() -> Result<Self> {
        let root = core::data_dir().join("updates");
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let directory = root.join(format!("download-{}-{}", std::process::id(), core::stamp()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| e.to_string())?;
        Ok(Self(directory))
    }
}
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Cache successful checks for a day. A manual check always reaches GitHub.
pub fn check(force: bool) -> Result<Option<Release>> {
    check_cached(if force { 0 } else { 86400 })
}

pub fn check_scheduled(policy: &str) -> Result<Option<Release>> {
    match policy {
        "startup" => check_cached(0),
        "daily" => check_cached(86400),
        "weekly" => check_cached(7 * 86400),
        "manual" => Ok(None),
        _ => Err("Неизвестный режим проверки обновлений".into()),
    }
}

fn cache_fresh(checked_at: u64, timestamp: u64, interval: u64) -> bool {
    checked_at <= timestamp && timestamp - checked_at < interval
}

fn check_cached(interval: u64) -> Result<Option<Release>> {
    if std::env::consts::ARCH != "x86_64" {
        return Err("Автообновление доступно для x86_64 Linux".into());
    }
    let value = if interval > 0 {
        fs::read_to_string(cache_file())
            .ok()
            .filter(|s| s.len() <= 2 * 1024 * 1024)
            .and_then(|s| json::parse(&s).ok())
            .filter(|v| {
                v.get("checked_at")
                    .text()
                    .parse::<u64>()
                    .is_ok_and(|t| cache_fresh(t, now(), interval))
            })
            .map(|v| v.get("release").clone())
    } else {
        None
    };
    let value = match value {
        Some(value) if Release::parse(&value).is_ok() => value,
        _ => {
            let stage = Staging::new()?;
            let path = stage.0.join("release.json");
            mods::fetch_with_timeout(API, &path, 2 * 1024 * 1024, 15)?;
            let value = json::parse(&fs::read_to_string(&path).map_err(|e| e.to_string())?)?;
            Release::parse(&value)?;
            mods::atomic_bytes(
                &cache_file(),
                object([
                    ("checked_at", Value::Number(now().to_string())),
                    ("release", value.clone()),
                ])
                .dump()
                .as_bytes(),
            )?;
            value
        }
    };
    let release = Release::parse(&value)?;
    if version(&release.version)? > version(CURRENT)? {
        Ok(Some(release))
    } else {
        Ok(None)
    }
}

#[derive(Debug)]
enum InstallKind {
    Local,
    AppImage,
    Arch,
    Fedora,
}

fn installation(executable: &Path) -> Result<InstallKind> {
    for (tool, args, kind) in [
        ("pacman", vec!["-Qoq"], InstallKind::Arch),
        ("rpm", vec!["-qf", "--qf", "%{NAME}"], InstallKind::Fedora),
    ] {
        if let Ok(output) = crate::portable::command(tool)
            .args(args)
            .arg(executable)
            .output()
            && output.status.success()
        {
            if String::from_utf8_lossy(&output.stdout).trim() == "alol" {
                return Ok(kind);
            }
            return Err("Этот файл принадлежит другому системному пакету. Обновите его средствами дистрибутива.".into());
        }
    }
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    let parent = executable.parent().ok_or("Нет папки приложения")?;
    let file_meta = executable.metadata().map_err(|e| e.to_string())?;
    let parent_meta = parent.metadata().map_err(|e| e.to_string())?;
    if unsafe { getuid() } == 0
        || file_meta.uid() != unsafe { getuid() }
        || parent_meta.uid() != unsafe { getuid() }
    {
        return Err("Не удалось определить способ установки. Установите пакет ALOL из GitHub Releases вручную.".into());
    }
    Ok(InstallKind::Local)
}

struct Lock(File);
impl Lock {
    fn acquire() -> Result<Self> {
        let path = cache_file().with_file_name("update.lock");
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)
            .map_err(|e| e.to_string())?;
        unsafe extern "C" {
            fn flock(fd: i32, operation: i32) -> i32;
        }
        if unsafe { flock(file.as_raw_fd(), 2 | 4) } != 0 {
            return Err("Обновление ALOL уже выполняется в другом окне".into());
        }
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe extern "C" {
            fn flock(fd: i32, operation: i32) -> i32;
        }
        unsafe {
            flock(self.0.as_raw_fd(), 8);
        }
    }
}

fn binary_version(executable: &Path) -> Result<String> {
    let output = crate::portable::command(executable)
        .env("APPIMAGE_EXTRACT_AND_RUN", "1")
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("Не удалось проверить версию приложения".into());
    }
    let text = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let release_version = text
        .trim()
        .strip_prefix("ALOL ")
        .ok_or("Неизвестный формат версии приложения")?;
    version(release_version)?;
    Ok(release_version.into())
}

fn replace_binary(source: &Path, executable: &Path, expected: &str, appimage: bool) -> Result<()> {
    let parent = executable.parent().ok_or("Нет папки приложения")?;
    let temporary = parent.join(format!(
        ".alol-update-{}-{}.tmp",
        std::process::id(),
        core::stamp()
    ));
    let result = (|| {
        let mut input = File::open(source).map_err(|e| e.to_string())?;
        let mut header = [0; 20];
        input.read_exact(&mut header).map_err(|e| e.to_string())?;
        if &header[..6] != b"\x7fELF\x02\x01" || header[18..20] != [0x3e, 0] {
            return Err("Обновление не является приложением ELF x86_64".into());
        }
        if appimage && &header[8..11] != b"AI\x02" {
            return Err("Обновление не является AppImage типа 2".into());
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let mut input = File::open(source).map_err(|e| e.to_string())?;
        std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output); // An executable cannot run while still open for writing (ETXTBSY).
        if binary_version(&temporary)? != expected {
            return Err("Версия загруженного приложения не совпадает с релизом".into());
        }
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        // Same filesystem + rename: a failed download/install leaves the old executable intact.
        fs::rename(&temporary, executable).map_err(|e| e.to_string())?;
        File::open(parent)
            .and_then(|file| file.sync_all())
            .map_err(|e| e.to_string())
    })();
    let _ = fs::remove_file(&temporary);
    result
}

pub fn install(expected: Option<&str>) -> Result<()> {
    if crate::desktop::development_copy() {
        return Err("Это рабочая копия проекта. Обновляйте её через Git и пересборку; версия из GitHub устанавливается отдельно.".into());
    }
    let _lock = Lock::acquire()?;
    let executable = crate::portable::executable()
        .and_then(|p| p.canonicalize())
        .map_err(|e| e.to_string())?;
    let Some(release) = check(true)? else {
        println!("ALOL {CURRENT} — актуальная версия");
        return Ok(());
    };
    if expected.is_some_and(|v| v != release.version) {
        return Err("На GitHub появилась другая версия. Проверьте обновления ещё раз.".into());
    }
    let kind = match installation(&executable)? {
        InstallKind::Local if crate::portable::appimage().is_some() => InstallKind::AppImage,
        kind => kind,
    };
    let asset = release.asset(&kind)?;
    let stage = Staging::new()?;
    let path = stage.0.join(&asset.name);
    println!("Загрузка ALOL {}…", release.version);
    mods::fetch(
        &format!("{DOWNLOADS}/v{}/{}", release.version, asset.name),
        &path,
        MAX_DOWNLOAD,
    )?;
    if path.metadata().map_err(|e| e.to_string())?.len() != asset.size
        || mods::hash(&path, "sha256")? != asset.sha256
    {
        return Err(
            "Проверка SHA256/размера обновления не пройдена. Установленная версия сохранена."
                .into(),
        );
    }
    println!("Размер и SHA256 обновления проверены.");
    match kind {
        InstallKind::Local | InstallKind::AppImage => replace_binary(
            &path,
            &executable,
            &release.version,
            matches!(kind, InstallKind::AppImage),
        )?,
        InstallKind::Arch | InstallKind::Fedora => {
            let tool = if matches!(kind, InstallKind::Arch) {
                "pacman"
            } else {
                "dnf"
            };
            let tool_path = Path::new("/usr/bin").join(tool);
            if !tool_path.is_file() || !core::available("pkexec") {
                return Err("Для обновления пакета нужны pkexec и системный пакетный менеджер. Скачайте пакет из GitHub Releases.".into());
            }
            println!("Установка пакета: подтвердите системный запрос прав администратора.");
            let status = crate::portable::command("pkexec")
                .arg(tool_path)
                .args(if matches!(kind, InstallKind::Arch) {
                    vec!["-U", "--noconfirm", "--"]
                } else {
                    vec!["install", "--assumeyes", "--"]
                })
                .arg(&path)
                .status()
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err(format!(
                    "Установка обновления отменена или завершилась ошибкой: {status}. Попробуйте снова."
                ));
            }
            if binary_version(&executable)? != release.version {
                return Err("Пакетный менеджер не подтвердил новую версию приложения".into());
            }
        }
    }
    println!(
        "ALOL {} установлена. Перезапустите приложение.",
        release.version
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schedules_do_not_cache_startup_checks_or_future_timestamps() {
        assert!(!cache_fresh(100, 100, 0));
        assert!(cache_fresh(100, 101, 86400));
        assert!(!cache_fresh(100, 86500, 86400));
        assert!(cache_fresh(100, 86500, 7 * 86400));
        assert!(!cache_fresh(101, 100, 86400));
    }
    #[test]
    fn versions_compare_numerically_and_reject_tags_with_paths() {
        assert!(version("0.10.0").unwrap() > version("0.9.99").unwrap());
        for invalid in [
            "v0.5.4",
            "0.5",
            "0.5.4-beta",
            "0.05.4",
            "0.5.4/../x",
            "0.5.4294967296",
        ] {
            assert!(version(invalid).is_err());
        }
    }
    #[test]
    fn release_requires_stable_version_and_verified_assets() {
        let mut value = json::parse(r#"{"tag_name":"v0.5.4","assets":[{"name":"../evil","size":5,"digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}"#).unwrap();
        let release = Release::parse(&value).unwrap();
        assert!(release.asset(&InstallKind::Local).is_err());
        value.put("prerelease", Value::Bool(true));
        assert!(Release::parse(&value).is_err());
        value.put("prerelease", Value::Bool(false));
        value.put("tag_name", json::string("v0.5.4/evil"));
        assert!(Release::parse(&value).is_err());
    }
}
