//! Observe directory changes without repeatedly walking an idle game tree.
//! A lost/invalid watch is an error so the caller falls back to periodic scans.
use std::{
    collections::HashMap,
    ffi::{CString, OsStr, c_char, c_int},
    fs::{self, File},
    io::{self, Read},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::OsStrExt,
    },
    path::{Component, Path, PathBuf},
};

const CLOSE_WRITE: u32 = 0x0000_0008;
const MOVED_TO: u32 = 0x0000_0080;
const CREATE: u32 = 0x0000_0100;
const DELETE_SELF: u32 = 0x0000_0400;
const MOVE_SELF: u32 = 0x0000_0800;
const UNMOUNT: u32 = 0x0000_2000;
const OVERFLOW: u32 = 0x0000_4000;
const IGNORED: u32 = 0x0000_8000;
const IS_DIRECTORY: u32 = 0x4000_0000;
const ONLY_DIRECTORY: u32 = 0x0100_0000;
const NO_FOLLOW: u32 = 0x0200_0000;

unsafe extern "C" {
    fn inotify_init1(flags: c_int) -> c_int;
    fn inotify_add_watch(fd: c_int, path: *const c_char, mask: u32) -> c_int;
}

pub struct ZoneWatch {
    file: File,
    root: PathBuf,
    directories: HashMap<c_int, PathBuf>,
}

