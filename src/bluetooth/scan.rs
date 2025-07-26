use crate::config::TARGET_NAME;
use crate::errors::CentralError;
use crate::messages::{SourceState, SCAN_CHANNEL, SOURCE_STATE_WATCH};

use bt_hci::cmd::le::LeSetScanParams;
use bt_hci::controller::ControllerCmdSync;
use core::{u8, usize};
use embassy_time::Duration;
use embedded_io::ErrorType;
use log::*;
use trouble_host::prelude::{Central, EventHandler, ScanConfig};
use trouble_host::scan::{LeAdvReportsIter, Scanner};
use trouble_host::{Address, Controller, PacketPool};

fn parse_local_name(data: &[u8]) -> Option<&str> {
    let mut i = 0;
    while i < data.len() {
        let length = data[i] as usize;
        if length == 0 || i + length >= data.len() {
            break;
        }
        let ad_type = data[i + 1];
        let ad_data = &data[i + 2..i + 1 + length];

        if ad_type == 0x08 || ad_type == 0x09 {
            if let Ok(name) = core::str::from_utf8(ad_data) {
                return Some(name);
            }
        }
        i += length + 1;
    }
    None
}

pub struct ScanEventHandler;

impl EventHandler for ScanEventHandler {
    fn on_adv_reports(&self, mut it: LeAdvReportsIter<'_>) {
        while let Some(Ok(report)) = it.next() {
            if let Some(local_name) = parse_local_name(report.data) {
                if local_name == TARGET_NAME {
                    match SCAN_CHANNEL.try_send(Address {
                        kind: report.addr_kind,
                        addr: report.addr,
                    }) {
                        Ok(_) => {}
                        Err(e) => {
                            error!("[Central] Could not send scan result to channel: {:?}", e)
                        }
                    }
                }
            }
        }
    }
}

pub async fn scan<'a, C, P>(
    mut central: Central<'a, C, P>,
) -> Result<(Address, Central<'a, C, P>), (CentralError<<C as ErrorType>::Error>, Central<'a, C, P>)>
where
    C: Controller + ControllerCmdSync<LeSetScanParams>,
    P: PacketPool,
{
    info!("[Central] Starting scanning for device");
    let sender = SOURCE_STATE_WATCH.sender();
    sender.send(SourceState::Scanning);

    let target: Address;

    let receiver = SCAN_CHANNEL.receiver();

    let mut scanner = Scanner::new(central);
    let mut scan_config = ScanConfig::default();
    scan_config.active = true;
    scan_config.interval = Duration::from_secs(1);
    scan_config.window = Duration::from_secs(1);

    let _scan_session = scanner.scan(&scan_config).await;

    if _scan_session.is_err() {
        drop(_scan_session);
        central = scanner.into_inner();
        return Err((CentralError::ScanInstantiationError(), central));
    }

    let device = receiver.receive().await;

    info!("[Central] Device found: {:?}", device.addr.into_inner());
    target = device;
    drop(_scan_session);
    central = scanner.into_inner();
    return Ok((target, central));
}
