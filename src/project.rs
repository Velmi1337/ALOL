//! Games supported by the shared official launcher; identifiers are never user paths.
use crate::core::{self, Result};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Project {
    #[default]
    Arizona,
    Rodina,
}

impl Project {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "arizona" => Ok(Self::Arizona),
            "rodina" => Ok(Self::Rodina),
            _ => Err("Выберите игру: arizona или rodina".into()),
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Arizona => "arizona",
            Self::Rodina => "rodina",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Arizona => "Arizona RP",
            Self::Rodina => "Родина RP",
        }
    }
    pub fn website(self) -> &'static str {
        match self {
            Self::Arizona => "https://arizona-rp.com/",
            Self::Rodina => "https://rodina-rp.com/",
        }
    }
    pub fn launch_flag(self) -> &'static str {
        match self {
            Self::Arizona => "-arizona",
            Self::Rodina => "-rodina",
        }
    }
    pub fn config(self, filename: &str) -> PathBuf {
        let root = core::config_file().parent().unwrap().to_path_buf();
        if self == Self::Arizona {
            root.join(filename)
        } else {
            root.join("projects").join(self.id()).join(filename)
        }
    }
    pub fn cache(self, directory: &str, filename: &str) -> PathBuf {
        let root = core::data_dir().join(directory);
        if self == Self::Arizona {
            root.join(filename)
        } else {
            root.join(self.id()).join(filename)
        }
    }
    pub fn mod_cdns(self) -> [&'static str; 2] {
        match self {
            Self::Arizona => [
                "https://pc.az-ins.com/resource/mods",
                "https://arz-pc.react-group.tech/resource/mods",
            ],
            Self::Rodina => [
                "https://pc.rod-ins.com/resource/mods",
                "https://rod-pc.react-group.tech/resource/mods",
            ],
        }
    }
}
