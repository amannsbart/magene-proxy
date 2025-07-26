use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Watch;
use trouble_host::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceState {
    Disconnected,
    Scanning,
    Connecting,
    Connected,
}

// Channel declarations
pub static SCAN_CHANNEL: Channel<CriticalSectionRawMutex, Address, 32> = Channel::new();
pub static RADAR_DATA_WATCH: Watch<CriticalSectionRawMutex, Option<[u8; 16]>, 2> = Watch::new();
pub static BATTERY_DATA_WATCH: Watch<CriticalSectionRawMutex, Option<[u8; 1]>, 2> = Watch::new();
pub static CLIENT_STATE_WATCH: Watch<CriticalSectionRawMutex, ClientState, 4> = Watch::new();
pub static SOURCE_STATE_WATCH: Watch<CriticalSectionRawMutex, SourceState, 4> = Watch::new();