impl ZoneWatch {
    pub fn new(root: &Path) -> io::Result<Self> {
        if !root.is_absolute() || root.components().any(|c| c == Component::ParentDir) {
            return Err(io::Error::other("Unsupported watch path"));
        }
        // Missing game directories are normal during the initial download.
        // Observe only their nearest existing ancestor, then follow new children.
        let anchor = root
            .ancestors()
            .find(|p| p.is_dir())
            .ok_or_else(|| io::Error::other("No watch directory"))?;
        let anchor_real = anchor.canonicalize()?;
        let root = anchor_real.join(root.strip_prefix(anchor).map_err(io::Error::other)?);
        // Linux O_NONBLOCK | O_CLOEXEC: the descriptor never leaks to Wine.
        let fd = unsafe { inotify_init1(0x800 | 0x80000) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut result = Self {
            file: unsafe { File::from_raw_fd(fd) },
            root,
            directories: HashMap::new(),
        };
        result.add_tree(&anchor_real)?;
        Ok(result)
    }

    fn relevant(&self, path: &Path) -> bool {
        path.starts_with(&self.root) || self.root.starts_with(path)
    }

    fn add_tree(&mut self, base: &Path) -> io::Result<()> {
        let mut pending = vec![base.to_path_buf()];
        while let Some(path) = pending.pop() {
            if !self.relevant(&path) {
                continue;
            }
            // Match the existing metadata cleaner's traversal limit.
            if path
                .strip_prefix(&self.root)
                .is_ok_and(|p| p.components().count() >= 16)
            {
                continue;
            }
            let kind = match fs::symlink_metadata(&path) {
                Ok(kind) => kind,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };
            if kind.is_symlink() || !kind.is_dir() {
                continue;
            }
            if self.directories.len() >= 65536 {
                return Err(io::Error::other("Too many watched directories"));
            }
            let name = CString::new(path.as_os_str().as_bytes()).map_err(io::Error::other)?;
            let mask = CLOSE_WRITE
                | MOVED_TO
                | CREATE
                | DELETE_SELF
                | MOVE_SELF
                | ONLY_DIRECTORY
                | NO_FOLLOW;
            let wd = unsafe { inotify_add_watch(self.file.as_raw_fd(), name.as_ptr(), mask) };
            if wd < 0 {
                return Err(io::Error::last_os_error());
            }
            self.directories.insert(wd, path.clone());
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    pending.push(entry.path());
                }
            }
        }
        Ok(())
    }

    fn event(&mut self, wd: c_int, mask: u32, name: &OsStr) -> io::Result<bool> {
        if mask & (OVERFLOW | UNMOUNT | DELETE_SELF | MOVE_SELF | IGNORED) != 0 {
            return Err(io::Error::other(
                "Directory watch changed or event queue overflowed",
            ));
        }
        let Some(parent) = self.directories.get(&wd) else {
            return Ok(false);
        };
        let path = parent.join(name);
        if !self.relevant(&path) {
            return Ok(false);
        }
        if mask & IS_DIRECTORY != 0 && mask & (CREATE | MOVED_TO) != 0 {
            // A whole populated tree can be moved in before we get this event.
            self.add_tree(&path)?;
            return Ok(true);
        }
        Ok(mask & (CLOSE_WRITE | MOVED_TO) != 0
            && name
                .as_bytes()
                .to_ascii_lowercase()
                .ends_with(b":zone.identifier"))
    }

    pub fn changed(&mut self) -> io::Result<bool> {
        let mut dirty = false;
        let mut buffer = [0u8; 65536];
        // Bound work even when another process writes logs continuously.
        for _ in 0..16 {
            let length = match self.file.read(&mut buffer) {
                Ok(0) => return Err(io::Error::other("Directory watch closed")),
                Ok(length) => length,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(dirty),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            let mut offset = 0;
            while offset + 16 <= length {
                let bytes = &buffer[offset..];
                let wd = i32::from_ne_bytes(bytes[0..4].try_into().unwrap());
                let mask = u32::from_ne_bytes(bytes[4..8].try_into().unwrap());
                let size = u32::from_ne_bytes(bytes[12..16].try_into().unwrap()) as usize;
                if size > length - offset - 16 {
                    return Err(io::Error::other("Incomplete directory event"));
                }
                let bytes = &bytes[16..16 + size];
                let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
                dirty |= self.event(wd, mask, OsStr::from_bytes(&bytes[..end]))?;
                offset += 16 + size;
            }
            if offset != length {
                return Err(io::Error::other("Invalid directory event"));
            }
        }
        Ok(dirty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "alol-watch-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn idle_and_normal_game_writes_do_not_request_scans() {
        let root = Temp::new();
        let mut watch = ZoneWatch::new(&root.0).unwrap();
        assert!(!watch.changed().unwrap());
        fs::write(root.0.join("moonloader.log"), "normal log").unwrap();
        assert!(!watch.changed().unwrap());
        fs::write(
            root.0.join("font.ttf:Zone.Identifier"),
            "[ZoneTransfer]\nZoneId=3",
        )
        .unwrap();
        assert!(watch.changed().unwrap());
        assert!(!watch.changed().unwrap());
    }
    #[test]
    fn tracks_initial_download_new_subdirectories_and_moved_bundles() {
        let root = Temp::new();
        let game = root.0.join("bin/arizona");
        let mut watch = ZoneWatch::new(&game).unwrap();
        fs::create_dir_all(game.join("fonts/nested")).unwrap();
        fs::write(game.join("fonts/nested/a.ttf:Zone.Identifier"), "metadata").unwrap();
        assert!(watch.changed().unwrap());
        assert!(!watch.changed().unwrap());
        fs::write(game.join("fonts/nested/b.ttf:Zone.Identifier"), "metadata").unwrap();
        assert!(watch.changed().unwrap());
        let bundle = root.0.join("incoming");
        fs::create_dir(&bundle).unwrap();
        fs::write(bundle.join("c.dll:Zone.Identifier"), "metadata").unwrap();
        fs::rename(bundle, game.join("libraries")).unwrap();
        assert!(watch.changed().unwrap());
    }
    #[test]
    fn never_recurses_into_symlink_directories() {
        let root = Temp::new();
        let outside = Temp::new();
        std::os::unix::fs::symlink(&outside.0, root.0.join("linked")).unwrap();
        let mut watch = ZoneWatch::new(&root.0).unwrap();
        fs::write(outside.0.join("external.dll:Zone.Identifier"), "keep").unwrap();
        assert!(!watch.changed().unwrap());
    }
    #[test]
    fn lost_events_and_replaced_directories_request_fallback() {
        let root = Temp::new();
        let game = root.0.join("game");
        fs::create_dir(&game).unwrap();
        let mut watch = ZoneWatch::new(&game).unwrap();
        assert!(watch.event(-1, OVERFLOW, OsStr::new("")).is_err());
        fs::rename(&game, root.0.join("old-game")).unwrap();
        assert!(watch.changed().is_err());
    }
}
