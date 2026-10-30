//! Direct Arizona RP launch, official server catalogue and per-server nicknames.
use crate::{
    core::{self, Result, Settings},
    json::{self, Value, object, string},
    mods,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::IpAddr,
    path::PathBuf,
};

const SERVER_URLS: [&str; 2] = [
    "https://api.arizona-five.com/launcher/servers",
    "https://arz-api-reserve.s-amp.com/launcher/servers",
];
#[derive(Clone, Debug)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub online: Option<u32>,
    pub capacity: Option<u32>,
}
impl Server {
    pub fn label(&self) -> String {
        let population = match (self.online, self.capacity) {
            (Some(n), Some(max)) => format!(" · {n}/{max}"),
            _ => String::new(),
        };
        format!("№{} · {}{}", self.id, self.name, population)
    }
}
pub fn valid_host(host: &str) -> bool {
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }
    let h = host.trim_end_matches('.');
    !h.is_empty()
        && h.len() <= 253
        && h.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
pub fn validate_nickname(nick: &str) -> Result<()> {
    if !(3..=20).contains(&nick.len())
        || !nick.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(
            "Ник должен содержать 3–20 английских букв, цифр или символов _. Например: Nick_Name"
                .into(),
        );
    }
    Ok(())
}
fn server_cache() -> PathBuf {
    core::data_dir().join("servers/catalog.json")
}
pub fn player_file() -> PathBuf {
    core::config_file().with_file_name("player.json")
}
/// Remove only files owned by the discontinued 0.5.0 password helper.
/// Called with the prefix lock held; keep Moonloader and unrelated mods intact.
pub fn cleanup_legacy(s: &Settings) -> Result<()> {
    if player_file().is_file() {
        PlayerProfile::load(s)?;
    }
    let mut removed = 0;
    if let Ok(root) = mods::game_dir(s) {
        let directory = root.join("moonloader");
        let path = directory.join("alol_autopassword.lua");
        if directory
            .symlink_metadata()
            .is_ok_and(|m| !m.file_type().is_symlink())
            && path
                .symlink_metadata()
                .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink() && m.len() < 64 * 1024)
            && fs::read_to_string(&path)
                .is_ok_and(|text| text.starts_with("-- ALOL owned autopassword helper v1"))
        {
            fs::remove_file(path).map_err(|e| e.to_string())?;
            removed += 1;
        }
    }
    let directory = s.prefix().join("drive_c/alol");
    if directory
        .symlink_metadata()
        .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
    {
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(token) = name
                .strip_prefix("login-")
                .and_then(|n| n.strip_suffix(".bin"))
                && token.len() == 48
                && token.bytes().all(|c| c.is_ascii_hexdigit())
                && entry.file_type().is_ok_and(|t| t.is_file())
            {
                fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
                removed += 1;
            }
        }
    }
    if removed > 0 {
        println!("Удалены служебные файлы старой версии: {removed}");
    }
    Ok(())
}
fn parse_servers(v: &Value) -> Result<Vec<Server>> {
    let a = match v {
        Value::Array(a) => a.as_slice(),
        _ => v.get("arizona").array(),
    };
    let mut out = Vec::new();
    let mut ids = BTreeSet::new();
    for s in a {
        let id = if matches!(s.get("number"), Value::Null) {
            s.get("id").text()
        } else {
            s.get("number").text()
        };
        let number = id
            .parse::<u32>()
            .map_err(|_| "Некорректный ID сервера Arizona")?;
        let id = number.to_string();
        if number == 0 || !ids.insert(id.clone()) {
            return Err("Некорректный или повторный ID сервера Arizona".into());
        }
        let host = s.get("ip").text();
        if !valid_host(&host) {
            return Err("Некорректный адрес в списке серверов".into());
        }
        let port = s
            .get("port")
            .text()
            .parse::<u16>()
            .map_err(|_| "Некорректный порт сервера")?;
        if port == 0 {
            return Err("Порт сервера не может быть 0".into());
        }
        let name = s
            .get("name")
            .text()
            .chars()
            .filter(|c| !c.is_control())
            .take(80)
            .collect::<String>();
        out.push(Server {
            id,
            name,
            host,
            port,
            online: s.get("online").text().parse().ok(),
            capacity: s.get("maxplayers").text().parse().ok(),
        });
    }
    if out.is_empty() {
        return Err("Список серверов Arizona пуст".into());
    }
    out.sort_by_key(|s| s.id.parse::<u32>().unwrap_or(0));
    Ok(out)
}
pub fn servers() -> Result<Vec<Server>> {
    if let Ok(bytes) = fs::read_to_string(server_cache())
        && let Ok(v) = json::parse(&bytes)
        && let Ok(servers) = parse_servers(&v)
    {
        return Ok(servers);
    }
    parse_servers(&json::parse(include_str!("../assets/servers.json"))?)
}
pub fn refresh_servers() -> Result<()> {
    let directory = core::data_dir().join("servers");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let temporary = directory.join(format!(
        "download-{}-{}.json",
        std::process::id(),
        core::stamp()
    ));
    let mut last = String::new();
    for url in SERVER_URLS {
        let result = (|| {
            mods::fetch(url, &temporary, 8 * 1024 * 1024)?;
            let bytes = fs::read_to_string(&temporary).map_err(|e| e.to_string())?;
            let value = json::parse(&bytes)?;
            let records = parse_servers(&value)?;
            mods::atomic_bytes(&server_cache(), value.dump().as_bytes())?;
            println!("Список серверов обновлён: {}", records.len());
            Ok(())
        })();
        let _ = fs::remove_file(&temporary);
        match result {
            Ok(()) => return Ok(()),
            Err(e) => {
                println!("{e}");
                last = e;
            }
        }
    }
    Err(last)
}
pub fn official(s: &Settings) -> Result<Value> {
    match mods::official_settings(s) {
        Some(p) => {
            let bytes = fs::read_to_string(p).map_err(|e| e.to_string())?;
            json::parse(&bytes)
        }
        None => Ok(object([])),
    }
}

