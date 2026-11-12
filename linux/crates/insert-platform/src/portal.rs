use std::sync::atomic::{AtomicBool, Ordering};

use crate::APP_ID;

static REGISTERED: AtomicBool = AtomicBool::new(false);

/// Gives the portals the app id of this host process. Without it KDE stores permissions under an empty id.
/// The portal accepts the call only as the first call on the connection, so each portal user awaits this first.
/// A call during a running call does not wait for it, so await this once before two portal users start together.
pub async fn register_app() {
    if REGISTERED.load(Ordering::Acquire) {
        return;
    }
    let result = match ashpd::AppID::try_from(APP_ID) {
        Ok(app_id) => ashpd::register_host_app(app_id).await,
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        log::debug!("portal app registration failed: {error}");
    }
    REGISTERED.store(true, Ordering::Release);
}
