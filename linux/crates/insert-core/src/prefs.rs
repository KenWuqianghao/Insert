use std::io;
use std::path::Path;

use serde_json::{json, Value};

use crate::format::format_count;
use crate::model::{app_id_key, SourceApp};
use crate::storage::write_private_atomic;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HistoryLimit {
    Clips100,
    #[default]
    Clips500,
    Clips1000,
    Clips5000,
}

impl HistoryLimit {
    pub const ALL: [HistoryLimit; 4] = [
        Self::Clips100,
        Self::Clips500,
        Self::Clips1000,
        Self::Clips5000,
    ];

    pub fn count(self) -> usize {
        match self {
            Self::Clips100 => 100,
            Self::Clips500 => 500,
            Self::Clips1000 => 1_000,
            Self::Clips5000 => 5_000,
        }
    }

    pub fn from_count(count: u64) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|limit| limit.count() as u64 == count)
    }

    pub fn label(self) -> String {
        format!("{} clips", format_count(self.count()))
    }
}

/// Stored as JSON in `$XDG_CONFIG_HOME/insert/preferences.json`. A missing or invalid value
/// gives its default; the other values still load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub show_tray_icon: bool,
    pub paste_directly: bool,
    pub history_limit: HistoryLimit,
    pub capture_paused: bool,
    pub ignored_apps: Vec<SourceApp>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            show_tray_icon: true,
            paste_directly: true,
            history_limit: HistoryLimit::default(),
            capture_paused: false,
            ignored_apps: Vec::new(),
        }
    }
}

impl Preferences {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .map(|bytes| Self::from_json(&bytes))
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_private_atomic(path, &self.to_json())
    }

    pub fn from_json(bytes: &[u8]) -> Self {
        let defaults = Self::default();
        let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(bytes) else {
            return defaults;
        };
        let flag =
            |key: &str, default: bool| map.get(key).and_then(Value::as_bool).unwrap_or(default);
        Self {
            show_tray_icon: flag("showTrayIcon", defaults.show_tray_icon),
            paste_directly: flag("pasteDirectly", defaults.paste_directly),
            history_limit: map
                .get("historyLimit")
                .and_then(Value::as_u64)
                .and_then(HistoryLimit::from_count)
                .unwrap_or(defaults.history_limit),
            capture_paused: flag("capturePaused", defaults.capture_paused),
            ignored_apps: map
                .get("ignoredApps")
                .and_then(Value::as_array)
                .map(|apps| {
                    apps.iter()
                        .filter_map(|app| serde_json::from_value(app.clone()).ok())
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    pub fn to_json(&self) -> Vec<u8> {
        let value = json!({
            "showTrayIcon": self.show_tray_icon,
            "pasteDirectly": self.paste_directly,
            "historyLimit": self.history_limit.count(),
            "capturePaused": self.capture_paused,
            "ignoredApps": self.ignored_apps,
        });
        serde_json::to_vec_pretty(&value).expect("preferences serialize")
    }

    /// True if a copy made while `source` is in front must not be saved.
    pub fn is_ignored(&self, source: Option<&SourceApp>) -> bool {
        source.is_some_and(|source| self.ignores_id(&source.id))
    }

    pub fn ignores_id(&self, id: &str) -> bool {
        let key = app_id_key(id);
        self.ignored_apps.iter().any(|app| app_id_key(&app.id) == key)
    }

    /// Returns false if an app with the same id is already in the list.
    pub fn ignore_app(&mut self, app: SourceApp) -> bool {
        if self.ignores_id(&app.id) {
            return false;
        }
        self.ignored_apps.push(app);
        true
    }

    pub fn unignore_app(&mut self, id: &str) {
        let key = app_id_key(id);
        self.ignored_apps.retain(|app| app_id_key(&app.id) != key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> SourceApp {
        SourceApp {
            id: id.into(),
            name: id.into(),
        }
    }

    #[test]
    fn defaults() {
        let prefs = Preferences::default();
        assert!(prefs.show_tray_icon && prefs.paste_directly);
        assert!(!prefs.capture_paused);
        assert_eq!(prefs.history_limit.count(), 500);
        assert!(prefs.ignored_apps.is_empty());
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("insert/preferences.json");
        let prefs = Preferences {
            show_tray_icon: false,
            paste_directly: false,
            history_limit: HistoryLimit::Clips5000,
            capture_paused: true,
            ignored_apps: vec![app("org.keepassxc.KeePassXC")],
        };
        prefs.save(&path).unwrap();
        assert_eq!(Preferences::load(&path), prefs);
    }

    #[test]
    fn bad_values_fall_back_per_key() {
        let prefs = Preferences::from_json(
            br#"{"historyLimit": 250, "pasteDirectly": "yes", "capturePaused": true, "showTrayIcon": false,
                 "ignoredApps": [{"id": "a", "name": "a"}, 7, {"id": "b"}], "unknown": 1}"#,
        );
        assert_eq!(prefs.history_limit, HistoryLimit::Clips500);
        assert!(prefs.paste_directly);
        assert!(prefs.capture_paused);
        assert!(!prefs.show_tray_icon);
        assert_eq!(prefs.ignored_apps, [app("a")]);
        assert_eq!(Preferences::from_json(b"[1,2]"), Preferences::default());
        assert_eq!(
            Preferences::load(Path::new("/nonexistent/preferences.json")),
            Preferences::default()
        );
    }

    #[test]
    fn ignored_apps_match_by_id_and_skip_duplicates() {
        let mut prefs = Preferences::default();
        assert!(prefs.ignore_app(app("a")));
        assert!(!prefs.ignore_app(SourceApp {
            id: "a".into(),
            name: "Other".into()
        }));
        assert!(prefs.is_ignored(Some(&app("a"))));
        assert!(prefs.is_ignored(Some(&app("A"))));
        assert!(!prefs.is_ignored(Some(&app("b"))));
        assert!(!prefs.is_ignored(None));
        prefs.unignore_app("a");
        assert!(!prefs.is_ignored(Some(&app("a"))));
    }

    #[test]
    fn history_limit_labels() {
        let labels: Vec<String> = HistoryLimit::ALL
            .iter()
            .map(|limit| limit.label())
            .collect();
        assert_eq!(
            labels,
            ["100 clips", "500 clips", "1,000 clips", "5,000 clips"]
        );
    }
}
