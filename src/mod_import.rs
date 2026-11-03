//! Plan local mod imports without running scripts, installers or DLLs.
use crate::{archive, core::Result, mods};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const FOLDERS: &[&str] = &[
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
    "sampfuncs",
];
const ARCHIVES: &[&str] = &["zip", "rar", "7z", "tar", "gz", "xz", "bz2"];
pub struct Plan {
    pub entries: Vec<(PathBuf, PathBuf)>,
    pub loaders: BTreeSet<String>,
    pub missing_modules: BTreeSet<String>,
    pub required_paths: BTreeSet<String>,
    pub compiled: bool,
    pub oversized_script: bool,
    pub skipped: usize,
}
fn extension(path: &Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase()
}
fn ignored(path: &Path) -> bool {
    path.components().any(|part| {
        let name = part.as_os_str().to_string_lossy().to_lowercase();
        name == "__macosx"
            || name == ".ds_store"
            || name == "thumbs.db"
            || name == "desktop.ini"
            || name.starts_with("._")
    })
}
fn wrapper(name: &str) -> bool {
    [
        "dependencies",
        "dependency",
        "deps",
        "requirements",
        "required",
        "зависимости",
        "библиотеки",
        "в папку игры",
        "game",
        "gta san andreas",
        "arizona",
        "bin",
    ]
    .contains(&name)
}
fn gather(
    root: &Path,
    stage: &Path,
    out: &mut Vec<(PathBuf, PathBuf)>,
    depth: usize,
    total: &mut u64,
    archives: &mut usize,
) -> Result<()> {
    if depth > 3 {
        return Err("В моде слишком много вложенных архивов (не более трёх уровней)".into());
    }
    let mut base = root.to_path_buf();
    loop {
        let entries: Vec<_> = fs::read_dir(&base)
            .map_err(|e| e.to_string())?
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        let relevant: Vec<_> = entries
            .iter()
            .filter(|e| {
                !ignored(&e.path())
                    && !["txt", "md", "url", "html", "pdf"].contains(&extension(&e.path()).as_str())
            })
            .collect();
        if relevant.len() == 1
            && relevant[0].file_type().map_err(|e| e.to_string())?.is_dir()
            && !FOLDERS.contains(
                &relevant[0]
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .as_str(),
            )
            && relevant[0].file_name() != "lib"
            && relevant[0].file_name() != "config"
        {
            base = relevant[0].path();
        } else {
            break;
        }
    }
    for path in mods::files(&base)? {
        if ignored(&path) {
            continue;
        }
        let source = base.join(&path);
        if ARCHIVES.contains(&extension(&path).as_str()) {
            *archives += 1;
            if *archives > 64 {
                return Err("В моде больше 64 вложенных архивов".into());
            }
            let directory = stage.join(format!("nested-{}-{}", depth, out.len()));
            // Each archive has its own destination, including empty bundles.
            let mut directory = directory;
            let mut number = 0;
            while directory.exists() {
                number += 1;
                directory.set_file_name(format!("nested-{depth}-{}-{number}", out.len()));
            }
            fs::create_dir(&directory).map_err(|e| e.to_string())?;
            archive::extract(&source, &directory)?;
            gather(&directory, stage, out, depth + 1, total, archives)?;
        } else {
            *total += fs::metadata(&source).map_err(|e| e.to_string())?.len();
            if *total > 8 * 1024 * 1024 * 1024 || out.len() >= 20000 {
                return Err("Мод с зависимостями превышает лимит: 8 ГБ / 20000 файлов".into());
            }
            out.push((source, path));
        }
    }
    Ok(())
}
pub fn unpack(source: &Path, stage: &Path) -> Result<PathBuf> {
    let root = stage.join("raw");
    fs::create_dir(&root).map_err(|e| e.to_string())?;
    if ARCHIVES.contains(&extension(source).as_str()) {
        archive::extract(source, &root)?;
    } else if [
        "asi", "cs", "cs3", "cs4", "cleo", "sf", "lua", "luac", "dll", "ini", "cfg", "dff", "txd",
        "col", "ifp",
    ]
    .contains(&extension(source).as_str())
    {
        if fs::metadata(source).map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 * 1024 {
            return Err("Файл мода больше 2 ГБ".into());
        }
        fs::copy(
            source,
            root.join(source.file_name().ok_or("Нет имени файла")?),
        )
        .map_err(|e| e.to_string())?;
    } else {
        return Err("Выберите ZIP, RAR, 7z, TAR или файл ASI, CLEO, Lua, DLL, DFF, TXD".into());
    }
    Ok(root)
}
pub fn plan(raw: &Path, stage: &Path, game: &Path, slug: &str, mode: &str) -> Result<Plan> {
    if !["auto", "root", "modloader", "cleo", "moonloader"].contains(&mode) {
        return Err("Неизвестная папка установки мода".into());
    }
    let mut gathered = Vec::new();
    gather(raw, stage, &mut gathered, 0, &mut 0, &mut 0)?;
    let lua = gathered
        .iter()
        .any(|(_, p)| ["lua", "luac"].contains(&extension(p).as_str()));
    let cleo = gathered
        .iter()
        .any(|(_, p)| ["cs", "cs3", "cs4", "cleo"].contains(&extension(p).as_str()));
    let mut companions = BTreeMap::<String, BTreeSet<&str>>::new();
    let mut asi = false;
    for (_, p) in &gathered {
        let ext = extension(p);
        let kind = match ext.as_str() {
            "lua" | "luac" => "moonloader",
            "cs" | "cs3" | "cs4" | "cleo" => "cleo",
            "asi" => {
                asi = true;
                "root"
            }
            _ => continue,
        };
        companions
            .entry(
                p.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase(),
            )
            .or_default()
            .insert(kind);
    }
    let mut entries = BTreeMap::<String, (PathBuf, PathBuf)>::new();
    let mut skipped = 0;
    for (source, relative) in gathered {
        let mut parts: Vec<_> = relative
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        while parts.len() > 1 && wrapper(&parts[0].to_lowercase()) {
            parts.remove(0);
        }
        let relative = mods::safe_relative(&parts.join("/"))?;
        let first = parts[0].to_ascii_lowercase();
        let ext = extension(&relative);
        if ["exe", "msi", "bat", "cmd", "ps1", "sh", "lnk", "reg"].contains(&ext.as_str()) {
            return Err(format!(
                "Архив содержит установщик или командный файл: {}. Выберите архив с готовыми файлами мода; ALOL не выполняет установщики модов.",
                relative.display()
            ));
        }
        let destination = if mode != "auto" {
            match mode {
                "root" => relative.clone(),
                "modloader" => PathBuf::from("modloader").join(slug).join(&relative),
                _ => PathBuf::from(mode).join(&relative),
            }
        } else if FOLDERS.contains(&first.as_str()) {
            relative.clone()
        } else if ["lua", "luac"].contains(&ext.as_str())
            || first == "lib"
            || (lua && ["config", "resource", "resources"].contains(&first.as_str()))
        {
            PathBuf::from("moonloader").join(&relative)
        } else if ["cs", "cs3", "cs4", "cleo"].contains(&ext.as_str()) {
            PathBuf::from("cleo").join(&relative)
        } else if ext == "sf" {
            PathBuf::from("sampfuncs").join(&relative)
        } else if ["dff", "txd", "col", "ifp"].contains(&ext.as_str()) {
            PathBuf::from("modloader").join(slug).join(&relative)
        } else if ["txt", "md", "url", "html", "pdf", "png", "jpg", "jpeg"].contains(&ext.as_str())
            && parts.len() == 1
        {
            skipped += 1;
            continue;
        } else if ["asi", "dll", "ini", "cfg", "json", "dat", "ide", "ipl"].contains(&ext.as_str())
        {
            // A flat script with a neighbouring settings file uses its loader's directory.
            if parts.len() == 1 && ["ini", "cfg", "json"].contains(&ext.as_str()) && (lua || cleo) {
                let matching = companions.get(
                    &relative
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase(),
                );
                let kind = if let Some(matching) = matching.filter(|set| set.len() == 1) {
                    *matching.first().unwrap()
                } else if (lua && cleo) || asi || matching.is_some_and(|set| set.len() > 1) {
                    return Err(format!(
                        "Неоднозначное назначение {}: несколько видов модов. Выберите расположение в ALOL или архив с папками соответствующих загрузчиков.",
                        relative.display()
                    ));
                } else if lua {
                    "moonloader"
                } else {
                    "cleo"
                };
                if kind == "root" {
                    relative.clone()
                } else {
                    PathBuf::from(kind).join(&relative)
                }
            } else {
                relative.clone()
            }
        } else {
            return Err(format!(
                "Не удалось определить назначение {}. В списке расположения выберите папку мода или используйте архив с папками moonloader / cleo / modloader.",
                relative.display()
            ));
        };
        let key = destination.to_string_lossy().to_lowercase();
        if let Some((existing, _)) = entries.get(&key) {
            if mods::hash(existing, "sha256")? != mods::hash(&source, "sha256")? {
                return Err(format!(
                    "Два разных файла предназначены для {}",
                    destination.display()
                ));
            }
        } else {
            entries.insert(key, (source, destination));
        }
    }
    if entries.is_empty() {
        return Err("В архиве нет распознанных файлов мода".into());
    }
    let mut plan = Plan {
        entries: entries.into_values().collect(),
        loaders: BTreeSet::new(),
        missing_modules: BTreeSet::new(),
        required_paths: BTreeSet::new(),
        compiled: false,
        oversized_script: false,
        skipped,
    };
    let destinations: BTreeMap<_, _> = plan
        .entries
        .iter()
        .map(|(src, dst)| (dst.to_string_lossy().to_lowercase(), src.clone()))
        .collect();
    for (folder, loader, candidates) in [
        ("moonloader/", "MoonLoader", &["moonloader.asi"][..]),
        ("cleo/", "CLEO", &["cleo.asi"][..]),
        ("modloader/", "Mod Loader", &["modloader.asi"][..]),
        ("sampfuncs/", "SAMPFUNCS", &["sampfuncs.asi"][..]),
    ] {
        if destinations.keys().any(|p| p.starts_with(folder)) {
            if let Some(p) = candidates.iter().find(|p| {
                destinations.contains_key(**p)
                    || mods::target(game, Path::new(p)).is_ok_and(|p| p.is_file())
            }) {
                plan.required_paths.insert(p.to_string());
            } else {
                plan.loaders.insert(loader.into());
            }
        }
    }
    // Parse literal requires only, following reachable Lua modules. Compiled Lua
    // and dynamic requires cannot be reliably inspected without running code.
    let mut queue: Vec<_> = plan
        .entries
        .iter()
        .filter(|(_, p)| p.parent() == Some(Path::new("moonloader")))
        .map(|(s, _)| s.clone())
        .collect();
    let mut visited = BTreeSet::new();
    while let Some(source) = queue.pop() {
        if visited.len() >= 20000 {
            return Err("Слишком длинная цепочка Lua-зависимостей".into());
        }
        if !visited.insert(source.clone())
            || !["lua", "luac"].contains(&extension(&source).as_str())
        {
            continue;
        }
        if fs::metadata(&source).map_err(|e| e.to_string())?.len() > 8 * 1024 * 1024 {
            plan.oversized_script = true;
            continue;
        }
        let bytes = fs::read(&source).map_err(|e| e.to_string())?;
        if bytes.first() == Some(&0x1b) || extension(&source) == "luac" {
            plan.compiled = true;
            continue;
        }
        for module in requires(&String::from_utf8_lossy(&bytes)) {
            if [
                "ffi",
                "bit",
                "jit",
                "table",
                "string",
                "math",
                "io",
                "os",
                "coroutine",
                "debug",
                "package",
                "utf8",
                "moonloader",
                "memory",
                "sampfuncs",
            ]
            .contains(&module.as_str())
            {
                continue;
            }
            let relative = module.replace('.', "/");
            let relative = relative.strip_prefix("lib/").unwrap_or(&relative);
            let options = [
                format!("moonloader/lib/{relative}.lua"),
                format!("moonloader/lib/{relative}.luac"),
                format!("moonloader/lib/{relative}.dll"),
                format!("moonloader/lib/{relative}/init.lua"),
            ];
            let found = options.iter().find_map(|p| {
                destinations
                    .get(&p.to_lowercase())
                    .cloned()
                    .or_else(|| {
                        mods::target(game, Path::new(p))
                            .ok()
                            .filter(|p| p.is_file())
                    })
                    .map(|s| (p.clone(), s))
            });
            if let Some((path, source)) = found {
                plan.required_paths.insert(path);
                queue.push(source);
            } else {
                plan.missing_modules.insert(module);
            }
        }
    }
    Ok(plan)
}

