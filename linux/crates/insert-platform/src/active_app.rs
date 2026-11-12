//! A Wayland client cannot give its own window the tray role, and Plasma sends no activation token with a
//! global shortcut, so KWin must set them.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use insert_core::{app_id_key, SourceApp};

use crate::APP_ID;

const KWIN_SERVICE: &str = "org.kde.KWin";
const SCRIPTING_PATH: &str = "/Scripting";
const SCRIPTING_INTERFACE: &str = "org.kde.kwin.Scripting";
const SCRIPT_INTERFACE: &str = "org.kde.kwin.Script";
const SCRIPT_NAME: &str = "insert-active-app";
const RECEIVER_PATH: &str = "/io/github/kenwuqianghao/Insert/ActiveApp";
const RECEIVER_INTERFACE: &str = "io.github.kenwuqianghao.Insert.ActiveApp";

/// Terminals with no `TerminalEmulator` category in their desktop file, or with no desktop file.
const TERMINAL_APP_IDS: &[&str] = &[
    "konsole",
    "org.kde.konsole",
    "yakuake",
    "org.kde.yakuake",
    "gnome-terminal",
    "gnome-terminal-server",
    "org.gnome.terminal",
    "org.gnome.console",
    "kgx",
    "org.gnome.ptyxis",
    "kitty",
    "alacritty",
    "foot",
    "footclient",
    "wezterm",
    "org.wezfurlong.wezterm",
    "ghostty",
    "com.mitchellh.ghostty",
    "xterm",
    "tilix",
    "com.gexperts.tilix",
    "terminator",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveApp {
    /// `id` is the desktop file id of the window when KWin knows it, else the window class.
    /// `name` is `Name` from the desktop file, else the window class.
    pub source: SourceApp,
    /// Terminals use Ctrl+V for a control character.
    pub is_terminal: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DesktopEntry {
    name: Option<String>,
    terminal_emulator: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ActiveAppError {
    #[error("the active app is not available: {0}")]
    Unavailable(String),
}

#[derive(Default)]
struct Focus {
    /// `None` when no window is active or the active window has no app id.
    app: Option<ActiveApp>,
    insert_is_active: bool,
}

pub struct ActiveAppTracker {
    connection: zbus::Connection,
    focus: Arc<Mutex<Focus>>,
    external_activations: async_channel::Receiver<()>,
}

impl ActiveAppTracker {
    /// Loads the script into KWin. Fails on a desktop that is not KDE Plasma.
    ///
    /// Each time Insert opens a window with the title `tray_title`, KWin gives it the keyboard focus, keeps it
    /// above other windows, shows it on all desktops, and hides it from the taskbar, the pager and the window
    /// switcher.
    pub async fn start(tray_title: &str) -> Result<Self, ActiveAppError> {
        let focus = Arc::new(Mutex::new(Focus::default()));
        let (activated, external_activations) = async_channel::bounded(1);
        let connection = zbus::Connection::session().await.map_err(unavailable)?;
        let receiver = Receiver { focus: Arc::clone(&focus), activated, entries: HashMap::new() };
        connection.object_server().at(RECEIVER_PATH, receiver).await.map_err(unavailable)?;

        let unique_name = connection.unique_name().ok_or_else(|| unavailable("no D-Bus name"))?;
        let script_file = script_file()?;
        std::fs::write(&script_file, script_source(unique_name.as_str(), tray_title)).map_err(unavailable)?;

        // A script of an earlier run that did not stop cleanly still has this name.
        unload(&connection).await?;
        let script_id: i32 = scripting_call(&connection, "loadScript", &(script_file.to_string_lossy().as_ref(), SCRIPT_NAME))
            .await?
            .body()
            .deserialize()
            .map_err(unavailable)?;
        if script_id < 0 {
            return Err(unavailable("KWin did not load the script"));
        }
        connection
            .call_method(Some(KWIN_SERVICE), format!("{SCRIPTING_PATH}/Script{script_id}").as_str(), Some(SCRIPT_INTERFACE), "run", &())
            .await
            .map_err(unavailable)?;
        Ok(Self { connection, focus, external_activations })
    }

    /// The app of the last active window that is not a window of Insert. `None` when that
    /// window has no app id, or when no window is active.
    pub fn current(&self) -> Option<ActiveApp> {
        self.focus.lock().expect("the lock is not poisoned").app.clone()
    }

    /// Returns when the active window is not a window of Insert. Returns at once when that is already true.
    pub async fn external_window_active(&self) {
        while self.focus.lock().expect("the lock is not poisoned").insert_is_active {
            if self.external_activations.recv().await.is_err() {
                return;
            }
        }
    }

    pub async fn stop(&self) {
        let _ = unload(&self.connection).await;
    }
}

struct Receiver {
    focus: Arc<Mutex<Focus>>,
    activated: async_channel::Sender<()>,
    entries: HashMap<String, DesktopEntry>,
}

#[zbus::interface(name = "io.github.kenwuqianghao.Insert.ActiveApp")]
impl Receiver {
    fn activated(&mut self, desktop_file: String, resource_class: String) {
        let id = if desktop_file.is_empty() { resource_class.clone() } else { desktop_file };
        let insert_is_active = id == APP_ID;
        self.focus.lock().expect("the lock is not poisoned").insert_is_active = insert_is_active;
        if insert_is_active {
            return;
        }
        let app = (!id.is_empty()).then(|| {
            let entry = self.entries.entry(id.clone()).or_insert_with(|| desktop_entry(&id)).clone();
            let is_terminal = entry.terminal_emulator || TERMINAL_APP_IDS.contains(&app_id_key(&id).as_str());
            let fallback = if resource_class.is_empty() { id.clone() } else { resource_class };
            ActiveApp { source: SourceApp { name: entry.name.unwrap_or(fallback), id }, is_terminal }
        });
        self.focus.lock().expect("the lock is not poisoned").app = app;
        let _ = self.activated.try_send(());
    }

    /// KWin reports no window between the tray and the next window. That is not an external
    /// window yet, so a paste keeps its wait for one.
    fn no_window(&mut self) {
        self.focus.lock().expect("the lock is not poisoned").app = None;
    }
}

async fn scripting_call<B>(connection: &zbus::Connection, method: &str, body: &B) -> Result<zbus::Message, ActiveAppError>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    connection
        .call_method(Some(KWIN_SERVICE), SCRIPTING_PATH, Some(SCRIPTING_INTERFACE), method, body)
        .await
        .map_err(unavailable)
}

async fn unload(connection: &zbus::Connection) -> Result<(), ActiveAppError> {
    scripting_call(connection, "unloadScript", &(SCRIPT_NAME,)).await.map(|_| ())
}

fn script_file() -> Result<PathBuf, ActiveAppError> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(format!("{SCRIPT_NAME}.js")))
        .ok_or_else(|| unavailable("XDG_RUNTIME_DIR is not set"))
}

