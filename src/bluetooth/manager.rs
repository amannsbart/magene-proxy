use crate::bluetooth::central::ble_central_task;
use crate::bluetooth::peripheral::ble_peripheral_task;
use crate::config::Server;

use bt_hci::cmd::le::LeSetScanParams;
use bt_hci::controller::ControllerCmdSync;
use embassy_futures::select::{select, Either};
use log::*;
use trouble_host::prelude::{Central, DefaultPacketPool, Peripheral};
use trouble_host::{Controller, PacketPool, Stack};

pub async fn ble_manager_task<'a, 'server, C, P>(
    central: Central<'a, C, P>,
    stack: &'a Stack<'a, C, P>,
    server: &'server Server<'a>,
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
) where
    C: Controller + ControllerCmdSync<LeSetScanParams>,
    P: PacketPool,
{
    match select(
        ble_central_task(central, stack),
        ble_peripheral_task(server, peripheral),
    )
    .await
    {
        Either::First(_) => info!("[Manager] BLE central task ended."),
        Either::Second(_) => info!("[Manager] BLE peripheral task ended."),
    }
}
