use crate::{
    core::{self, Result, Settings},
    game,
    json::{self, Value, object},
    mods,
};
use std::{collections::BTreeMap, fs, path::PathBuf};

pub struct OptionSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub tooltip: &'static str,
    pub flag: &'static str,
    pub default: bool,
}
pub const OPTIONS: [OptionSpec; 10] = [
    OptionSpec {
        id: "wideScreen",
        label: "Широкий экран",
        tooltip: "Расширяет угол обзора и адаптирует интерфейс под широкий монитор.",
        flag: "-widescreen",
        default: true,
    },
    OptionSpec {
        id: "windowed",
        label: "Запуск в окне",
        tooltip: "Изменяемое окно вместо полноэкранного масштабирования.",
        flag: "",
        default: false,
    },
    OptionSpec {
        id: "preload",
        label: "Предзагрузка моделей",
        tooltip: "Снижает нагрузку на диск; может вызывать подлагивания при движении.",
        flag: "-ldo",
        default: false,
    },
    OptionSpec {
        id: "shitPc",
        label: "Режим слабого ПК",
        tooltip: "Уменьшает качество текстур и отключает часть эффектов.",
        flag: "-t",
        default: false,
    },
    OptionSpec {
        id: "newGraphics",
        label: "Новые текстуры",
        tooltip: "Новые текстуры растительности, модели оружия и другие ресурсы.",
        flag: "-trees_new",
        default: true,
    },
    OptionSpec {
        id: "grass",
        label: "Дополнительная растительность",
        tooltip: "Повышает детализацию травы и листвы.",
        flag: "-enable_grass",
        default: true,
    },
    OptionSpec {
        id: "oldResolution",
        label: "Устаревшие разрешения (16 бит)",
        tooltip: "Используйте только для оборудования без поддержки 32-битного режима экрана.",
        flag: "-16bpp",
        default: false,
    },
    OptionSpec {
        id: "hdrResolution",
        label: "Вывод в HDR",
        tooltip: "Требуется HDR-монитор и поддержка HDR в графическом окружении.",
        flag: "-allow_hdr",
        default: false,
    },
    OptionSpec {
        id: "authCef",
        label: "Новая авторизация CEF",
        tooltip: "Обновлённый интерфейс авторизации Arizona.",
        flag: "-auth_cef_enable",
        default: true,
    },
    OptionSpec {
        id: "cefDirtyRects",
        label: "Оптимизация интерфейсов CEF",
        tooltip: "Включает режим частичного обновления интерфейсов.",
        flag: "-cef_dirty_rects",
        default: false,
    },
];
#[derive(Clone, Debug)]
pub struct GameOptions {
    pub memory: u32,
    pub values: BTreeMap<String, bool>,
}
pub fn file() -> PathBuf {
    core::config_file().with_file_name("game-options.json")
}
impl GameOptions {
    pub fn load(s: &Settings) -> Result<Self> {
        match fs::read_to_string(file()) {
            Ok(text) => {
                let v = json::parse(&text)?;
                let memory = v
                    .get("memory")
                    .text()
                    .parse::<u32>()
                    .map_err(|_| "Некорректная память игры")?;
                let values = OPTIONS
                    .iter()
                    .map(|o| (o.id.into(), v.get("options").get(o.id).boolean()))
                    .collect();
                let result = Self { memory, values };
                result.validate()?;
                Ok(result)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let native = game::official(s)?;
                let a = native.get("arizona");
                let present = !matches!(a, Value::Null);
                let native_options: BTreeMap<_, _> = a
                    .get("options")
                    .array()
                    .iter()
                    .map(|o| (o.get("id").text(), o.get("value").boolean()))
                    .collect();
                let memory = a
                    .get("memory")
                    .get("value")
                    .text()
                    .parse::<u32>()
                    .ok()
                    .filter(|v| [1024, 2048, 3072, 4096].contains(v))
                    .unwrap_or(2048);
                let values = OPTIONS
                    .iter()
                    .map(|o| {
                        (
                            o.id.into(),
                            native_options.get(o.id).copied().unwrap_or(if present {
                                false
                            } else {
                                o.default
                            }),
                        )
                    })
                    .collect();
                Ok(Self { memory, values })
            }
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn enabled(&self, key: &str) -> bool {
        self.values.get(key).copied().unwrap_or(false)
    }
    fn validate(&self) -> Result<()> {
        if ![1024, 2048, 3072, 4096].contains(&self.memory) {
            return Err("Память игры: выберите 1024, 2048, 3072 или 4096 МБ".into());
        }
        Ok(())
    }
    pub fn save(&self) -> Result<()> {
        self.validate()?;
        let options = Value::Object(
            self.values
                .iter()
                .map(|(k, v)| (k.clone(), Value::Bool(*v)))
                .collect(),
        );
        let v = object([
            ("memory", Value::Number(self.memory.to_string())),
            ("options", options),
        ]);
        mods::atomic_bytes(&file(), v.dump().as_bytes())
    }
    pub fn sync(&self, s: &Settings) -> Result<()> {
        self.validate()?;
        let Some(path) = mods::official_settings(s) else {
            return Ok(());
        };
        let mut value = game::official(s)?;
        let mut a = value.get("arizona").clone();
        let mut memory = a.get("memory").clone();
        memory.put("value", Value::Number(self.memory.to_string()));
        a.put("memory", memory);
        let mut options = a.get("options").array().to_vec();
        for (id, enabled) in self.values.iter().map(|(k, v)| (k.as_str(), *v)) {
            if let Some(option) = options.iter_mut().find(|o| o.get("id").text() == id) {
                option.put("value", Value::Bool(enabled));
            } else {
                options.push(object([
                    ("id", json::string(id)),
                    ("value", Value::Bool(enabled)),
                ]));
            }
        }
        a.put("options", Value::Array(options));
        value.put("arizona", a);
        mods::atomic_bytes(&path, value.dump().as_bytes())
    }
}