fn script_source(receiver: &str, tray_title: &str) -> String {
    format!(
        r#"function report(window) {{
    if (!window) {{
        callDBus("{receiver}", "{RECEIVER_PATH}", "{RECEIVER_INTERFACE}", "NoWindow");
        return;
    }}
    callDBus("{receiver}", "{RECEIVER_PATH}", "{RECEIVER_INTERFACE}", "Activated",
        String(window.desktopFileName), String(window.resourceClass));
}}
function setUpTray(window) {{
    if (String(window.desktopFileName) !== "{APP_ID}" || String(window.caption) !== {tray_title:?}) {{
        return;
    }}
    window.skipTaskbar = true;
    window.skipSwitcher = true;
    window.skipPager = true;
    window.keepAbove = true;
    window.onAllDesktops = true;
    workspace.activeWindow = window;
}}
workspace.windowActivated.connect(report);
workspace.windowAdded.connect(setUpTray);
workspace.windowList().forEach(setUpTray);
report(workspace.activeWindow);
"#
    )
}

fn desktop_entry(id: &str) -> DesktopEntry {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    data_home
        .into_iter()
        .chain(data_dirs.split(':').filter(|dir| !dir.is_empty()).map(PathBuf::from))
        .find_map(|dir| std::fs::read_to_string(dir.join("applications").join(format!("{id}.desktop"))).ok())
        .map(|text| parse_desktop_entry(&text))
        .unwrap_or_default()
}

