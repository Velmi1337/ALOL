use std::{
    env, fs,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub type Result<T> = std::result::Result<T, String>;
const WINEBOOT: &str = r"C:\windows\system32\wineboot.exe";
const REG: &str = r"C:\windows\system32\reg.exe";
const WINECFG: &str = r"C:\windows\system32\winecfg.exe";
static TERMINATED: AtomicBool = AtomicBool::new(false);
extern "C" fn terminate(_: i32) {
    TERMINATED.store(true, Ordering::Relaxed);
}

pub fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn xdg(name: &str, fallback: &str) -> PathBuf {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
}

pub fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("alol")
}
pub fn config_file() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("alol/settings.conf")
}
pub fn log_file() -> PathBuf {
    data_dir().join("logs/alol.log")
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub runner: String,
    pub proton: String,
    pub wine: String,
    pub installer: String,
    pub executable: String,
    pub prefix_root: String,
    pub wined3d: bool,
    pub disable_gpu: bool,
    pub discrete_gpu: bool,
    pub debug: bool,
    pub no_sync: bool,
    pub launcher_compatibility: bool,
    pub input_compatibility: bool,
    pub update_policy: String,
    pub update_notifications: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            runner: if !available("umu-run") && available("wine") {
                "wine"
            } else {
                "proton"
            }
            .into(),
            proton: "GE-Proton".into(),
            wine: "wine".into(),
            installer: String::new(),
            executable: String::new(),
            prefix_root: data_dir().join("prefixes").to_string_lossy().into(),
            wined3d: false,
            disable_gpu: true,
            discrete_gpu: false,
            debug: false,
            no_sync: false,
            launcher_compatibility: true,
            input_compatibility: true,
            update_policy: "startup".into(),
            update_notifications: true,
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self> {
        let mut s = Self::default();
        match fs::read_to_string(config_file()) {
            Ok(text) => s.parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(s),
            Err(e) => Err(format!("Не прочитать настройки: {e}")),
        }
    }

    fn parse(&mut self, text: &str) -> Result<Self> {
        for line in text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
        {
            let Some((k, v)) = line.split_once('=') else {
                return Err("Повреждён settings.conf: ожидается ключ=значение".into());
            };
            match k {
                "runner" => self.runner = v.into(),
                "proton" => self.proton = v.into(),
                "wine" => self.wine = v.into(),
                "installer" => self.installer = v.into(),
                "executable" => self.executable = v.into(),
                "prefix_root" => self.prefix_root = v.into(),
                "wined3d" => self.wined3d = v == "true",
                "disable_gpu" => self.disable_gpu = v == "true",
                "discrete_gpu" => self.discrete_gpu = v == "true",
                "debug" => self.debug = v == "true",
                "no_sync" => self.no_sync = v == "true",
                "launcher_compatibility" => self.launcher_compatibility = v == "true",
                "input_compatibility" => self.input_compatibility = v == "true",
                "update_policy" => self.update_policy = v.into(),
                "update_notifications" => self.update_notifications = v == "true",
                _ => (),
            }
        }
        self.validate()?;
        Ok(self.clone())
    }

    pub fn validate(&self) -> Result<()> {
        if !["startup", "daily", "weekly", "manual"].contains(&self.update_policy.as_str()) {
            return Err("Неизвестный режим проверки обновлений".into());
        }
        for value in [
            &self.runner,
            &self.proton,
            &self.wine,
            &self.installer,
            &self.executable,
            &self.prefix_root,
        ] {
            if value.contains(['\n', '\r', '\0']) {
                return Err("Пути не должны содержать перевод строки или NUL".into());
            }
        }
        if !["proton", "wine"].contains(&self.runner.as_str()) {
            return Err("Неизвестное окружение".into());
        }
        let root = Path::new(&self.prefix_root);
        if !root.is_absolute()
            || root.parent().is_none()
            || root
                .components()
                .any(|c| c == std::path::Component::ParentDir)
        {
            return Err("Укажите абсолютную папку окружений без '..'".into());
        }
        if self.proton != "GE-Proton"
            && self.proton != "UMU-Proton"
            && !Path::new(&self.proton).is_absolute()
        {
            return Err(
                "Proton: выберите GE-Proton, UMU-Proton или абсолютный путь к сборке".into(),
            );
        }
        if self.wine.is_empty() {
            return Err("Укажите команду Wine".into());
        }
        for (label, path) in [
            ("Установщик", &self.installer),
            ("Лаунчер", &self.executable),
        ] {
            if !path.is_empty() && !Path::new(path).is_absolute() {
                return Err(format!("{label}: нужен абсолютный путь"));
            }
        }
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        let path = config_file();
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let text = format!(
            "# ALOL — пути записываются без кавычек\nrunner={}\nproton={}\nwine={}\ninstaller={}\nexecutable={}\nprefix_root={}\nwined3d={}\ndisable_gpu={}\ndiscrete_gpu={}\ndebug={}\nno_sync={}\nlauncher_compatibility={}\ninput_compatibility={}\nupdate_policy={}\nupdate_notifications={}\n",
            self.runner,
            self.proton,
            self.wine,
            self.installer,
            self.executable,
            self.prefix_root,
            self.wined3d,
            self.disable_gpu,
            self.discrete_gpu,
            self.debug,
            self.no_sync,
            self.launcher_compatibility,
            self.input_compatibility,
            self.update_policy,
            self.update_notifications
        );
        // Unique temp file and atomic rename keep readers from seeing a partial config.
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, text).map_err(|e| e.to_string())?;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        fs::rename(tmp, path).map_err(|e| e.to_string())
    }

    pub fn prefix(&self) -> PathBuf {
        Path::new(&self.prefix_root).join(&self.runner)
    }

    pub fn prefix_ready(&self) -> bool {
        self.prefix().join("drive_c").is_dir() && self.prefix().join("system.reg").is_file()
    }

    pub fn find_executable(&self) -> Option<PathBuf> {
        if !self.executable.is_empty() {
            return Path::new(&self.executable)
                .is_file()
                .then(|| PathBuf::from(&self.executable));
        }
        let mut matches = Vec::new();
        walk(&self.prefix().join("drive_c"), 9, &mut |p| {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if p.is_file()
                && [
                    "arizona games launcher.exe",
                    "arizona launcher.exe",
                    "arizonalauncher.exe",
                ]
                .contains(&name.as_str())
            {
                matches.push(p.to_path_buf());
            }
        });
        matches.sort_by_key(|p| (p.components().count(), p.clone()));
        matches.into_iter().next()
    }

    pub fn command(&self, executable: &str, args: &[&str]) -> Result<Command> {
        self.validate()?;
        let mut c = if self.runner == "proton" {
            if !available("umu-run") {
                return Err("Не найден umu-run. Установите UMU или выберите установленный Wine во вкладке «Настройки».".into());
            }
            if Path::new(&self.proton).is_absolute()
                && !Path::new(&self.proton).join("proton").is_file()
            {
                return Err("В выбранной папке Proton нет файла proton".into());
            }
            let mut c = Command::new("umu-run");
            c.env("PROTONPATH", &self.proton)
                .env("GAMEID", crate::desktop::arizona_game_id())
                .env("STORE", "none");
            // Never run an arbitrary inherited Proton verb such as destroyprefix.
            c.env("PROTON_VERB", "waitforexitandrun");
            c
        } else {
            if !available(&self.wine) {
                return Err(format!(
                    "Wine не найден: {}. Установите Wine и winetricks средствами вашего дистрибутива.",
                    self.wine
                ));
            }
            Command::new(&self.wine)
        };
        crate::language::configure(&mut c);
        c.env("WINEPREFIX", self.prefix())
            .env_remove("WINEARCH")
            .env_remove("WINELOADER")
            .env_remove("WINESERVER")
            .env_remove("ALOL_LOGIN_FILE")
            .env_remove("ALOL_LOGIN_TOKEN")
            .env(
                "WINEDEBUG",
                if self.debug {
                    "+timestamp,+pid,+tid,+seh"
                } else {
                    "-all"
                },
            )
            .env("PROTON_LOG", if self.debug { "1" } else { "0" })
            .env("PROTON_LOG_DIR", data_dir().join("logs"))
            .env("PROTON_USE_WINED3D", if self.wined3d { "1" } else { "0" })
            .env("PROTON_NO_ESYNC", if self.no_sync { "1" } else { "0" })
            .env("PROTON_NO_FSYNC", if self.no_sync { "1" } else { "0" });
        if self.no_sync {
            c.env("WINEESYNC", "0")
                .env("WINEFSYNC", "0")
                .env("PROTON_NO_NTSYNC", "1");
        }
        if self.discrete_gpu {
            c.env("DRI_PRIME", "1");
        }
        if self.runner == "wine" && self.wined3d {
            c.env("WINEDLLOVERRIDES", "d3d9,d3d10core,d3d11,dxgi=b");
        }
        if self.launcher_compatibility && self.is_launcher(executable) {
            launcher_environment(&mut c)?;
        }
        if self.input_compatibility && (is_game(executable) || self.is_launcher(executable)) {
            input_environment(&mut c);
        }
        c.arg(executable).args(args);
        if matches!(executable, WINEBOOT | REG | WINECFG) {
            // UMU preserves Windows paths, so Proton runs this real builtin rather
            // than its createprefix sentinel, which deliberately launches an empty EXE.
            // A real install directory also avoids Proton's empty game-drive path.
            c.env("STEAM_COMPAT_INSTALL_PATH", self.prefix())
                .current_dir(self.prefix());
        }
        let path = Path::new(executable);
        if path.is_absolute() && path.is_file() {
            c.current_dir(path.parent().unwrap());
        }
        Ok(c)
    }

    fn is_launcher(&self, executable: &str) -> bool {
        (!self.installer.is_empty() && executable == self.installer)
            || (!self.executable.is_empty() && executable == self.executable)
            || Path::new(executable).file_name().is_some_and(|n| {
                [
                    "arizona games launcher.exe",
                    "arizona launcher.exe",
                    "arizonalauncher.exe",
                ]
                .iter()
                .any(|name| n.eq_ignore_ascii_case(name))
            })
    }
}

