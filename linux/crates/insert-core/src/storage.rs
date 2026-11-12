use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::mime::MimeMap;
use crate::model::{Clip, ClipId, Pinboard};

const APP_DIR: &str = "insert";
const CORRUPT_INDEX_PREFIX: &str = "index.json.corrupt-";
/// Clips and preferences can hold passwords and private text. Only the user can read them.
const PRIVATE_DIR_MODE: u32 = 0o700;
const PRIVATE_FILE_MODE: u32 = 0o600;

/// The XDG base directories of Insert.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    /// Clips: `$XDG_DATA_HOME/insert`.
    pub data_dir: PathBuf,
    /// Preferences: `$XDG_CONFIG_HOME/insert`.
    pub config_dir: PathBuf,
    /// Portal restore tokens: `$XDG_STATE_HOME/insert`.
    pub state_dir: PathBuf,
}

impl Paths {
    pub fn from_env() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let base = |variable: &str, fallback: &str| {
            std::env::var_os(variable)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or_else(|| home.join(fallback))
                .join(APP_DIR)
        };
        Self {
            data_dir: base("XDG_DATA_HOME", ".local/share"),
            config_dir: base("XDG_CONFIG_HOME", ".config"),
            state_dir: base("XDG_STATE_HOME", ".local/state"),
        }
    }

    /// All three directories under one root, for tests and for a private profile.
    pub fn under(root: &Path) -> Self {
        Self {
            data_dir: root.join("data"),
            config_dir: root.join("config"),
            state_dir: root.join("state"),
        }
    }

    pub fn preferences_file(&self) -> PathBuf {
        self.config_dir.join("preferences.json")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LibraryIndex {
    pub clips: Vec<Clip>,
    pub pinboards: Vec<Pinboard>,
}

#[derive(Serialize)]
struct IndexRef<'a> {
    clips: &'a [Clip],
    pinboards: &'a [Pinboard],
}

#[derive(Debug)]
pub struct LoadedIndex {
    pub index: LibraryIndex,
    /// Where a corrupt `index.json` was moved before Insert started with an empty library.
    pub corrupt_backup: Option<PathBuf>,
}

