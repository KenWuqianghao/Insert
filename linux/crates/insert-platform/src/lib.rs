//! Portal and D-Bus futures run on zbus's built-in runtime (ashpd `async-io`, no tokio), so the app can
//! await them from `glib::MainContext::spawn_local`.

pub mod active_app;
pub mod autostart;
pub mod clipboard;
pub mod paste;
pub mod portal;
pub mod shortcut;
pub mod tray_icon;

pub const APP_ID: &str = "io.github.kenwuqianghao.Insert";
