mod alol_proton;
mod archive;
mod core;
mod desktop;
mod file_watch;
mod fonts;
mod game;
mod game_options;
mod gui;
mod json;
mod keyboard;
mod language;
mod logs;
mod mod_import;
mod mods;
mod portable;
mod project;
mod screenshots;
mod updates;

fn run() -> core::Result<()> {
    let mut args: Vec<String> = std::env::args().collect();
    let project = if args.get(1).is_some_and(|arg| arg == "--project") {
        let project = project::Project::parse(
            args.get(2)
                .ok_or("После --project укажите arizona или rodina")?,
        )?;
        args.drain(1..3);
        Some(project)
    } else {
        None
    };
    match args.get(1).map(String::as_str) {
        None => {
            if let Some(project) = project {
                let mut settings = core::Settings::load()?;
                settings.project = project;
                settings.save()?;
            }
            gui::run()
        }
        Some("--version" | "-V") => {
            println!("ALOL {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--license") => {
            print!(
                "{}\n{}",
                include_str!("../NOTICE"),
                include_str!("../LICENSE")
            );
            Ok(())
        }
        Some("--help" | "-h") => {
            println!(
                "ALOL — Arizona Launcher On Linux\n\nБез аргументов: графический интерфейс\n\nalol --license     Лицензия и сведения об авторстве\nalol check-update Проверить обновления ALOL\nalol self-update Обновить ALOL из GitHub Releases\nalol project arizona|rodina Переключить игру\nalol --project rodina play [SERVER_ID] Запуск Родины; никнейм вводится в игре\nalol logs-clear   Очистить текущий и архивный журнал ALOL\nalol doctor       Диагностика\nalol prepare      Загрузить Proton и подготовить окружение\nalol runtime-prepare [--select] Создать экспериментальный ALOL-Proton\nalol install      Установить ArizonaLauncher.exe из настроек\nalol play [SERVER_ID] [Nick_Name] Подключиться к игре напрямую\nalol servers-refresh Обновить список серверов\nalol launch       Открыть официальный лаунчер для обновления игры\nalol repair       Обновить окружение и сохранить старый кэш\nalol repair-game  Исправить метки Windows в папке игры\nalol repair-language Исправить русский текст (Windows-1251)\nalol libraries    Установить corefonts и vcrun2022\nalol reset-game   Сохранить и сбросить gta_sa.set\nalol winecfg      Открыть настройки Wine\nalol mods-refresh Обновить официальный каталог\nalol mods-install ID  Установить официальный мод\nalol mods-add FILE [auto|root|modloader|cleo|moonloader]\nalol mods-preview FILE [auto|root|modloader|cleo|moonloader] Проверить импорт\nalol clean-cache Сбросить кэш интерфейса Arizona с копией\nalol mods-enable ID / mods-disable ID / mods-remove ID\nalol --smoke-test Проверить создание интерфейса\nalol --screenshots DIR Снять три экрана с отдельным демо-профилем\n\nНастройки: ~/.config/alol/settings.conf\nИгра: ~/.config/alol/game-options.json\nСервер и ники: ~/.config/alol/player.json\nЛоги: ~/.local/share/alol/logs/"
            );
            Ok(())
        }
        Some("--smoke-test") => gui::run(),
        Some("logs-clear") => {
            logs::clear().map(|bytes| println!("Журнал ALOL очищен: {bytes} байт"))
        }
        Some("check-update") => updates::check(true).map(|release| match release {
            Some(release) => println!(
                "Доступна ALOL {}. Для установки: alol self-update",
                release.version
            ),
            None => println!("ALOL {} — актуальная версия", env!("CARGO_PKG_VERSION")),
        }),
        Some("self-update") => updates::install(args.get(2).map(String::as_str)),
        Some("--screenshots") => args
            .get(2)
            .ok_or_else(|| "Укажите папку для PNG: alol --screenshots docs/screenshots".into())
            .and_then(|path| screenshots::run(std::path::Path::new(path))),
        Some(action) => core::execute(action, &args[2..], project),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Ошибка: {error}");
        std::process::exit(1);
    }
}