/// A launch-only environment: dotfile overrides and injected Linux overlays often
/// break Wine's Electron process. Display/auth/runtime sockets stay available.
fn launcher_environment(command: &mut Command) -> Result<()> {
    for variable in [
        "LD_PRELOAD",
        "LD_AUDIT",
        "LD_LIBRARY_PATH",
        "GTK_THEME",
        "GTK_MODULES",
        "GTK_PATH",
        "QT_QPA_PLATFORMTHEME",
        "QT_STYLE_OVERRIDE",
        "QT_PLUGIN_PATH",
        "QT_QPA_PLATFORM_PLUGIN_PATH",
        "QML2_IMPORT_PATH",
        "ELECTRON_OZONE_PLATFORM_HINT",
        "OZONE_PLATFORM",
        "MANGOHUD",
        "MANGOHUD_CONFIG",
        "MANGOHUD_DLSYM",
        "ENABLE_VKBASALT",
        "VK_INSTANCE_LAYERS",
        "VK_LAYER_PATH",
        "VK_ADD_LAYER_PATH",
        "DXVK_HUD",
    ] {
        command.env_remove(variable);
    }
    if env::var_os("DISPLAY").is_some_and(|s| !s.is_empty()) {
        command
            .env_remove("WAYLAND_DISPLAY")
            .env("PROTON_ENABLE_WAYLAND", "0")
            .env("GDK_BACKEND", "x11")
            .env("QT_QPA_PLATFORM", "xcb")
            .env("SDL_VIDEODRIVER", "x11")
            .env("XDG_SESSION_TYPE", "x11");
    } else if env::var("XDG_SESSION_TYPE").is_ok_and(|s| s == "wayland")
        || env::var_os("WAYLAND_DISPLAY").is_some_and(|s| !s.is_empty())
    {
        return Err("Для совместимого запуска Arizona нужен XWayland (DISPLAY). В niri проверьте установку xwayland-satellite; в Hyprland — поддержку XWayland. Либо отключите совместимый запуск в настройках ALOL для использования нативного Wine Wayland.".into());
    }
    Ok(())
}