#[derive(Clone)]
pub struct PlayerProfile {
    pub server_id: String,
    pub nickname: String,
    pub nicknames: BTreeMap<String, String>,
}
impl PlayerProfile {
    pub fn load(s: &Settings) -> Result<Self> {
        match fs::read_to_string(player_file()) {
            Ok(text) => {
                let v = json::parse(&text)?;
                let mut names = BTreeMap::new();
                if let Value::Object(o) = v.get("nicknames") {
                    for (id, nick) in o {
                        names.insert(id.clone(), nick.text());
                    }
                }
                let profile = Self {
                    server_id: v.get("server_id").text(),
                    nickname: v.get("nickname").text(),
                    nicknames: names,
                };
                profile.validate_draft()?;
                // Upgrade 0.5.0 profiles without retaining any credentials.
                if !matches!(v.get("logins"), Value::Null) {
                    profile.save()?;
                }
                Ok(profile)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let native = official(s)?;
                let game = native.get("arizona");
                let id = game.get("lastSelectedServerId").text();
                let id = if id.parse::<u32>().is_ok_and(|id| id > 0) {
                    id
                } else {
                    "1".into()
                };
                let nick = game.get("userName").text();
                let mut names = BTreeMap::new();
                names.insert(id.clone(), nick.clone());
                let profile = Self {
                    server_id: id,
                    nickname: nick,
                    nicknames: names,
                };
                Ok(profile)
            }
            Err(e) => Err(format!("Не прочитать профиль игрока: {e}")),
        }
    }
    fn validate_draft(&self) -> Result<()> {
        if self.server_id.parse::<u32>().is_err()
            || self.nickname.chars().count() > 128
            || self.nickname.chars().any(char::is_control)
            || self.nicknames.len() > 1000
            || self.nicknames.iter().any(|(k, v)| {
                k.parse::<u32>().is_err()
                    || v.chars().count() > 128
                    || v.chars().any(char::is_control)
            })
        {
            return Err("Повреждён профиль игрока ALOL".into());
        }
        Ok(())
    }
    pub fn set_nickname(&mut self, nick: String) {
        self.nickname = nick;
        self.nicknames
            .insert(self.server_id.clone(), self.nickname.clone());
    }
    pub fn select(&mut self, id: &str) {
        self.nicknames
            .insert(self.server_id.clone(), self.nickname.clone());
        self.server_id = id.into();
        if let Some(nick) = self.nicknames.get(id) {
            self.nickname = nick.clone();
        }
        self.nicknames.insert(id.into(), self.nickname.clone());
    }
    pub fn save(&self) -> Result<()> {
        self.validate_draft()?;
        let names = Value::Object(
            self.nicknames
                .iter()
                .map(|(k, v)| (k.clone(), string(v)))
                .collect(),
        );
        let value = object([
            ("server_id", string(&self.server_id)),
            ("nickname", string(&self.nickname)),
            ("nicknames", names),
        ]);
        mods::atomic_bytes(&player_file(), value.dump().as_bytes())
    }
    pub fn server(&self) -> Result<Server> {
        servers()?
            .into_iter()
            .find(|s| s.id == self.server_id)
            .ok_or("Выбранный сервер не найден. Нажмите «Обновить серверы».".into())
    }
    pub fn validate_launch(&self) -> Result<Server> {
        validate_nickname(&self.nickname)?;
        let server = self.server()?;
        if server.capacity == Some(0) {
            return Err("Сервер временно недоступен. Выберите другой сервер.".into());
        }
        Ok(server)
    }
}
pub fn sync_player(s: &Settings, p: &PlayerProfile) -> Result<()> {
    let Some(path) = mods::official_settings(s) else {
        return Ok(());
    };
    let mut value = official(s)?;
    let mut game = value.get("arizona").clone();
    if game.get("userName").text() == p.nickname
        && game.get("lastSelectedServerId").text() == p.server_id
    {
        return Ok(());
    }
    game.put("userName", string(&p.nickname));
    let id = p
        .server_id
        .parse::<u32>()
        .map_err(|_| "Неверный ID сервера")?
        .to_string();
    game.put("lastSelectedServerId", Value::Number(id));
    value.put("arizona", game);
    mods::atomic_bytes(&path, value.dump().as_bytes())
}
pub struct LaunchPlan {
    pub executable: PathBuf,
    pub game_directory: PathBuf,
    pub args: Vec<String>,
}
pub fn game_executable(s: &Settings) -> Result<PathBuf> {
    let root = mods::game_dir(s)?;
    for name in ["enb.exe", "gta-sa.exe", "gtasa.exe", "gta_sa.exe"] {
        let p = root.join(name);
        if p.is_file() {
            return Ok(p);
        }
    }
    Err("Игровой EXE не найден. Откройте «Обновить игру» и загрузите файлы Arizona.".into())
}
pub fn plan(s: &Settings, profile: &PlayerProfile) -> Result<LaunchPlan> {
    let server = profile.validate_launch()?;
    let executable = game_executable(s)?;
    let root = executable.parent().ok_or("Нет папки игры")?.to_path_buf();
    let value = official(s)?;
    let options = crate::game_options::GameOptions::load(s)?;
    let mut args = vec![
        "-c".into(),
        "-h".into(),
        server.host,
        "-p".into(),
        server.port.to_string(),
        "-n".into(),
        profile.nickname.clone(),
        "-mem".into(),
        options.memory.to_string(),
        "-window".into(),
        "-arizona".into(),
    ];
    if !options.enabled("windowed") {
        args.push("-modern_scale".into());
    }
    for option in crate::game_options::OPTIONS {
        if !option.flag.is_empty() && options.enabled(option.id) {
            args.push(option.flag.into());
        }
    }
    // Reuse the identifier already created by Arizona; never invent or log one.
    let fingerprint = value.get("fprint").text();
    if !fingerprint.is_empty()
        && fingerprint.len() <= 256
        && !fingerprint.chars().any(char::is_control)
    {
        args.extend(["-userId".into(), fingerprint]);
    }
    if let Value::Object(mods) = value.get("installedMods").get("arizona") {
        for m in mods.values() {
            let flag = m.get("flag").text();
            if flag.is_empty() {
                continue;
            }
            let tokens: Vec<_> = flag.split_whitespace().collect();
            if tokens.len() == 1
                && flag.starts_with('-')
                && flag.len() <= 80
                && flag
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                && !["-c", "-h", "-p", "-n", "-mem", "-userId", "-cdn", "-z"]
                    .iter()
                    .any(|reserved| reserved.eq_ignore_ascii_case(&flag))
                && !["-x", "-window", "-modern_scale", "-arizona"].contains(&flag.as_str())
                && !crate::game_options::OPTIONS.iter().any(|o| o.flag == flag)
                && !args.contains(&flag)
            {
                args.push(flag);
            }
        }
    }
    args.extend(["-cdn".into(), "0,0,0".into()]);
    Ok(LaunchPlan {
        executable,
        game_directory: root,
        args,
    })
}
pub fn log_args(args: &[&str]) -> String {
    let mut hidden = false;
    args.iter()
        .map(|a| {
            if hidden {
                hidden = false;
                return "[скрыто]".to_string();
            }
            if ["-userId", "-z", "-old_n"].contains(a) {
                hidden = true;
            }
            a.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_servers_and_addresses() {
        let all =
            parse_servers(&json::parse(include_str!("../assets/servers.json")).unwrap()).unwrap();
        assert!(!all.is_empty());
        assert!(all.iter().all(|s| valid_host(&s.host) && s.port > 0));
    }
    #[test]
    fn nickname_validation() {
        assert!(validate_nickname("Nick_Name12").is_ok());
        for n in ["AB", "Русский_Ник", "Nick Name", "Nick;touch", "Nick\nName"] {
            assert!(validate_nickname(n).is_err());
        }
    }
    #[test]
    fn redact_identity() {
        let s = log_args(&["-n", "Nick_Name", "-userId", "private-id", "-cdn", "0,0,0"]);
        assert!(!s.contains("private-id"));
        assert!(s.contains("Nick_Name"));
    }
    #[test]
    fn nicknames_are_kept_per_server() {
        let mut p = PlayerProfile {
            server_id: "1".into(),
            nickname: "First_Name".into(),
            nicknames: BTreeMap::new(),
        };
        p.select("2");
        p.set_nickname("Second_Name".into());
        p.select("1");
        assert_eq!(p.nickname, "First_Name");
        p.select("2");
        assert_eq!(p.nickname, "Second_Name");
    }
    #[test]
    fn api_servers_validate_and_preserve_population() {
        let v=json::parse(r#"{"arizona":[{"number":2,"name":"Tucson","ip":"127.0.0.1","port":7777,"online":12,"maxplayers":1000}]}"#).unwrap();
        let a = parse_servers(&v).unwrap();
        assert_eq!(a[0].online, Some(12));
        assert!(a[0].label().contains("12/1000"));
    }
}
