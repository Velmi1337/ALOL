//! Versioned experimental Proton distribution, isolated from games and other builds.
use crate::{
    core::{self, Result, Settings},
    json::{self, Value, object, string},
    mods,
};
use std::{fs, io::Write, path::PathBuf, process::Stdio};

pub const RECIPE_TEXT: &str = include_str!("../runtime/base.json");
pub const WRAPPER: &str = include_str!("../runtime/proton_wrapper.py");

fn recipe() -> Value {
    json::parse(RECIPE_TEXT).expect("embedded runtime recipe")
}

pub fn directory() -> PathBuf {
    core::data_dir()
        .join("runtimes")
        .join(recipe().get("runtime_name").text())
}

pub fn installed() -> Result<PathBuf> {
    let path = directory();
    let marker = fs::read_to_string(path.join("alol-runtime.json"))
        .map_err(|_| "ALOL-Proton ещё не подготовлен. В настройках окружения нажмите «Создать и выбрать ALOL-Proton» или выполните alol runtime-prepare.".to_string())?;
    let marker = json::parse(&marker)?;
    if marker.get("recipe").dump() != recipe().dump()
        || path.is_symlink()
        || !path.join("files/bin/wine").is_file()
        || !path.join("toolmanifest.vdf").is_file()
        || mods::hash(&path.join("compatibilitytool.vdf"), "sha256")?
            != marker.get("tool_vdf_sha256").text()
        || fs::read(path.join("proton")).ok().as_deref() != Some(WRAPPER.as_bytes())
        || mods::hash(&path.join("proton.alol-upstream"), "sha256")?
            != marker.get("upstream_proton_sha256").text()
    {
        return Err(
            "ALOL-Proton не прошёл проверку. Можно выбрать прежний GE-Proton; его файлы сохранены."
                .into(),
        );
    }
    if recipe().get("engine_patched").boolean() {
        if marker.get("keyboard_patch").get("protocol").text() != "xkb-state-v1"
            || !path.join("ALOL-WINE-SOURCE.tar.gz").is_file()
        {
            return Err("ALOL-Proton: отсутствует исправление раскладки или его исходники".into());
        }
        for arch in ["i386", "x86_64"] {
            let relative = format!("files/lib/wine/{arch}-windows/user32.dll");
            if mods::hash(&path.join(&relative), "sha256")?
                != recipe().get("engine_files").get(&relative).text()
            {
                return Err("ALOL-Proton: проверка исправления раскладки не пройдена; подготовьте сборку заново или выберите прежнюю версию".into());
            }
        }
    }
    Ok(path)
}

pub fn prepare(settings: &Settings, select: bool) -> Result<()> {
    if select && !core::available("umu-run") {
        return Err("Для выбора ALOL-Proton нужен установленный UMU (umu-run). Можно оставить Wine; системные пакеты автоматически не устанавливаются.".into());
    }
    println!(
        "ALOL-Proton: профиль Arizona и исправление индикатора EN/RU. Готовые проверяемые файлы, компиляция Wine не нужна. DXVK сохранён; преимущество по FPS не заявляется."
    );
    let resources = object([
        ("recipe", recipe()),
        ("wrapper", string(WRAPPER)),
        ("license", string(include_str!("../LICENSE"))),
        ("notice", string(include_str!("../NOTICE"))),
    ]);
    let mut child = crate::portable::command("python3")
        .args([
            "-I",
            "-c",
            include_str!("../runtime/build.py"),
            "--embedded",
            "--output-root",
        ])
        .arg(core::data_dir().join("runtimes"))
        .arg("--cache")
        .arg(core::data_dir().join("runtime-cache"))
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Не запустить сборщик ALOL-Proton: {e}"))?;
    let write = child
        .stdin
        .take()
        .ok_or("Нет канала сборщика")?
        .write_all(resources.dump().as_bytes());
    let status = child.wait().map_err(|e| e.to_string())?;
    write.map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("ALOL-Proton не подготовлен. Прежний выбор окружения и файлы игры сохранены; подробности в журнале.".into());
    }
    let path = installed()?;
    if select {
        let mut current = Settings::load()?;
        if current.runner == settings.runner
            && current.proton == settings.proton
            && current.prefix_root == settings.prefix_root
        {
            current.runner = "proton".into();
            current.proton = path.to_string_lossy().into();
            current.save()?;
            println!(
                "Выбран ALOL-Proton. Изменение действует при следующем запуске; вернуться к GE-Proton можно в настройках."
            );
        } else {
            println!(
                "Сборка готова; выбор окружения не изменён, поскольку настройки были изменены параллельно."
            );
        }
    }
    Ok(())
}