/// Files under the data directory: `index.json`, `payloads/<id>/<MIME>`, `thumbnails/<id>.png`.
#[derive(Clone, Debug)]
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { root: data_dir }
    }

    pub fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    pub fn payload_dir(&self, id: ClipId) -> PathBuf {
        self.payloads_dir().join(id.to_string())
    }

    pub fn thumbnail_path(&self, id: ClipId) -> PathBuf {
        self.thumbnails_dir().join(format!("{id}.png"))
    }

    fn payloads_dir(&self) -> PathBuf {
        self.root.join("payloads")
    }

    fn thumbnails_dir(&self) -> PathBuf {
        self.root.join("thumbnails")
    }

    /// A missing index gives an empty library. An index that cannot be read or does not parse is
    /// renamed to `index.json.corrupt-<unix time>` so that the next save cannot destroy it.
    pub fn load_index(&self) -> LoadedIndex {
        let path = self.index_path();
        let parsed = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).ok(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Some(LibraryIndex::default()),
            Err(_) => None,
        };
        if let Some(index) = parsed {
            return LoadedIndex {
                index,
                corrupt_backup: None,
            };
        }
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let backup = self.root.join(format!("{CORRUPT_INDEX_PREFIX}{seconds}"));
        LoadedIndex {
            index: LibraryIndex::default(),
            corrupt_backup: fs::rename(&path, &backup).ok().map(|()| backup),
        }
    }

    /// True while an `index.json.corrupt-*` file exists. Its clips can own files that the
    /// current index does not know.
    pub fn has_corrupt_index_backup(&self) -> bool {
        fs::read_dir(&self.root).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(CORRUPT_INDEX_PREFIX)
            })
        })
    }

    pub fn save_index(&self, clips: &[Clip], pinboards: &[Pinboard]) -> io::Result<()> {
        let json = serde_json::to_vec(&IndexRef { clips, pinboards })?;
        write_atomic(&self.index_path(), &json)
    }

    /// Writes the payload directory and the thumbnail. On failure, nothing is left behind.
    pub fn write_clip(
        &self,
        id: ClipId,
        payload: &MimeMap,
        thumbnail_png: Option<&[u8]>,
    ) -> io::Result<()> {
        let result = self.try_write_clip(id, payload, thumbnail_png);
        if result.is_err() {
            self.remove_files(id);
        }
        result
    }

    fn try_write_clip(
        &self,
        id: ClipId,
        payload: &MimeMap,
        thumbnail_png: Option<&[u8]>,
    ) -> io::Result<()> {
        create_private_dir(&self.root)?;
        create_private_dir(&self.payloads_dir())?;
        let partial = self.payloads_dir().join(format!(".{id}.partial"));
        if partial.exists() {
            fs::remove_dir_all(&partial)?;
        }
        create_private_dir(&partial)?;
        for (mime, data) in &payload.0 {
            let mut file = create_private_file(&partial.join(encode_file_name(mime)))?;
            file.write_all(data)?;
            file.sync_all()?;
        }
        let final_dir = self.payload_dir(id);
        if final_dir.exists() {
            fs::remove_dir_all(&final_dir)?;
        }
        fs::rename(&partial, &final_dir)?;
        if let Some(png) = thumbnail_png {
            write_atomic(&self.thumbnail_path(id), png)?;
        }
        Ok(())
    }

    /// Entries are sorted by MIME name.
    pub fn load_payload(&self, id: ClipId) -> io::Result<MimeMap> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(self.payload_dir(id))? {
            let entry = entry?;
            let Some(mime) = entry.file_name().to_str().and_then(decode_file_name) else {
                continue;
            };
            entries.push((mime, fs::read(entry.path())?));
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(MimeMap(entries))
    }

    pub fn has_payload(&self, id: ClipId) -> bool {
        self.payload_dir(id).is_dir()
    }

    pub fn remove_files(&self, id: ClipId) {
        let _ = fs::remove_dir_all(self.payload_dir(id));
        let _ = fs::remove_file(self.thumbnail_path(id));
    }

    /// Deletes payloads and thumbnails that no kept clip owns. A crash between a payload write
    /// and an index save leaves such files.
    pub fn remove_orphans(&self, keep: &HashSet<ClipId>) {
        let owned =
            |name: &str| Uuid::parse_str(name).is_ok_and(|uuid| keep.contains(&ClipId(uuid)));
        for (dir, suffix) in [(self.payloads_dir(), ""), (self.thumbnails_dir(), ".png")] {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.strip_suffix(suffix).is_some_and(owned) {
                    continue;
                }
                let path = entry.path();
                let _ = if path.is_dir() {
                    fs::remove_dir_all(&path)
                } else {
                    fs::remove_file(&path)
                };
            }
        }
    }
}

/// Writes a private temp file next to `path`, flushes it, and renames it over `path`. The
/// directory of `path` becomes private too.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("path has no parent"))?;
    create_private_dir(dir)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("path has no file name"))?;
    let temp = dir.join(format!(".{}.tmp", name.to_string_lossy()));
    let mut file = create_private_file(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)
}

/// Creates `dir` and its missing parents with mode 0700. An existing `dir` also gets mode 0700,
/// because an earlier version of Insert created it with the default mode.
fn create_private_dir(dir: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(PRIVATE_DIR_MODE)
        .create(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
}

/// Creates or truncates `path` with mode 0600.
fn create_private_file(path: &Path) -> io::Result<fs::File> {
    let file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(PRIVATE_FILE_MODE)
        .open(path)?;
    file.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
    Ok(file)
}

fn encode_file_name(mime: &str) -> String {
    let mut name = String::with_capacity(mime.len());
    for byte in mime.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+') {
            name.push(char::from(byte));
        } else {
            name.push_str(&format!("%{byte:02X}"));
        }
    }
    name
}

