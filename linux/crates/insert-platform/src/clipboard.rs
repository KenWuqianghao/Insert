//! Clipboard capture and clipboard writes through the Wayland data-control protocols.
//!
//! The watcher owns a thread and a Wayland connection. Data-control needs no keyboard focus, so the app can
//! write the clipboard after its window hides.

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::os::fd::{AsFd, OwnedFd};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::fs::{fcntl_setfl, OFlags};
use rustix::pipe::{pipe_with, PipeFlags};
use wayland_client::backend::ObjectId;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry::WlRegistry, wl_seat::WlSeat};
use wayland_client::{event_created_child, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::{self, ExtDataControlOfferV1},
    ext_data_control_source_v1::{self, ExtDataControlSourceV1},
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::{self, ZwlrDataControlDeviceV1},
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_offer_v1::{self, ZwlrDataControlOfferV1},
    zwlr_data_control_source_v1::{self, ZwlrDataControlSourceV1},
};

/// `set_clipboard` adds this type to each write. The watcher ignores offers that have it.
pub const ORIGIN_MIME: &str = "application/x-insert-origin";
/// KDE password managers set this type to `secret`.
pub const CONCEALED_MIME: &str = "x-kde-passwordManagerHint";
pub const DEFAULT_MAX_TOTAL_BYTES: usize = 50 * 1024 * 1024;

const X11_META_TARGETS: [&str; 4] = ["TARGETS", "TIMESTAMP", "MULTIPLE", "SAVE_TARGETS"];
const READ_IDLE_TIMEOUT: Duration = Duration::from_secs(10);
const SET_TIMEOUT: Duration = Duration::from_secs(3);

pub type MimeData = Vec<(String, Vec<u8>)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardEvent {
    /// Another client changed the clipboard. An entry can have no data. `mimes` is empty when `concealed` is true.
    Changed { mimes: MimeData, concealed: bool },
    /// The watcher stopped. No more events follow.
    Unavailable { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataControlProtocol {
    /// `ext-data-control-v1` when the compositor has it, else `zwlr-data-control-unstable-v1`.
    #[default]
    Auto,
    Ext,
    Wlr,
}

pub struct ClipboardConfig {
    /// Gets the MIME types of an offer and returns the ones that the watcher must read.
    pub select_mimes: Box<dyn Fn(&[String]) -> Vec<String> + Send>,
    /// A clipboard change with more accepted bytes than this gives no event.
    pub max_total_bytes: usize,
    pub protocol: DataControlProtocol,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            select_mimes: Box::new(|mimes| mimes.to_vec()),
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            protocol: DataControlProtocol::Auto,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("the clipboard watcher is not running")]
    NotRunning,
    #[error("the compositor did not confirm the clipboard write")]
    Timeout,
    #[error("the clipboard write failed: {0}")]
    Wayland(String),
}

pub struct ClipboardWatcher {
    commands: mpsc::Sender<Command>,
    wake: File,
    thread: Option<JoinHandle<()>>,
}

impl ClipboardWatcher {
    /// Starts the watcher thread. A start failure arrives as `ClipboardEvent::Unavailable`.
    /// The clipboard content at start gives no event.
    pub fn start(config: ClipboardConfig, events: async_channel::Sender<ClipboardEvent>) -> std::io::Result<Self> {
        let (wake_rx, wake_tx) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)?;
        let (commands, command_rx) = mpsc::channel();
        let thread = std::thread::Builder::new().name("insert-clipboard".into()).spawn(move || {
            if let Err(reason) = run(config, &events, command_rx, wake_rx) {
                let _ = events.send_blocking(ClipboardEvent::Unavailable { reason });
            }
        })?;
        Ok(Self { commands, wake: File::from(wake_tx), thread: Some(thread) })
    }

    /// Replaces the clipboard content and serves it until another client replaces it.
    /// Returns after the compositor has the new selection. An empty list clears the clipboard.
    /// The watcher gives no `Changed` event for this write.
    pub fn set_clipboard(&self, mimes: MimeData) -> Result<(), ClipboardError> {
        self.request(|done| Command::Set { mimes, done })
    }

    /// Offers the content of the last `set_clipboard` as the primary selection too. Most terminals paste the
    /// primary selection on Shift+Insert.
    pub fn mirror_to_primary(&self) -> Result<(), ClipboardError> {
        self.request(|done| Command::MirrorToPrimary { done })
    }

    fn request(&self, command: impl FnOnce(Done) -> Command) -> Result<(), ClipboardError> {
        let (done, done_rx) = mpsc::sync_channel(1);
        self.send(command(done))?;
        match done_rx.recv_timeout(SET_TIMEOUT) {
            Ok(result) => result.map_err(ClipboardError::Wayland),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ClipboardError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ClipboardError::NotRunning),
        }
    }

    fn send(&self, command: Command) -> Result<(), ClipboardError> {
        self.commands.send(command).map_err(|_| ClipboardError::NotRunning)?;
        // A full pipe already has a wake-up in it.
        let _ = (&self.wake).write(&[1]);
        Ok(())
    }
}

