//! Binds the global shortcut and prints its events. `shortcut_probe configure` also opens the settings dialog.

use insert_platform::shortcut::GlobalShortcut;

fn main() {
    let configure = std::env::args().nth(1).as_deref() == Some("configure");
    let (sender, receiver) = async_channel::unbounded();
    futures_lite::future::block_on(async {
        let shortcut = match GlobalShortcut::connect(sender).await {
            Ok(shortcut) => shortcut,
            Err(error) => {
                println!("connect failed: {error}");
                return;
            }
        };
        println!("connected, can_configure={}", shortcut.can_configure());
        if configure {
            println!("configure: {:?}", shortcut.configure().await);
        }
        while let Ok(event) = receiver.recv().await {
            println!("{event:?}");
        }
    });
}
