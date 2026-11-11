//! Desktop integration for Insert on Linux. No GTK dependency.
//!
//! Runtime: portal and D-Bus futures run on zbus's built-in runtime (ashpd `async-io`, no tokio), so the app can
//! await them from `glib::MainContext::spawn_local`. Long-lived listeners own a thread and report over `async-channel`.

pub mod autostart;

pub const APP_ID: &str = "io.github.kenwuqianghao.Insert";
