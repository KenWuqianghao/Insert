//! Prints the active app each time it changes.

use insert_platform::active_app::ActiveAppTracker;

fn main() {
    let tracker = match futures_lite::future::block_on(ActiveAppTracker::start()) {
        Ok(tracker) => tracker,
        Err(error) => {
            println!("start failed: {error}");
            return;
        }
    };
    println!("started");
    let seconds: u64 = std::env::args().nth(1).and_then(|arg| arg.parse().ok()).unwrap_or(30);
    let mut last = None;
    for _ in 0..seconds * 10 {
        let current = tracker.current();
        if current != last {
            println!("{current:?}");
            last = current;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    futures_lite::future::block_on(tracker.stop());
    println!("stopped");
}
