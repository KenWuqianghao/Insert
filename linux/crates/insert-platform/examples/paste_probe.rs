//! `paste_probe [interactive]`: opens the RemoteDesktop session, waits, then sends Ctrl+V.

use insert_platform::paste::{PasteInjector, PasteStatus};

fn main() {
    let interactive = std::env::args().nth(1).as_deref() == Some("interactive");
    futures_lite::future::block_on(async {
        let result = PasteInjector::connect(interactive).await;
        println!("status: {:?}", PasteStatus::of(&result));
        if let Ok(injector) = result {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            println!("send_paste: {:?}", injector.send_paste().await);
            std::thread::sleep(std::time::Duration::from_millis(500));
            injector.close().await;
        }
    });
}
