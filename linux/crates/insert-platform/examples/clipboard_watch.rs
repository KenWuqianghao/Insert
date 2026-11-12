//! Prints clipboard changes. `clipboard_watch set <mime> <text>` also writes the clipboard first.
//! Set INSERT_DATA_CONTROL=wlr or ext to force one protocol.

use insert_platform::clipboard::{ClipboardConfig, ClipboardEvent, ClipboardWatcher, DataControlProtocol};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let protocol = match std::env::var("INSERT_DATA_CONTROL").as_deref() {
        Ok("wlr") => DataControlProtocol::Wlr,
        Ok("ext") => DataControlProtocol::Ext,
        _ => DataControlProtocol::Auto,
    };
    let (sender, receiver) = async_channel::unbounded();
    let watcher = ClipboardWatcher::start(ClipboardConfig { protocol, ..Default::default() }, sender).unwrap();
    if args.get(1).map(String::as_str) == Some("set") {
        let result = watcher.set_clipboard(vec![(args[2].clone(), args[3].clone().into_bytes())]);
        println!("set_clipboard: {result:?}");
    }
    while let Ok(event) = receiver.recv_blocking() {
        match event {
            ClipboardEvent::Changed { mimes } => {
                println!("changed");
                for (mime, data) in mimes {
                    let preview = String::from_utf8_lossy(&data[..data.len().min(40)]).into_owned();
                    println!("  {mime} {} bytes {preview:?}", data.len());
                }
            }
            ClipboardEvent::Unavailable { reason } => {
                println!("unavailable: {reason}");
                break;
            }
        }
    }
}