fn is_game(executable: &str) -> bool {
    Path::new(executable).file_name().is_some_and(|name| {
        ["enb.exe", "gta-sa.exe", "gtasa.exe", "gta_sa.exe"]
            .iter()
            .any(|expected| name.eq_ignore_ascii_case(expected))
    })
}

fn input_environment(command: &mut Command) {
    // Keep overlays/graphics settings. Bypass only inherited input-method hooks,
    // which can consume letter events while arrows continue to reach the game.
    command
        .env("XMODIFIERS", "@im=none")
        .env("GTK_IM_MODULE", "xim")
        .env("QT_IM_MODULE", "xim")
        .env("SDL_IM_MODULE", "none");
    if env::var_os("DISPLAY").is_some_and(|display| !display.is_empty()) {
        command
            .env_remove("WAYLAND_DISPLAY")
            .env("PROTON_ENABLE_WAYLAND", "0")
            .env("SDL_VIDEODRIVER", "x11");
    }
    // No DISPLAY means native Wayland stays available. Never fabricate a socket
    // name, change the compositor's keymap or run global setxkbmap commands.
}

fn ensure_input(s: &Settings) -> Result<()> {
    let marker = s.prefix().join(".alol-input-v1");
    let desired = if s.input_compatibility {
        "enabled"
    } else {
        "disabled"
    };
    let previous = fs::read_to_string(&marker).ok();
    if previous.as_deref() == Some(desired) || (!s.input_compatibility && previous.is_none()) {
        return Ok(());
    }
    let registry = s.prefix().join("user.reg");
    if registry.is_file() {
        let backup =
            data_dir()
                .join("backups/input")
                .join(format!("{}-{}", stamp(), std::process::id()));
        fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
        fs::copy(registry, backup.join("user.reg")).map_err(|e| e.to_string())?;
    }
    // Wine reads per-EXE X11 settings before the shared X11 Driver key. Scope
    // this to game executables, leaving launcher/installer text entry intact.
    let value = if s.input_compatibility { "\"N\"" } else { "-" };
    let mut text = "Windows Registry Editor Version 5.00\r\n".to_string();
    for executable in ["enb.exe", "gta-sa.exe", "gtasa.exe", "gta_sa.exe"] {
        text.push_str(&format!(
            "\r\n[HKEY_CURRENT_USER\\Software\\Wine\\AppDefaults\\{executable}\\X11 Driver]\r\n\"UseXIM\"={value}\r\n"
        ));
    }
    let directory = s.prefix().join("drive_c/alol");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    fs::write(directory.join("input.reg"), bytes).map_err(|e| e.to_string())?;
    run(s, REG, &["import", r"C:\alol\input.reg"])?;
    fs::write(marker, desired).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn available(name: &str) -> bool {
    let executable = |p: &Path| {
        fs::metadata(p)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    };
    if name.contains('/') {
        return executable(Path::new(name));
    }
    env::var_os("PATH")
        .map(|v| env::split_paths(&v).any(|p| executable(&p.join(name))))
        .unwrap_or(false)
}

// Do not follow directory symlinks: Wine's dosdevices/z: points at the host root.
fn walk(path: &Path, depth: usize, visit: &mut impl FnMut(&Path)) {
    if depth == 0 || path.is_symlink() {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        visit(&p);
        if kind.is_dir() {
            walk(&p, depth - 1, visit);
        }
    }
}

pub fn proton_versions() -> Vec<PathBuf> {
    let mut versions = Vec::new();
    for base in [
        xdg("XDG_DATA_HOME", ".local/share").join("Steam/compatibilitytools.d"),
        home().join(".steam/steam/compatibilitytools.d"),
        xdg("XDG_DATA_HOME", ".local/share").join("umu/compatibilitytools"),
        PathBuf::from("/usr/share/steam/compatibilitytools.d"),
    ] {
        if let Ok(entries) = fs::read_dir(base) {
            for e in entries.flatten() {
                if e.path().join("proton").is_file()
                    && let Ok(p) = e.path().canonicalize()
                {
                    versions.push(p);
                }
            }
        }
    }
    versions.sort();
    versions.dedup();
    versions
}

fn pin_proton(s: &Settings) -> Result<()> {
    if s.runner != "proton" || !["GE-Proton", "UMU-Proton"].contains(&s.proton.as_str()) {
        return Ok(());
    }
    // Proton records the actual fonts/lib/default-prefix paths used in config_info.
    // Do not guess from directory mtimes: an older manually extracted build can be newer.
    if let Some(p) = used_proton(s) {
        let mut pinned = Settings::load()?;
        // Do not overwrite concurrent settings edits or a switch to a different family.
        if pinned.runner == s.runner && pinned.proton == s.proton && pinned.prefix() == s.prefix() {
            pinned.proton = p.to_string_lossy().into();
            pinned.save()?;
            println!("Закреплена версия: {}", p.display());
        }
    }
    Ok(())
}

fn used_proton(s: &Settings) -> Option<PathBuf> {
    let info = fs::read_to_string(s.prefix().join("config_info")).ok()?;
    for line in info.lines().filter(|l| l.starts_with('/')) {
        for p in Path::new(line).ancestors() {
            if p.join("proton").is_file()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(&s.proton))
            {
                return p.canonicalize().ok();
            }
        }
    }
    None
}