impl Drop for ClipboardWatcher {
    fn drop(&mut self) {
        let _ = self.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

type Done = mpsc::SyncSender<Result<(), String>>;

enum Command {
    Set { mimes: MimeData, done: Done },
    MirrorToPrimary { done: Done },
    Stop,
}

enum Manager {
    Ext(ExtDataControlManagerV1),
    Wlr(ZwlrDataControlManagerV1),
}

enum Device {
    Ext(ExtDataControlDeviceV1),
    Wlr(ZwlrDataControlDeviceV1),
}

enum Offer {
    Ext(ExtDataControlOfferV1),
    Wlr(ZwlrDataControlOfferV1),
}

enum Source {
    Ext(ExtDataControlSourceV1),
    Wlr(ZwlrDataControlSourceV1),
}

impl Manager {
    fn device(&self, seat: &WlSeat, qh: &QueueHandle<State>) -> Device {
        match self {
            Self::Ext(manager) => Device::Ext(manager.get_data_device(seat, qh, ())),
            Self::Wlr(manager) => Device::Wlr(manager.get_data_device(seat, qh, ())),
        }
    }

    fn source(&self, qh: &QueueHandle<State>) -> Source {
        match self {
            Self::Ext(manager) => Source::Ext(manager.create_data_source(qh, ())),
            Self::Wlr(manager) => Source::Wlr(manager.create_data_source(qh, ())),
        }
    }
}

impl Device {
    fn set_selection(&self, source: Option<&Source>) {
        match (self, source) {
            (Self::Ext(device), Some(Source::Ext(source))) => device.set_selection(Some(source)),
            (Self::Wlr(device), Some(Source::Wlr(source))) => device.set_selection(Some(source)),
            (Self::Ext(device), None) => device.set_selection(None),
            (Self::Wlr(device), None) => device.set_selection(None),
            (Self::Ext(_), Some(Source::Wlr(_))) | (Self::Wlr(_), Some(Source::Ext(_))) => {
                unreachable!("a source comes from the manager of its device")
            }
        }
    }
}

impl Device {
    /// `zwlr-data-control` has the primary selection from version 2.
    fn has_primary_selection(&self) -> bool {
        match self {
            Self::Ext(_) => true,
            Self::Wlr(device) => device.version() >= 2,
        }
    }

    fn set_primary_selection(&self, source: &Source) {
        match (self, source) {
            (Self::Ext(device), Source::Ext(source)) => device.set_primary_selection(Some(source)),
            (Self::Wlr(device), Source::Wlr(source)) => device.set_primary_selection(Some(source)),
            (Self::Ext(_), Source::Wlr(_)) | (Self::Wlr(_), Source::Ext(_)) => {
                unreachable!("a source comes from the manager of its device")
            }
        }
    }
}

impl Offer {
    fn receive(&self, mime: &str, fd: &OwnedFd) {
        match self {
            Self::Ext(offer) => offer.receive(mime.to_owned(), fd.as_fd()),
            Self::Wlr(offer) => offer.receive(mime.to_owned(), fd.as_fd()),
        }
    }

    fn destroy(&self) {
        match self {
            Self::Ext(offer) => offer.destroy(),
            Self::Wlr(offer) => offer.destroy(),
        }
    }
}

impl Source {
    fn offer(&self, mime: &str) {
        match self {
            Self::Ext(source) => source.offer(mime.to_owned()),
            Self::Wlr(source) => source.offer(mime.to_owned()),
        }
    }

    fn destroy(&self) {
        match self {
            Self::Ext(source) => source.destroy(),
            Self::Wlr(source) => source.destroy(),
        }
    }

    fn id(&self) -> ObjectId {
        match self {
            Self::Ext(source) => source.id(),
            Self::Wlr(source) => source.id(),
        }
    }
}

struct AnnouncedOffer {
    offer: Offer,
    mimes: Vec<String>,
}

struct Transfer {
    offer: Offer,
    queue: VecDeque<String>,
    collected: MimeData,
    total: usize,
    current: Option<PipeRead>,
}

struct PipeRead {
    mime: String,
    pipe: File,
    data: Vec<u8>,
    deadline: Instant,
}

struct OwnedSelection {
    source: Source,
    data: Arc<HashMap<String, Vec<u8>>>,
}

struct State {
    connection: Connection,
    events: async_channel::Sender<ClipboardEvent>,
    select_mimes: Box<dyn Fn(&[String]) -> Vec<String> + Send>,
    max_total_bytes: usize,
    announced: HashMap<ObjectId, AnnouncedOffer>,
    transfer: Option<Transfer>,
    owned: Option<OwnedSelection>,
    owned_primary: Option<OwnedSelection>,
    /// True until the first roundtrip ends, so the clipboard content at start gives no event.
    starting: bool,
    stop: Option<Result<(), String>>,
}

impl State {
    fn offer_announced(&mut self, offer: Offer, id: ObjectId) {
        self.announced.insert(id, AnnouncedOffer { offer, mimes: Vec::new() });
    }

    fn offer_mime(&mut self, id: &ObjectId, mime: String) {
        if let Some(announced) = self.announced.get_mut(id) {
            if !announced.mimes.contains(&mime) {
                announced.mimes.push(mime);
            }
        }
    }

    fn drop_offer(&mut self, id: Option<ObjectId>) {
        if let Some(announced) = id.and_then(|id| self.announced.remove(&id)) {
            announced.offer.destroy();
        }
    }

    fn selection_changed(&mut self, id: Option<ObjectId>) {
        if let Some(previous) = self.transfer.take() {
            previous.offer.destroy();
        }
        let Some(AnnouncedOffer { offer, mimes }) = id.and_then(|id| self.announced.remove(&id)) else {
            return;
        };
        if self.starting || mimes.iter().any(|mime| mime == ORIGIN_MIME) {
            offer.destroy();
            return;
        }
        let mut queue = VecDeque::new();
        if mimes.iter().any(|mime| mime == CONCEALED_MIME) {
            queue.push_back(CONCEALED_MIME.to_owned());
        }
        queue.extend(
            (self.select_mimes)(&mimes)
                .into_iter()
                .filter(|mime| mime != CONCEALED_MIME && !X11_META_TARGETS.contains(&mime.as_str())),
        );
        self.transfer = Some(Transfer { offer, queue, collected: Vec::new(), total: 0, current: None });
        self.start_next_read();
    }

    fn start_next_read(&mut self) {
        let Some(transfer) = &mut self.transfer else { return };
        let Some(mime) = transfer.queue.pop_front() else {
            let transfer = self.transfer.take().expect("checked above");
            transfer.offer.destroy();
            if !transfer.collected.is_empty() {
                self.emit(ClipboardEvent::Changed { mimes: transfer.collected, concealed: false });
            }
            return;
        };
        // Only the read end is nonblocking: the write end goes to the source app, which can expect blocking writes.
        let pipe = pipe_with(PipeFlags::CLOEXEC)
            .and_then(|(read_end, write_end)| fcntl_setfl(&read_end, OFlags::NONBLOCK).map(|()| (read_end, write_end)));
        let (read_end, write_end) = match pipe {
            Ok(pipe) => pipe,
            Err(error) => {
                self.stop = Some(Err(format!("cannot create a pipe: {error}")));
                return;
            }
        };
        transfer.offer.receive(&mime, &write_end);
        // The source sees end-of-file only after this process closes its copy of the write end.
        let _ = self.connection.flush();
        drop(write_end);
        transfer.current = Some(PipeRead {
            mime,
            pipe: File::from(read_end),
            data: Vec::new(),
            deadline: Instant::now() + READ_IDLE_TIMEOUT,
        });
    }

    fn pipe_readable(&mut self) {
        let Some(transfer) = &mut self.transfer else { return };
        let Some(read) = &mut transfer.current else { return };
        let mut chunk = [0u8; 64 * 1024];
        match read.pipe.read(&mut chunk) {
            Ok(0) => {
                let PipeRead { mime, data, .. } = transfer.current.take().expect("checked above");
                if mime == CONCEALED_MIME {
                    if data == b"secret" {
                        self.abandon_transfer();
                        self.emit(ClipboardEvent::Changed { mimes: Vec::new(), concealed: true });
                        return;
                    }
                } else {
                    // An empty entry still counts: privacy markers such as `org.nspasteboard.ConcealedType` have no data.
                    transfer.collected.push((mime, data));
                }
                self.start_next_read();
            }
            Ok(count) => {
                transfer.total += count;
                if transfer.total > self.max_total_bytes {
                    log::debug!("clipboard change skipped: more than {} bytes", self.max_total_bytes);
                    self.abandon_transfer();
                    return;
                }
                read.data.extend_from_slice(&chunk[..count]);
                read.deadline = Instant::now() + READ_IDLE_TIMEOUT;
            }
            // The poll result can be for a pipe that a new selection has replaced.
            Err(error) if matches!(error.kind(), std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock) => {}
            Err(error) => {
                log::debug!("clipboard read of {} failed: {error}", read.mime);
                transfer.current = None;
                self.start_next_read();
            }
        }
    }

    fn abandon_transfer(&mut self) {
        if let Some(transfer) = self.transfer.take() {
            transfer.offer.destroy();
        }
    }

    fn read_deadline(&self) -> Option<Instant> {
        self.transfer.as_ref()?.current.as_ref().map(|read| read.deadline)
    }

    fn source_send(&mut self, source: &ObjectId, mime: String, fd: OwnedFd) {
        let mut selections = self.owned.iter().chain(&self.owned_primary);
        let Some(owned) = selections.find(|owned| &owned.source.id() == source) else { return };
        let data = Arc::clone(&owned.data);
        // A slow reader must not stop the event loop.
        std::thread::spawn(move || {
            if let Some(bytes) = data.get(&mime) {
                let _ = File::from(fd).write_all(bytes);
            }
        });
    }

    fn source_cancelled(&mut self, source: &ObjectId) {
        for selection in [&mut self.owned, &mut self.owned_primary] {
            if let Some(owned) = selection.take_if(|owned| &owned.source.id() == source) {
                owned.source.destroy();
            }
        }
    }

    fn emit(&mut self, event: ClipboardEvent) {
        if self.events.send_blocking(event).is_err() {
            self.stop = Some(Ok(()));
        }
    }
}

fn run(
    config: ClipboardConfig,
    events: &async_channel::Sender<ClipboardEvent>,
    commands: mpsc::Receiver<Command>,
    wake: OwnedFd,
) -> Result<(), String> {
    let connection =
        Connection::connect_to_env().map_err(|error| format!("no Wayland connection: {error}"))?;
    let (globals, mut queue) =
        registry_queue_init::<State>(&connection).map_err(|error| format!("no Wayland registry: {error}"))?;
    let qh = queue.handle();
    let seat: WlSeat = globals.bind(&qh, 1..=1, ()).map_err(|error| format!("no Wayland seat: {error}"))?;
    let ext = || globals.bind::<ExtDataControlManagerV1, _, _>(&qh, 1..=1, ()).map(Manager::Ext);
    let wlr = || globals.bind::<ZwlrDataControlManagerV1, _, _>(&qh, 1..=2, ()).map(Manager::Wlr);
    let manager = match config.protocol {
        DataControlProtocol::Auto => ext().or_else(|_| wlr()),
        DataControlProtocol::Ext => ext(),
        DataControlProtocol::Wlr => wlr(),
    }
    .map_err(|error| format!("the compositor has no data-control protocol: {error}"))?;
    let device = manager.device(&seat, &qh);

    let mut state = State {
        connection: connection.clone(),
        events: events.clone(),
        select_mimes: config.select_mimes,
        max_total_bytes: config.max_total_bytes,
        announced: HashMap::new(),
        transfer: None,
        owned: None,
        owned_primary: None,
        starting: true,
        stop: None,
    };
    let wayland_error = |error: &dyn std::fmt::Display| format!("the Wayland connection failed: {error}");
    queue.roundtrip(&mut state).map_err(|error| wayland_error(&error))?;
    state.starting = false;
    let mut wake = File::from(wake);

    loop {
        if let Some(result) = state.stop.take() {
            return result;
        }
        connection.flush().map_err(|error| wayland_error(&error))?;
        let Some(guard) = queue.prepare_read() else {
            queue.dispatch_pending(&mut state).map_err(|error| wayland_error(&error))?;
            continue;
        };

        let timeout = state.read_deadline().map(|deadline| {
            let left = deadline.saturating_duration_since(Instant::now());
            Timespec { tv_sec: left.as_secs() as i64, tv_nsec: left.subsec_nanos() as _ }
        });
        let (wayland_ready, wake_ready, pipe_ready) = {
            let pipe = state.transfer.as_ref().and_then(|transfer| transfer.current.as_ref()).map(|read| &read.pipe);
            let wayland = guard.connection_fd();
            let mut fds = vec![
                PollFd::new(&wayland, PollFlags::IN),
                PollFd::new(&wake, PollFlags::IN),
            ];
            if let Some(pipe) = pipe {
                fds.push(PollFd::new(pipe, PollFlags::IN));
            }
            match poll(&mut fds, timeout.as_ref()) {
                Ok(_) => {}
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(format!("poll failed: {error}")),
            }
            let ready = |fd: &PollFd| !fd.revents().is_empty();
            (ready(&fds[0]), ready(&fds[1]), fds.get(2).is_some_and(ready))
        };

        if wayland_ready {
            guard.read().map_err(|error| wayland_error(&error))?;
        } else {
            drop(guard);
        }
        queue.dispatch_pending(&mut state).map_err(|error| wayland_error(&error))?;

        if pipe_ready {
            state.pipe_readable();
        }
        if state.read_deadline().is_some_and(|deadline| Instant::now() >= deadline) {
            log::debug!("clipboard read timed out");
            state.abandon_transfer();
        }

        if wake_ready {
            let mut drained = [0u8; 64];
            let _ = wake.read(&mut drained);
            while let Ok(command) = commands.try_recv() {
                match command {
                    Command::Stop => return Ok(()),
                    Command::Set { mimes, done } => {
                        let result = set_selection(&mut state, &mut queue, &manager, &device, mimes);
                        let _ = done.send(result.map_err(|error| wayland_error(&error)));
                    }
                    Command::MirrorToPrimary { done } => {
                        let _ = done.send(mirror_to_primary(&mut state, &mut queue, &manager, &device));
                    }
                }
            }
        }
    }
}

fn set_selection(
    state: &mut State,
    queue: &mut EventQueue<State>,
    manager: &Manager,
    device: &Device,
    mimes: MimeData,
) -> Result<(), wayland_client::DispatchError> {
    let previous = state.owned.take();
    if mimes.is_empty() {
        device.set_selection(None);
    } else {
        let source = manager.source(&queue.handle());
        let mut data: HashMap<String, Vec<u8>> = mimes.into_iter().collect();
        data.insert(ORIGIN_MIME.to_owned(), b"1".to_vec());
        for mime in data.keys() {
            source.offer(mime);
        }
        device.set_selection(Some(&source));
        state.owned = Some(OwnedSelection { source, data: Arc::new(data) });
    }
    if let Some(previous) = previous {
        previous.source.destroy();
    }
    queue.roundtrip(state).map(|_| ())
}

fn mirror_to_primary(
    state: &mut State,
    queue: &mut EventQueue<State>,
    manager: &Manager,
    device: &Device,
) -> Result<(), String> {
    if !device.has_primary_selection() {
        return Err("the compositor has no primary selection".into());
    }
    let Some(clipboard) = &state.owned else {
        return Err("Insert does not own the clipboard".into());
    };
    let data = Arc::clone(&clipboard.data);
    let source = manager.source(&queue.handle());
    for mime in data.keys() {
        source.offer(mime);
    }
    device.set_primary_selection(&source);
    if let Some(previous) = state.owned_primary.replace(OwnedSelection { source, data }) {
        previous.source.destroy();
    }
    queue.roundtrip(state).map(|_| ()).map_err(|error| format!("the Wayland connection failed: {error}"))
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(_: &mut Self, _: &WlSeat, _: <WlSeat as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

/// The two data-control protocols have the same requests and events under different names.
macro_rules! data_control_dispatch {
    ($variant:ident, $manager:ty, $device_mod:ident :: $device:ty, $offer_mod:ident :: $offer:ty, $source_mod:ident :: $source:ty) => {
        impl Dispatch<$manager, ()> for State {
            fn event(
                _: &mut Self,
                _: &$manager,
                _: <$manager as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }

        impl Dispatch<$device, ()> for State {
            fn event(
                state: &mut Self,
                _: &$device,
                event: $device_mod::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                match event {
                    $device_mod::Event::DataOffer { id } => {
                        let object = id.id();
                        state.offer_announced(Offer::$variant(id), object);
                    }
                    $device_mod::Event::Selection { id } => state.selection_changed(id.map(|offer| offer.id())),
                    $device_mod::Event::PrimarySelection { id } => state.drop_offer(id.map(|offer| offer.id())),
                    $device_mod::Event::Finished => {
                        state.stop = Some(Err("the compositor removed the clipboard device".into()));
                    }
                    _ => {}
                }
            }

            event_created_child!(State, $device, [
                $device_mod::EVT_DATA_OFFER_OPCODE => ($offer, ()),
            ]);
        }

        impl Dispatch<$offer, ()> for State {
            fn event(
                state: &mut Self,
                offer: &$offer,
                event: $offer_mod::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                if let $offer_mod::Event::Offer { mime_type } = event {
                    state.offer_mime(&offer.id(), mime_type);
                }
            }
        }

        impl Dispatch<$source, ()> for State {
            fn event(
                state: &mut Self,
                source: &$source,
                event: $source_mod::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                match event {
                    $source_mod::Event::Send { mime_type, fd } => state.source_send(&source.id(), mime_type, fd),
                    $source_mod::Event::Cancelled => state.source_cancelled(&source.id()),
                    _ => {}
                }
            }
        }
    };
}

data_control_dispatch!(
    Ext,
    ExtDataControlManagerV1,
    ext_data_control_device_v1::ExtDataControlDeviceV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1,
    ext_data_control_source_v1::ExtDataControlSourceV1
);
data_control_dispatch!(
    Wlr,
    ZwlrDataControlManagerV1,
    zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
    zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
    zwlr_data_control_source_v1::ZwlrDataControlSourceV1
);
