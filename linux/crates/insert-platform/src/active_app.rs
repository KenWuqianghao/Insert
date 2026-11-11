//! The app of the active window, from a KWin script that reports each window activation over D-Bus.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::APP_ID;

const KWIN_SERVICE: &str = "org.kde.KWin";
const SCRIPTING_PATH: &str = "/Scripting";
const SCRIPTING_INTERFACE: &str = "org.kde.kwin.Scripting";
const SCRIPT_INTERFACE: &str = "org.kde.kwin.Script";
const SCRIPT_NAME: &str = "insert-active-app";
const RECEIVER_PATH: &str = "/io/github/kenwuqianghao/Insert/ActiveApp";
const RECEIVER_INTERFACE: &str = "io.github.kenwuqianghao.Insert.ActiveApp";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceApp {
    /// The desktop file id of the window when KWin knows it, else the window class.
    pub id: String,
    /// `Name` from the desktop file, else the window class.
    pub name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ActiveAppError {
    #[error("the active app is not available: {0}")]
    Unavailable(String),
}

pub struct ActiveAppTracker {
    connection: zbus::Connection,
    current: Arc<Mutex<Option<SourceApp>>>,
}

impl ActiveAppTracker {
    /// Loads the script into KWin. Fails on a desktop that is not KDE Plasma.
    pub async fn start() -> Result<Self, ActiveAppError> {
        let current = Arc::new(Mutex::new(None));
        let connection = zbus::Connection::session().await.map_err(unavailable)?;
        let receiver = Receiver { current: Arc::clone(&current), names: HashMap::new() };
        connection.object_server().at(RECEIVER_PATH, receiver).await.map_err(unavailable)?;

        let unique_name = connection.unique_name().ok_or_else(|| unavailable("no D-Bus name"))?;
        let script_file = script_file();
        std::fs::write(&script_file, script_source(unique_name.as_str())).map_err(unavailable)?;

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
        Ok(Self { connection, current })
    }

    /// The app of the active window. `None` when no window is active.
    /// A window of Insert does not change the value.
    pub fn current(&self) -> Option<SourceApp> {
        self.current.lock().expect("the lock is not poisoned").clone()
    }

    pub async fn stop(self) {
        let _ = unload(&self.connection).await;
    }
}

struct Receiver {
    current: Arc<Mutex<Option<SourceApp>>>,
    names: HashMap<String, Option<String>>,
}

#[zbus::interface(name = "io.github.kenwuqianghao.Insert.ActiveApp")]
impl Receiver {
    fn activated(&mut self, desktop_file: String, resource_class: String) {
        let id = if desktop_file.is_empty() { resource_class.clone() } else { desktop_file };
        if id == APP_ID {
            return;
        }
        let app = (!id.is_empty()).then(|| {
            let name = self.names.entry(id.clone()).or_insert_with(|| desktop_entry_name(&id)).clone();
            let fallback = if resource_class.is_empty() { id.clone() } else { resource_class };
            SourceApp { name: name.unwrap_or(fallback), id }
        });
        *self.current.lock().expect("the lock is not poisoned") = app;
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

fn script_file() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("{SCRIPT_NAME}.js"))
}

fn script_source(receiver: &str) -> String {
    format!(
        r#"function report(window) {{
    callDBus("{receiver}", "{RECEIVER_PATH}", "{RECEIVER_INTERFACE}", "Activated",
        window ? String(window.desktopFileName) : "", window ? String(window.resourceClass) : "");
}}
workspace.windowActivated.connect(report);
report(workspace.activeWindow);
"#
    )
}

fn desktop_entry_name(id: &str) -> Option<String> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    data_home
        .into_iter()
        .chain(data_dirs.split(':').filter(|dir| !dir.is_empty()).map(PathBuf::from))
        .find_map(|dir| std::fs::read_to_string(dir.join("applications").join(format!("{id}.desktop"))).ok())
        .and_then(|text| parse_desktop_entry_name(&text))
}

fn parse_desktop_entry_name(text: &str) -> Option<String> {
    text.lines()
        .skip_while(|line| line.trim() != "[Desktop Entry]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .find_map(|line| line.strip_prefix("Name=").map(|name| name.trim().to_owned()))
        .filter(|name| !name.is_empty())
}

fn unavailable(error: impl std::fmt::Display) -> ActiveAppError {
    ActiveAppError::Unavailable(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_comes_from_the_desktop_entry_group() {
        let text = "[Desktop Action new]\nName=New Window\n\n[Desktop Entry]\nGenericName=Text Editor\nName[de]=KSchreiben\nName=KWrite\nExec=kwrite\n\n[Desktop Action other]\nName=Other\n";
        assert_eq!(parse_desktop_entry_name(text).as_deref(), Some("KWrite"));
        assert_eq!(parse_desktop_entry_name("[Desktop Entry]\nExec=x\n"), None);
    }

    #[test]
    fn a_window_of_insert_keeps_the_last_external_app() {
        let current = Arc::new(Mutex::new(None));
        let mut receiver = Receiver { current: Arc::clone(&current), names: HashMap::new() };
        receiver.activated("zz.no.such.app".into(), "someclass".into());
        let external = Some(SourceApp { id: "zz.no.such.app".into(), name: "someclass".into() });
        assert_eq!(*current.lock().unwrap(), external);
        receiver.activated(APP_ID.into(), "insert".into());
        assert_eq!(*current.lock().unwrap(), external);
        receiver.activated(String::new(), String::new());
        assert_eq!(*current.lock().unwrap(), None);
    }
}