fn resolved_settings(s: &Settings) -> Settings {
    let mut actual = s.clone();
    if s.runner == "proton"
        && ["GE-Proton", "UMU-Proton"].contains(&s.proton.as_str())
        && let Some(p) = used_proton(s)
    {
        actual.proton = p.to_string_lossy().into();
    }
    actual
}

unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
    fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
}
struct PrefixLock(File);
impl PrefixLock {
    fn acquire(prefix: &Path) -> Result<Self> {
        fs::create_dir_all(prefix).map_err(|e| e.to_string())?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(prefix.join(".alol.lock"))
            .map_err(|e| e.to_string())?;
        // SAFETY: file owns a valid fd; flock does not retain pointers.
        if unsafe { flock(file.as_raw_fd(), 2 | 4) } != 0 {
            return Err("Это окружение уже используется другой операцией ALOL".into());
        }
        Ok(Self(file))
    }
}
impl Drop for PrefixLock {
    fn drop(&mut self) {
        unsafe {
            flock(self.0.as_raw_fd(), 8);
        }
    }
}

fn run(s: &Settings, executable: &str, args: &[&str]) -> Result<()> {
    run_managed(s, executable, args, None)
}

fn run_managed(
    s: &Settings,
    executable: &str,
    args: &[&str],
    watch_game: Option<PathBuf>,
) -> Result<()> {
    if TERMINATED.load(Ordering::Relaxed) {
        return Err("Операция остановлена".into());
    }
    println!(
        "Запуск: {executable} {}\nОкружение: {}",
        crate::game::log_args(args),
        s.prefix().display()
    );
    let mut command = s.command(executable, args)?;
    if TERMINATED.load(Ordering::Relaxed) {
        return Err("Операция остановлена".into());
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Не удалось запустить процесс: {e}"))?;
    let status = if let Some(game) = watch_game {
        let mut next_check = Instant::now();
        let mut reported_error = false;
        loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            if !TERMINATED.load(Ordering::Relaxed) && Instant::now() >= next_check {
                // Updates in the official launcher can introduce new Windows ADS sidecars.
                // Keep checking while it runs, including the initial game download.
                if let Err(e) = backup_zone_identifiers_at(&game) {
                    if !reported_error {
                        eprintln!("Не удалось обработать метки Windows: {e}");
                        reported_error = true;
                    }
                } else {
                    reported_error = false;
                }
                next_check = Instant::now() + Duration::from_secs(2);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    } else {
        child.wait().map_err(|e| e.to_string())?
    };
    if TERMINATED.load(Ordering::Relaxed) {
        return Err("Операция остановлена".into());
    }
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{executable} завершился: {status}. Подробности выше и во вкладке «Журнал»."
        ))
    }
}