// A small Lua lexer skips comments and quoted/long strings, so text in manuals,
// ffi.cdef blocks and comments does not become a fake dependency.
fn requires(text: &str) -> BTreeSet<String> {
    let b = text.as_bytes();
    let mut i = 0;
    let mut result = BTreeSet::new();
    while i < b.len() {
        if b[i..].starts_with(b"--") {
            i += 2;
            if let Some(end) = long_string_end(b, i) {
                i = end;
            } else {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
        } else if let Some(end) = long_string_end(b, i) {
            i = end;
        } else if b[i] == b'\'' || b[i] == b'"' {
            i = quoted_end(b, i);
        } else if b[i..].starts_with(b"require")
            && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b"_.:".contains(&b[i - 1])))
            && b.get(i + 7)
                .is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_'))
        {
            i += 7;
            while b.get(i).is_some_and(u8::is_ascii_whitespace) {
                i += 1;
            }
            if b.get(i) == Some(&b'(') {
                i += 1;
                while b.get(i).is_some_and(u8::is_ascii_whitespace) {
                    i += 1;
                }
            }
            if b.get(i).is_some_and(|c| *c == b'\'' || *c == b'"') {
                let end = quoted_end(b, i);
                if end > i + 1 && end <= b.len() && b[end - 1] == b[i] {
                    let module = &text[i + 1..end - 1];
                    if !module.is_empty()
                        && module.len() < 200
                        && module
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"_./-".contains(&c))
                        && !module.contains("..")
                    {
                        result.insert(module.into());
                    }
                }
                i = end;
            }
        } else {
            i += 1;
        }
    }
    result
}
fn quoted_end(b: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2;
        } else if b[i] == b[start] {
            return i + 1;
        } else {
            i += 1;
        }
    }
    b.len()
}
fn long_string_end(b: &[u8], start: usize) -> Option<usize> {
    if b.get(start) != Some(&b'[') {
        return None;
    }
    let mut i = start + 1;
    while b.get(i) == Some(&b'=') {
        i += 1;
    }
    if b.get(i) != Some(&b'[') {
        return None;
    }
    let equals = i - start - 1;
    i += 1;
    while i < b.len() {
        if b[i] == b']'
            && b.get(i + 1..i + 1 + equals)
                .is_some_and(|s| s.iter().all(|c| *c == b'='))
            && b.get(i + 1 + equals) == Some(&b']')
        {
            return Some(i + 2 + equals);
        }
        i += 1;
    }
    Some(b.len())
}
impl Plan {
    pub fn summary(&self) -> String {
        let mut s = format!("Распознано файлов: {}\n", self.entries.len());
        let mut groups = BTreeMap::<String, usize>::new();
        for (_, p) in &self.entries {
            *groups
                .entry(p.parent().unwrap_or(Path::new("")).to_string_lossy().into())
                .or_default() += 1;
        }
        for (folder, count) in groups.iter().take(12) {
            s.push_str(&format!(
                "  {}: {count}\n",
                if folder.is_empty() {
                    "Корень игры"
                } else {
                    folder
                }
            ));
        }
        if groups.len() > 12 {
            s.push_str("  …и вложенные папки библиотек\n");
        }
        if !self.loaders.is_empty() {
            s.push_str(&format!("Нужны загрузчики: {}. При установке ALOL ищет их в официальном каталоге Arizona.\n", self.loaders.iter().cloned().collect::<Vec<_>>().join(", ")));
        }
        if !self.missing_modules.is_empty() {
            s.push_str(&format!("Не найдены Lua-библиотеки: {}. Добавьте архив зависимостей через ALOL и повторите проверку.\n", self.missing_modules.iter().cloned().collect::<Vec<_>>().join(", ")));
        }
        if self.compiled {
            s.push_str("Есть скомпилированный Lua: внутренние зависимости и совместимость версии проверить статически нельзя.\n");
        }
        if self.oversized_script {
            s.push_str("Есть Lua-файл больше 8 МБ: статический разбор этого файла пропущен.\n");
        }
        if self.skipped > 0 {
            s.push_str(&format!(
                "Пропущено документов и обложек: {}\n",
                self.skipped
            ));
        }
        s.push_str(
            "Файлы будут установлены с резервной копией. Отключение мода восстановит оригиналы.",
        );
        s
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_requires_skip_comments_strings_and_members() {
        let t = "-- require 'no'\nlocal a=require('samp.events'); local b = require \"encoding\"\nlocal x=[[require 'fake']]; --[=[require 'other']=]\nshared.require 'object'; local d='require \\\"fake2\\\"'";
        assert_eq!(
            requires(t),
            ["encoding".into(), "samp.events".into()].into()
        );
    }
}
