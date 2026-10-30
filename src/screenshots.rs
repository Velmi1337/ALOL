//! An isolated, disposable profile for screenshots of the real GTK interface.
use crate::{
    core::{self, Result, Settings},
    game::PlayerProfile,
    game_options::GameOptions,
    gui, json, mods,
};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};

pub fn run(output: &Path) -> Result<()> {
    fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let output = output.canonicalize().map_err(|e| e.to_string())?;
    let _demo = Demo::new()?;
    gui::run_mode(Some(output))
}

struct Demo {
    root: PathBuf,
}
impl Drop for Demo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
impl Demo {
    fn new() -> Result<Self> {
        let root = PathBuf::from(format!(
            "/tmp/alol-demo-{}-{}",
            std::process::id(),
            core::stamp()
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .map_err(|e| e.to_string())?;
        let demo = Self { root };
        let home = demo.root.join("home");
        fs::create_dir_all(&home).map_err(|e| e.to_string())?;
        // This command runs before GTK or any worker thread starts.
        // Environment changes affect only this disposable CLI process.
        unsafe {
            std::env::set_var("HOME", &home);
            std::env::set_var("XDG_CONFIG_HOME", demo.root.join("config"));
            std::env::set_var("XDG_DATA_HOME", demo.root.join("data"));
            std::env::set_var("XDG_CACHE_HOME", demo.root.join("cache"));
            std::env::set_var("GTK_THEME", "Adwaita:dark");
        }
        let settings = Settings::default();
        settings.save()?;
        let launcher = settings.prefix().join("drive_c/Arizona Games Launcher");
        let game = launcher.join("bin/arizona");
        fs::create_dir_all(&game).map_err(|e| e.to_string())?;
        // Existence-only fixtures for the ready screen; demo actions never execute.
        fs::write(settings.prefix().join("system.reg"), []).map_err(|e| e.to_string())?;
        fs::write(
            launcher.join("Arizona Games Launcher.exe"),
            b"ALOL demo fixture",
        )
        .map_err(|e| e.to_string())?;
        fs::write(game.join("gta_sa.exe"), b"ALOL demo fixture").map_err(|e| e.to_string())?;
        PlayerProfile {
            server_id: "1".into(),
            nickname: "Nick_Name".into(),
            nicknames: BTreeMap::from([("1".into(), "Nick_Name".into())]),
        }
        .save()?;
        GameOptions::load(&settings)?.save()?;
        let catalog = json::object([(
            "mods",
            json::Value::Array(vec![
                json::object([
                    ("id", json::Value::Number("1".into())),
                    ("title", json::string("Moonloader")),
                    (
                        "description",
                        json::string("Демонстрационная запись: загрузчик Lua-скриптов."),
                    ),
                ]),
                json::object([
                    ("id", json::Value::Number("2".into())),
                    ("title", json::string("Пример мода")),
                    (
                        "description",
                        json::string("Демонстрационная запись для показа интерфейса каталога."),
                    ),
                ]),
            ]),
        )]);
        mods::atomic_bytes(
            &core::data_dir().join("mods/catalog.json"),
            catalog.dump().as_bytes(),
        )?;
        Ok(demo)
    }
}