pub fn execute(action: &str, args: &[String]) -> Result<()> {
    // Stay alive until the child exits after a group SIGTERM, keeping the prefix locked.
    // exec resets this caught handler to the default disposition in child programs.
    unsafe {
        signal(15, terminate);
    }
    if ![
        "doctor",
        "prepare",
        "install",
        "launch",
        "play",
        "servers-refresh",
        "repair",
        "repair-game",
        "repair-language",
        "mods-refresh",
        "mods-install",
        "mods-add",
        "mods-enable",
        "mods-disable",
        "mods-remove",
        "libraries",
        "reset-game",
        "winecfg",
    ]
    .contains(&action)
    {
        return Err(format!("Неизвестная команда: {action}. См. alol --help"));
    }
    let s = Settings::load()?;
    if matches!(action, "launch" | "play")
        && let Err(error) = crate::desktop::integrate_arizona()
    {
        eprintln!("Иконка окна Arizona: {error}");
    }
    if action == "doctor" {
        println!("{}", doctor(&s));
        return Ok(());
    }
    if action == "mods-refresh" {
        return crate::mods::execute(&s, action, args);
    }
    if action == "servers-refresh" {
        return crate::game::refresh_servers();
    }
    let _lock = PrefixLock::acquire(&s.prefix())?;
    crate::game::cleanup_legacy(&s)?;
    fs::create_dir_all(data_dir().join("logs")).map_err(|e| e.to_string())?;
    println!(
        "ALOL {} · {action} · {}\n{} / {}",
        env!("CARGO_PKG_VERSION"),
        stamp(),
        s.runner,
        s.proton
    );
    match action {
        action if action.starts_with("mods-") => crate::mods::execute(&s, action, args)?,
        "play" => {
            let mut profile = crate::game::PlayerProfile::load(&s)?;
            if let Some(id) = args.first() {
                profile.select(id);
            }
            if let Some(nickname) = args.get(1) {
                profile.set_nickname(nickname.clone());
            }
            let server = profile.validate_launch()?;
            let plan = crate::game::plan(&s, &profile)?;
            profile.save()?;
            let actual = resolved_settings(&s);
            ensure_components(&actual)?;
            ensure_language(&actual, false)?;
            ensure_input(&actual)?;
            backup_zone_identifiers_at(&plan.game_directory)?;
            crate::game::sync_player(&s, &profile)?;
            let options = crate::game_options::GameOptions::load(&s)?;
            options.save()?;
            options.sync(&s)?;
            println!(
                "Подключение: {} · {}:{}",
                server.name, server.host, server.port
            );
            let arguments: Vec<_> = plan.args.iter().map(String::as_str).collect();
            run_managed(
                &actual,
                &plan.executable.to_string_lossy(),
                &arguments,
                Some(plan.game_directory),
            )?;
            pin_proton(&s)?;
            println!("Игра завершена.");
        }
        "prepare" => {
            prepare(&s)?;
            pin_proton(&s)?;
            println!("Окружение готово. Теперь можно установить Arizona.");
        }
        "install" => {
            if !Path::new(&s.installer).is_file() {
                return Err(format!("Установщик не найден: {}", s.installer));
            }
            prepare(&s)?;
            pin_proton(&s)?;
            // Reuse the pinned build immediately, without a second network version lookup.
            let current = resolved_settings(&s);
            run(
                &current,
                &s.installer,
                &["/S", "/D=C:\\Arizona Games Launcher"],
            )?;
            if let Some(exe) = s.find_executable() {
                println!("Установлено: {}", exe.display());
            } else {
                return Err("Установщик завершился, но лаунчер не найден. Укажите путь к .exe в настройках.".into());
            }
        }
        "launch" => {
            let exe = s.find_executable().ok_or(
                "Лаунчер не найден. Сначала установите Arizona или выберите .exe в настройках.",
            )?;
            ensure_components(&resolved_settings(&s))?;
            ensure_language(&resolved_settings(&s), false)?;
            ensure_input(&resolved_settings(&s))?;
            backup_zone_identifiers(&s)?;
            let profile = crate::game::PlayerProfile::load(&s)?;
            crate::game::sync_player(&s, &profile)?;
            let options = crate::game_options::GameOptions::load(&s)?;
            options.save()?;
            options.sync(&s)?;
            // Chromium's Windows sandbox is not implemented by Wine.
            let args = if s.disable_gpu {
                vec!["--no-sandbox", "--disable-gpu"]
            } else {
                vec!["--no-sandbox"]
            };
            let watched = exe.parent().map(|p| p.join("bin/arizona"));
            let result = run_managed(&s, &exe.to_string_lossy(), &args, watched.clone());
            if result.is_err()
                && s.launcher_compatibility
                && !s.disable_gpu
                && !TERMINATED.load(Ordering::Relaxed)
            {
                println!("Официальный лаунчер завершился с ошибкой. Повторный запуск без GPU…");
                run_managed(
                    &s,
                    &exe.to_string_lossy(),
                    &["--no-sandbox", "--disable-gpu"],
                    watched,
                )?;
            } else {
                result?;
            }
            pin_proton(&s)?;
        }
        "repair" => {
            if !s.prefix_ready() {
                return Err("Окружение ещё не создано. Нажмите «Подготовить Proton».".into());
            }
            run(&s, WINEBOOT, &["-u"])?;
            ensure_components(&resolved_settings(&s))?;
            ensure_language(&resolved_settings(&s), false)?;
            ensure_input(&resolved_settings(&s))?;
            backup_zone_identifiers(&s)?;
            let n = backup_caches(&s)?;
            println!(
                "Окружение обновлено. Сохранено папок кэша: {n}.\nДля проверки файлов игры откройте настройки официального лаунчера → восстановление игры."
            );
        }
        "libraries" => ensure_components(&resolved_settings(&s))?,
        "repair-language" => ensure_language(&resolved_settings(&s), true)?,
        "repair-game" => {
            let n = backup_zone_identifiers(&s)?;
            println!("Проверка файлов Linux завершена. Обработано меток Windows: {n}.");
        }
        "reset-game" => {
            let mut paths = Vec::new();
            walk(&s.prefix().join("drive_c/users"), 7, &mut |p| {
                if p.file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("gta_sa.set"))
                {
                    paths.push(p.to_path_buf());
                }
            });
            for p in &paths {
                backup(p)?;
            }
            println!(
                "Сохранено и сброшено файлов настроек GTA: {}. Файлы .alol-backup находятся рядом с оригиналами.",
                paths.len()
            );
        }
        "winecfg" => run(&s, WINECFG, &[])?,
        _ => unreachable!(),
    }
    Ok(())
}

