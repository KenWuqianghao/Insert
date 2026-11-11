//! Test tool for `linux/tools/nested-kwin.sh`: presses keys in the virtual KWin through the RemoteDesktop portal.
//! Do not run it on the real desktop.
//!
//! Steps are arguments, in order: `wait:<ms>`, `key:<keysym hex>` (press and release), `down:<keysym hex>`,
//! `up:<keysym hex>`. Example, Return then Ctrl+Shift+V: `nested_keys key:ff0d wait:500 down:ffe3 down:ffe1 key:56 up:ffe1 up:ffe3`.
//!
//! The script must first let this tool skip the permission dialog, with a private `XDG_DATA_HOME`:
//! 1. Write `$XDG_DATA_HOME/applications/dev.insert.NestedKeys.desktop` (Type, Name, Exec).
//! 2. Run `flatpak permission-set kde-authorized remote-desktop dev.insert.NestedKeys yes`.

use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions};

fn main() {
    futures_lite::future::block_on(async {
        ashpd::register_host_app("dev.insert.NestedKeys".try_into().unwrap()).await.unwrap();
        let portal = RemoteDesktop::new().await.unwrap();
        let session = portal.create_session(Default::default()).await.unwrap();
        portal
            .select_devices(&session, SelectDevicesOptions::default().set_devices(DeviceType::KEYBOARD))
            .await
            .unwrap()
            .response()
            .unwrap();
        portal.start(&session, None, Default::default()).await.unwrap().response().unwrap();
        println!("ready");
        for arg in std::env::args().skip(1) {
            let (kind, value) = arg.split_once(':').unwrap();
            match kind {
                "wait" => std::thread::sleep(std::time::Duration::from_millis(value.parse().unwrap())),
                "key" => {
                    let keysym = i32::from_str_radix(value, 16).unwrap();
                    for state in [KeyState::Pressed, KeyState::Released] {
                        portal.notify_keyboard_keysym(&session, keysym, state, Default::default()).await.unwrap();
                    }
                    println!("pressed {value}");
                }
                "down" | "up" => {
                    let keysym = i32::from_str_radix(value, 16).unwrap();
                    let state = if kind == "down" { KeyState::Pressed } else { KeyState::Released };
                    portal.notify_keyboard_keysym(&session, keysym, state, Default::default()).await.unwrap();
                }
                _ => panic!("unknown step {arg}"),
            }
        }
    });
}