fn decode_file_name(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ClipContent, ContentHash, TextSnippet};
    use time::OffsetDateTime;

    fn clip() -> Clip {
        Clip {
            id: ClipId::new(),
            created_at: OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap(),
            source: None,
            content: ClipContent::Text(TextSnippet::new("hello")),
            custom_title: Some("Greeting".into()),
            pinboard_id: None,
            content_hash: ContentHash("ab".repeat(32)),
            byte_count: 5,
        }
    }

    #[test]
    fn file_names_round_trip_mime_types() {
        for mime in [
            "text/plain;charset=utf-8",
            "UTF8_STRING",
            "image/svg+xml",
            "x-special/gnome-copied-files",
        ] {
            let name = encode_file_name(mime);
            assert!(!name.contains('/'), "{name}");
            assert_eq!(decode_file_name(&name).as_deref(), Some(mime));
        }
        assert_eq!(encode_file_name("text/plain"), "text%2Fplain");
    }

    #[test]
    fn index_and_payload_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::new(dir.path().to_owned());
        let clip = clip();
        let payload = MimeMap(vec![
            ("text/html".into(), b"<b>x</b>".to_vec()),
            ("text/plain".into(), b"x".to_vec()),
        ]);
        storage.write_clip(clip.id, &payload, Some(b"png")).unwrap();
        storage
            .save_index(std::slice::from_ref(&clip), &[])
            .unwrap();

        let loaded = storage.load_index();
        assert_eq!(loaded.index.clips, std::slice::from_ref(&clip));
        assert_eq!(loaded.corrupt_backup, None);
        assert_eq!(storage.load_payload(clip.id).unwrap(), payload);
        assert_eq!(fs::read(storage.thumbnail_path(clip.id)).unwrap(), b"png");
    }

    #[test]
    fn missing_index_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = Storage::new(dir.path().to_owned()).load_index();
        assert_eq!(loaded.index, LibraryIndex::default());
        assert_eq!(loaded.corrupt_backup, None);
    }

    #[test]
    fn corrupt_index_is_renamed_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::new(dir.path().to_owned());
        fs::write(storage.index_path(), b"{not json").unwrap();

        let loaded = storage.load_index();
        assert_eq!(loaded.index, LibraryIndex::default());
        let backup = loaded.corrupt_backup.expect("backup path");
        assert!(backup
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("index.json.corrupt-"));
        assert_eq!(fs::read(&backup).unwrap(), b"{not json");
        assert!(!storage.index_path().exists());
    }

    #[test]
    fn orphans_are_removed_and_owned_files_kept() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::new(dir.path().to_owned());
        let payload = MimeMap(vec![("text/plain".into(), b"x".to_vec())]);
        let (kept, orphan) = (ClipId::new(), ClipId::new());
        for id in [kept, orphan] {
            storage.write_clip(id, &payload, Some(b"png")).unwrap();
        }
        fs::create_dir_all(storage.payloads_dir().join(format!(".{orphan}.partial"))).unwrap();

        storage.remove_orphans(&HashSet::from([kept]));
        assert!(storage.has_payload(kept));
        assert!(storage.thumbnail_path(kept).exists());
        assert!(!storage.has_payload(orphan));
        assert!(!storage.thumbnail_path(orphan).exists());
        assert_eq!(fs::read_dir(storage.payloads_dir()).unwrap().count(), 1);
    }

    #[test]
    fn only_the_user_can_read_the_stored_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("insert");
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        let storage = Storage::new(root.clone());
        let id = ClipId::new();
        let payload = MimeMap(vec![("text/plain".into(), b"x".to_vec())]);
        storage.write_clip(id, &payload, Some(b"png")).unwrap();
        storage.save_index(&[], &[]).unwrap();
        let preferences = dir.path().join("config/insert/preferences.json");
        crate::Preferences::default().save(&preferences).unwrap();

        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        for private_dir in [
            root.clone(),
            storage.payloads_dir(),
            storage.payload_dir(id),
            storage.thumbnails_dir(),
            preferences.parent().unwrap().to_owned(),
        ] {
            assert_eq!(mode(&private_dir), 0o700, "{}", private_dir.display());
        }
        for private_file in [
            storage.payload_dir(id).join("text%2Fplain"),
            storage.thumbnail_path(id),
            storage.index_path(),
            preferences,
        ] {
            assert_eq!(mode(&private_file), 0o600, "{}", private_file.display());
        }
    }

    #[test]
    fn xdg_paths_end_in_the_app_dir() {
        let paths = Paths::from_env();
        assert!(paths.data_dir.ends_with("insert"));
        assert!(paths
            .preferences_file()
            .ends_with("insert/preferences.json"));
    }
}