fn parse_desktop_entry(text: &str) -> DesktopEntry {
    let group: Vec<&str> = text
        .lines()
        .skip_while(|line| line.trim() != "[Desktop Entry]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .collect();
    let value = |key: &str| group.iter().find_map(|line| line.strip_prefix(key)?.strip_prefix('=')).map(str::trim);
    DesktopEntry {
        name: value("Name").filter(|name| !name.is_empty()).map(str::to_owned),
        terminal_emulator: value("Categories").is_some_and(|categories| categories.split(';').any(|c| c == "TerminalEmulator")),
    }
}

fn unavailable(error: impl std::fmt::Display) -> ActiveAppError {
    ActiveAppError::Unavailable(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_and_category_come_from_the_desktop_entry_group() {
        let text = "[Desktop Action new]\nName=New Window\n\n[Desktop Entry]\nGenericName=Text Editor\nName[de]=KSchreiben\nName=KWrite\nExec=kwrite\nCategories=Qt;KDE;Utility;TextEditor;\n\n[Desktop Action other]\nName=Other\n";
        assert_eq!(parse_desktop_entry(text), DesktopEntry { name: Some("KWrite".into()), terminal_emulator: false });
        assert_eq!(parse_desktop_entry("[Desktop Entry]\nExec=x\n"), DesktopEntry::default());
        let terminal = "[Desktop Entry]\nName=Rio\nCategories=System;TerminalEmulator;\n";
        assert!(parse_desktop_entry(terminal).terminal_emulator);
    }

    #[test]
    fn a_known_terminal_id_in_any_letter_case_is_a_terminal() {
        let focus = Arc::new(Mutex::new(Focus::default()));
        let (activated, _activations) = async_channel::bounded(1);
        let mut receiver = Receiver { focus: Arc::clone(&focus), activated, entries: HashMap::new() };
        let is_terminal = |receiver: &mut Receiver, id: &str| {
            receiver.activated(id.into(), String::new());
            focus.lock().unwrap().app.as_ref().unwrap().is_terminal
        };
        assert!(is_terminal(&mut receiver, "Alacritty"));
        assert!(!is_terminal(&mut receiver, "zz.no.such.editor"));
    }

    #[test]
    fn a_window_of_insert_keeps_the_last_external_app() {
        let focus = Arc::new(Mutex::new(Focus::default()));
        let (activated, _activations) = async_channel::bounded(1);
        let mut receiver = Receiver { focus: Arc::clone(&focus), activated, entries: HashMap::new() };
        let state = || {
            let focus = focus.lock().unwrap();
            (focus.app.as_ref().map(|app| app.source.clone()), focus.insert_is_active)
        };
        receiver.activated("zz.no.such.app".into(), "someclass".into());
        let external = Some(SourceApp { id: "zz.no.such.app".into(), name: "someclass".into() });
        assert_eq!(state(), (external.clone(), false));
        receiver.activated(APP_ID.into(), "insert".into());
        assert_eq!(state(), (external, true));
    }

    #[test]
    fn no_active_window_has_no_app_and_is_not_an_external_window() {
        let focus = Arc::new(Mutex::new(Focus::default()));
        let (activated, activations) = async_channel::bounded(1);
        let mut receiver = Receiver { focus: Arc::clone(&focus), activated, entries: HashMap::new() };
        receiver.activated("zz.no.such.app".into(), "someclass".into());
        receiver.activated(APP_ID.into(), "insert".into());
        let _ = activations.try_recv();
        receiver.no_window();
        let focus = focus.lock().unwrap();
        assert!(focus.insert_is_active);
        assert_eq!(focus.app, None);
        assert!(activations.try_recv().is_err(), "a wake-up for no window");
    }

    #[test]
    fn a_window_with_no_app_id_is_an_external_window_with_no_app() {
        let focus = Arc::new(Mutex::new(Focus::default()));
        let (activated, activations) = async_channel::bounded(1);
        let mut receiver = Receiver { focus: Arc::clone(&focus), activated, entries: HashMap::new() };
        receiver.activated("zz.no.such.app".into(), "someclass".into());
        receiver.activated(APP_ID.into(), "insert".into());
        let _ = activations.try_recv();
        receiver.activated(String::new(), String::new());
        let focus = focus.lock().unwrap();
        assert!(!focus.insert_is_active);
        assert_eq!(focus.app, None);
        assert!(activations.try_recv().is_ok(), "no wake-up for the window");
    }
}
