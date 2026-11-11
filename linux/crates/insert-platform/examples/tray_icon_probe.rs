//! Shows the tray icon and prints menu events. The pause item changes the icon state.

use insert_platform::tray_icon::{TrayEvent, TrayIcon};

fn main() {
    let (sender, receiver) = async_channel::unbounded();
    let icon = match TrayIcon::spawn(sender, false) {
        Ok(icon) => icon,
        Err(error) => {
            println!("spawn failed: {error}");
            return;
        }
    };
    println!("spawned");
    let mut paused = false;
    while let Ok(event) = receiver.recv_blocking() {
        println!("{event:?}");
        match event {
            TrayEvent::TogglePause => {
                paused = !paused;
                icon.set_paused(paused);
            }
            TrayEvent::Quit => break,
            _ => {}
        }
    }
}