fn prepare(s: &Settings) -> Result<()> {
    // createprefix is UMU's empty-EXE sentinel: it initializes the prefix but
    // returns a Wine launch error. Use a real builtin and require its success.
    run(s, WINEBOOT, &["-u"])?;
    if !s.prefix_ready() {
        return Err("Wine/Proton не создал drive_c и system.reg; проверьте журнал".into());
    }
    // Use wine's own registry tool inside the same runtime. Do not patch registry files.
    let actual = resolved_settings(s);
    run(
        &actual,
        REG,
        &[
            "add",
            "HKCU\\Software\\Wine",
            "/v",
            "Version",
            "/t",
            "REG_SZ",
            "/d",
            "win10",
            "/f",
        ],
    )?;
    ensure_components(&actual)?;
    ensure_language(&actual, false)?;
    ensure_input(&actual)
}

const RUSSIAN_REG: &str = r#"Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\System\CurrentControlSet\Control\Nls\CodePage]
"ACP"="1251"
"OEMCP"="866"
"MACCP"="10007"

[HKEY_LOCAL_MACHINE\System\CurrentControlSet\Control\Nls\Language]
"Default"="0419"
"InstallLanguage"="0419"

[HKEY_CURRENT_USER\Control Panel\International]
"Locale"="00000419"
"LocaleName"="ru-RU"
"sLanguage"="RUS"
"sCountry"="Russia"
"iCountry"="7"
"sShortDate"="dd.MM.yyyy"
"sLongDate"="d MMMM yyyy"
"sTimeFormat"="H:mm:ss"
"iTime"="1"
"sDecimal"="," 
"sThousand"=" "
"sList"=";"
"iMeasure"="0"
"iPaperSize"="9"
"iFirstDayOfWeek"="0"

[HKEY_CURRENT_USER\Control Panel\International\Geo]
"Nation"="203"
"Name"="RU"
"#;

fn ensure_language(s: &Settings, force: bool) -> Result<()> {
    if !s.prefix_ready() {
        return Err("Сначала подготовьте окружение".into());
    }
    let marker = s.prefix().join(".alol-language-ru-v1");
    if !force && marker.is_file() {
        return Ok(());
    }
    let backup =
        data_dir()
            .join("backups/language")
            .join(format!("{}-{}", stamp(), std::process::id()));
    fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
    for name in ["user.reg", "system.reg"] {
        let p = s.prefix().join(name);
        if p.is_file() {
            fs::copy(p, backup.join(name)).map_err(|e| e.to_string())?;
        }
    }
    let directory = s.prefix().join("drive_c/alol");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let mut bytes = vec![0xff, 0xfe];
    for c in RUSSIAN_REG.replace('\n', "\r\n").encode_utf16() {
        bytes.extend_from_slice(&c.to_le_bytes());
    }
    fs::write(directory.join("locale-ru.reg"), bytes).map_err(|e| e.to_string())?;
    run(s, REG, &["import", r"C:\alol\locale-ru.reg"])?;
    fs::write(marker, b"ru-RU / ACP 1251 / OEM 866\n").map_err(|e| e.to_string())?;
    println!(
        "Русский язык игры настроен: Windows-1251, ru-RU. Резервная копия реестра: {}",
        backup.display()
    );
    Ok(())
}

fn missing_components(s: &Settings) -> Vec<&'static str> {
    let log = fs::read_to_string(s.prefix().join("winetricks.log")).unwrap_or_default();
    ["corefonts", "vcrun2022"]
        .into_iter()
        .filter(|verb| !log.lines().any(|line| line.trim() == *verb))
        .collect()
}

fn ensure_components(s: &Settings) -> Result<()> {
    if !s.prefix_ready() {
        return Err("Сначала подготовьте окружение".into());
    }
    let missing = missing_components(s);
    if missing.is_empty() {
        println!("Шрифты и Visual C++ уже установлены.");
        return Ok(());
    }
    println!(
        "Автоматическая установка компонентов: {}",
        missing.join(", ")
    );
    let args: Vec<_> = std::iter::once("-q").chain(missing).collect();
    if s.runner == "proton" {
        run(s, "winetricks", &args)?;
    } else {
        if !available("winetricks") {
            return Err("Для установки компонентов нужен winetricks".into());
        }
        let mut command = Command::new("winetricks");
        crate::language::configure(&mut command);
        let status = command
            .env("WINEPREFIX", s.prefix())
            .env("WINE", &s.wine)
            .env_remove("WINEARCH")
            .args(&args)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("Winetricks завершился: {status}"));
        }
    }
    if TERMINATED.load(Ordering::Relaxed) {
        return Err("Операция остановлена".into());
    }
    let missing = missing_components(s);
    if !missing.is_empty() {
        return Err(format!(
            "Winetricks не подтвердил установку: {}. Повторите подготовку окружения.",
            missing.join(", ")
        ));
    }
    println!("Шрифты и Visual C++ установлены.");
    Ok(())
}

