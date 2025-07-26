use embassy_time::Duration;
use trouble_host::prelude::*;

// Configuration constants
pub const LOG_LEVEL: log::LevelFilter = log::LevelFilter::Info;
pub const TARGET_NAME: &str = "34660-5";
pub const DISCOVERY_DELAY: Duration = Duration::from_millis(2000);
pub const RADAR_DATA_PAGE_TIMEOUT: Duration = Duration::from_secs(5);

// Max connections and channels
pub const CONNECTIONS_MAX: usize = 4;
pub const L2CAP_CHANNELS_MAX: usize = 4;
pub const MAX_SERVICES: usize = 10;

// BLE UUIDs
pub const TARGET_RADAR_SERVICE: u128 = 0xf364140000b04240ba5005ca45bf8abc;
pub const TARGET_RADAR_DATA_CHARACTERISTIC: u128 = 0xf364140100b04240ba5005ca45bf8abc;
pub const TARGET_BATTERY_SERVICE: u16 = 0x180F;
pub const TARGET_BATTERY_LEVEL_CHARACTERISTIC: u16 = 0x2A19;

// 16-bit UUIDs as u16
pub const BATTERY_SERVICE: u16 = 0x180F;
pub const BATTERY_LEVEL_CHARACTERISTIC: u16 = 0x2A19;

// 128-bit UUIDs as u128
pub const RADARLIGHT_SERVICE: u128 = 0x8ce5cc010a4d11e9ab14d663bd873d93;
pub const RADARLIGHT_CHARACTERISTIC: u128 = 0x8ce5cc020a4d11e9ab14d663bd873d93;

// Magic bytes for radar activation
pub const RADAR_ACTIVATION_BYTES: [u8; 3] = [0x57, 0x09, 0x01];

//GATT Server config

#[gatt_service(uuid = TARGET_RADAR_SERVICE.to_le_bytes())]
pub struct RadarService {
    #[characteristic(uuid = TARGET_RADAR_DATA_CHARACTERISTIC.to_le_bytes(), read, notify)]
    pub radar_data: [u8; 16],
}

#[gatt_service(uuid = TARGET_BATTERY_SERVICE.to_le_bytes())]
pub struct BatteryService {
    #[characteristic(uuid = TARGET_BATTERY_LEVEL_CHARACTERISTIC.to_le_bytes(), read, notify)]
    pub battery_level: [u8; 1],
}

#[gatt_server]
pub struct Server {
    pub radar_service: RadarService,
    pub battery_service: BatteryService,
}