// On Linux, Windows Zone.Identifier alternate streams may be extracted as regular
// files. Arizona enumerates them as fonts/DLLs and opens a libcore error per file.
// Only move recognized small metadata files, never fonts, DLLs, or arbitrary ADS.
fn is_zone_metadata(path: &Path) -> bool {
    if !path.file_name().is_some_and(|n| {
        n.to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(":zone.identifier")
    }) {
        return false;
    }
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() > 16 * 1024 {
        return false;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let text = text.trim_start_matches('\u{feff}');
    text.lines()
        .any(|l| l.trim().eq_ignore_ascii_case("[ZoneTransfer]"))
        && text.lines().any(|l| {
            l.trim()
                .strip_prefix("ZoneId=")
                .and_then(|v| v.trim().parse::<u8>().ok())
                .is_some_and(|n| n <= 4)
        })
}

fn backup_zone_identifiers(s: &Settings) -> Result<usize> {
    let Some(exe) = s.find_executable() else {
        return Ok(0);
    };
    let Some(launcher) = exe.parent() else {
        return Ok(0);
    };
    backup_zone_identifiers_at(&launcher.join("bin/arizona"))
}

fn backup_zone_identifiers_at(game: &Path) -> Result<usize> {
    let mut files = Vec::new();
    walk(game, 16, &mut |p| {
        if is_zone_metadata(p) {
            files.push(p.to_path_buf());
        }
    });
    if files.is_empty() {
        return Ok(0);
    }
    let destination = data_dir().join("backups/zone-identifiers").join(format!(
        "{}-{}",
        stamp(),
        std::process::id()
    ));
    fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
    println!("Сохранение меток Windows: {}", destination.display());
    for source in &files {
        let relative = source.strip_prefix(game).map_err(|e| e.to_string())?;
        let target = destination.join(relative);
        fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        // Copy first so backups work even when game and data live on different disks.
        fs::copy(source, &target).map_err(|e| format!("Не сохранить {}: {e}", source.display()))?;
        fs::remove_file(source).map_err(|e| {
            format!(
                "Копия сохранена, но не удалось убрать {}: {e}",
                source.display()
            )
        })?;
    }
    println!(
        "Убрано служебных меток Windows: {}. Настоящие шрифты и DLL сохранены.",
        files.len()
    );
    Ok(files.len())
}

fn backup(p: &Path) -> Result<()> {
    let name = p.file_name().ok_or("Нет имени файла")?.to_string_lossy();
    let dst = p.with_file_name(format!("{name}.alol-backup-{}", stamp()));
    if dst.exists() {
        return Err(format!("Резервная копия уже существует: {}", dst.display()));
    }
    fs::rename(p, &dst).map_err(|e| format!("Не сохранить {}: {e}", p.display()))?;
    println!("Сохранено: {}", dst.display());
    Ok(())
}

fn backup_caches(s: &Settings) -> Result<usize> {
    let mut candidates = Vec::new();
    walk(&s.prefix().join("drive_c/users"), 7, &mut |p| {
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        if p.is_dir() && name.eq_ignore_ascii_case("arizona-launcher") {
            for cache in ["GPUCache", "Code Cache", "Cache"] {
                if p.join(cache).is_dir() && !p.join(cache).is_symlink() {
                    candidates.push(p.join(cache));
                }
            }
        }
    });
    for p in &candidates {
        backup(p)?;
    }
    Ok(candidates.len())
}

pub fn doctor(s: &Settings) -> String {
    let yes = |b| if b { "✓" } else { "✗" };
    let versions = proton_versions()
        .iter()
        .map(|p| format!("  {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = format!(
        "ALOL {} · диагностика\n\n{} umu-run\n{} Wine ({})\n{} Установщик: {}\n{} Окружение: {}\n{} Официальный лаунчер: {}\n\nВыбрано: {} / {}\nГрафика: {}\nСессия: {}\nНастройки: {}\nЖурнал: {}\n\nУстановленные сборки Proton:\n{}\n",
        env!("CARGO_PKG_VERSION"),
        yes(available("umu-run")),
        yes(available(&s.wine)),
        s.wine,
        yes(Path::new(&s.installer).is_file()),
        s.installer,
        yes(s.prefix_ready()),
        s.prefix().display(),
        yes(s.find_executable().is_some()),
        s.find_executable()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "не найден".into()),
        s.runner,
        s.proton,
        if s.wined3d {
            "WineD3D / OpenGL"
        } else {
            "DXVK / Vulkan (Proton)"
        },
        env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "неизвестно".into()),
        config_file().display(),
        log_file().display(),
        if versions.is_empty() {
            "  Нет. UMU загрузит сборку при подготовке."
        } else {
            &versions
        }
    );
    for (cmd, args) in [
        ("umu-run", vec!["--version"]),
        ("lspci", vec![]),
        ("df", vec!["-h", &s.prefix_root]),
    ] {
        if let Ok(output) = Command::new(cmd).args(args).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            let filtered = if cmd == "lspci" {
                text.lines()
                    .filter(|l| {
                        ["VGA", "Display", "3D controller"]
                            .iter()
                            .any(|v| l.contains(v))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                text.trim().into()
            };
            if !filtered.is_empty() {
                out.push_str(&format!("\n{filtered}\n"));
            }
        }
    }
    out.push_str(&format!(
        "\nШрифты / Visual C++: {}\n",
        if missing_components(s).is_empty() {
            "установлены".into()
        } else {
            format!(
                "при подготовке будут установлены {}",
                missing_components(s).join(", ")
            )
        }
    ));
    out.push_str("\nGE-Proton запускается через UMU; первый запуск требует интернета и нескольких ГБ.\nПрефиксы Wine и Proton раздельные. Проверка файлов игры — в официальном лаунчере.\nПосле успешной подготовки сборка Proton закрепляется. Для обновления выберите GE-Proton снова.\nЕсли лаунчер пустой: включите «Отключить GPU в лаунчере».\nЕсли игра не отображается: попробуйте WineD3D.\n");
    out.push_str(&format!(
        "\nСовместимый запуск лаунчера: {}\nXWayland / DISPLAY: {}\nПроверка обновлений: {}\n",
        s.launcher_compatibility,
        if env::var_os("DISPLAY").is_some_and(|s| !s.is_empty()) {
            "доступен"
        } else {
            "не задан"
        },
        s.update_policy
    ));
    out.push_str(&format!(
        "\nЛокаль: {}\nСовместимость клавиатуры: {}\n",
        crate::language::description(),
        s.input_compatibility
    ));
    out
}

pub fn stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn open_log() -> Result<File> {
    let path = log_file();
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    if path.metadata().is_ok_and(|m| m.len() > 4 * 1024 * 1024) {
        fs::rename(&path, path.with_extension("previous.log")).map_err(|e| e.to_string())?;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    writeln!(f, "\n──────── ALOL · {} ────────", stamp()).map_err(|e| e.to_string())?;
    Ok(f)
}

pub fn tail_log() -> String {
    let Ok(mut f) = File::open(log_file()) else {
        return "Здесь появится журнал операций.".into();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(48 * 1024)));
    let mut bytes = Vec::new();
    let _ = f.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).replace('\0', "")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_preferences_migrate_and_validate() {
        let mut settings = Settings::default();
        assert_eq!(
            settings.parse("runner=wine\n").unwrap().update_policy,
            "startup"
        );
        assert!(settings.update_notifications);
        assert!(settings.input_compatibility);
        let saved = settings
            .parse(
                "update_policy=manual\nupdate_notifications=false\nlauncher_compatibility=false\ninput_compatibility=false\n",
            )
            .unwrap();
        assert_eq!(saved.update_policy, "manual");
        assert!(!saved.update_notifications);
        assert!(!saved.launcher_compatibility);
        assert!(!saved.input_compatibility);
        assert!(settings.parse("update_policy=never-ever\n").is_err());
    }
    fn temp() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = env::temp_dir().join(format!(
            "alol-test-{}-{}-{}",
            std::process::id(),
            stamp(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn rejects_unsafe_or_ambiguous_config() {
        let mut s = Settings::default();
        assert!(s.parse("prefix_root=/tmp/../home\n").is_err());
        s = Settings::default();
        assert!(s.parse("proton=some shell command\n").is_err());
        s = Settings::default();
        s.executable = "/tmp/a\nrunner=wine".into();
        assert!(s.validate().is_err());
    }
    #[test]
    fn paths_with_spaces_and_equals_roundtrip() {
        let mut s = Settings::default();
        s.parse("installer=/tmp/Arizona Games/a=b.exe\nrunner=wine\nwined3d=true\n")
            .unwrap();
        assert_eq!(s.installer, "/tmp/Arizona Games/a=b.exe");
        assert!(s.wined3d);
        assert!(s.prefix().ends_with("wine"));
    }
    #[test]
    fn discovery_skips_symlinks_and_uninstaller() {
        let root = temp();
        let mut s = Settings {
            prefix_root: root.to_string_lossy().into(),
            ..Settings::default()
        };
        let dir = s.prefix().join("drive_c/Arizona Games Launcher");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Uninstall Arizona.exe"), []).unwrap();
        let outside = root.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("ArizonaLauncher.exe"), []).unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("link")).unwrap();
        assert!(s.find_executable().is_none());
        let exe = dir.join("Arizona Games Launcher.exe");
        fs::write(&exe, []).unwrap();
        assert_eq!(s.find_executable(), Some(exe));
        s.executable = root.join("missing.exe").to_string_lossy().into();
        assert!(s.find_executable().is_none());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn prefix_lock_is_exclusive_and_released() {
        let root = temp();
        let lock = PrefixLock::acquire(&root).unwrap();
        assert!(PrefixLock::acquire(&root).is_err());
        drop(lock);
        assert!(PrefixLock::acquire(&root).is_ok());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn repair_keeps_settings_and_other_app_caches() {
        let root = temp();
        let s = Settings {
            prefix_root: root.to_string_lossy().into(),
            ..Settings::default()
        };
        let app = s
            .prefix()
            .join("drive_c/users/steamuser/AppData/Roaming/arizona-launcher");
        fs::create_dir_all(app.join("GPUCache")).unwrap();
        fs::write(app.join("settings.json"), b"keep").unwrap();
        let other = app.parent().unwrap().join("OtherApp/GPUCache");
        fs::create_dir_all(&other).unwrap();
        assert_eq!(backup_caches(&s).unwrap(), 1);
        assert!(!app.join("GPUCache").exists());
        assert_eq!(fs::read(app.join("settings.json")).unwrap(), b"keep");
        assert!(other.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
