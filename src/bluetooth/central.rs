use core::{u8, usize};

use super::scan::scan;
use super::utils::PageBuffer;

use crate::config::{
    BATTERY_LEVEL_CHARACTERISTIC, BATTERY_SERVICE, RADARLIGHT_CHARACTERISTIC, RADARLIGHT_SERVICE,
    RADAR_ACTIVATION_BYTES, RADAR_DATA_PAGE_TIMEOUT,
};
use crate::config::{DISCOVERY_DELAY, MAX_SERVICES};
use crate::errors::CentralError;

use crate::messages::{
    ClientState, SourceState, BATTERY_DATA_WATCH, CLIENT_STATE_WATCH, RADAR_DATA_WATCH,
    SOURCE_STATE_WATCH,
};

use embassy_futures::select::{select, Either};
use embassy_time::Timer;
use embedded_io::ErrorType;

use log::*;
use trouble_host::gatt::{GattClient, NotificationListener};
use trouble_host::prelude::{Characteristic, Connection, ConnectionEvent, Uuid};
use trouble_host::{Controller, PacketPool};

use bt_hci::cmd::le::LeSetScanParams;
use bt_hci::controller::ControllerCmdSync;
use embassy_futures::select::{select3, Either3};
use trouble_host::prelude::{Central, ConnectConfig, ScanConfig};
use trouble_host::{Address, Stack};

async fn radarlight_notification_task<'a, const MTU: usize>(
    listener: &mut NotificationListener<'a, MTU>,
) {
    let mut receiver = CLIENT_STATE_WATCH
        .receiver()
        .expect("[Central] Watch receiver returned None - watch not initialized");
    let sender = RADAR_DATA_WATCH.sender();
    let mut page_buffer = PageBuffer::new(RADAR_DATA_PAGE_TIMEOUT);

    loop {
        receiver
            .changed_and(|&value| value == ClientState::Connected)
            .await;

        select(
            receiver.changed_and(|&value| value == ClientState::Disconnected),
            async {
                loop {
                    match select(listener.next(), page_buffer.get_timer()).await {
                        Either::First(notification) => {
                            let data = notification.as_ref();
                            if data.len() == 11 {
                                let mut page: [u8; 8] = [0u8; 8];
                                page.copy_from_slice(&data[3..]);

                                match page.get(0) {
                                    Some(0x30) => {
                                        page_buffer.set_page1(page);
                                        let value = page_buffer.get();
                                        sender.send(value);
                                    }
                                    Some(0x31) => {
                                        page_buffer.set_page2(page);
                                        let value = page_buffer.get();
                                        sender.send(value);
                                    }
                                    _ => {
                                        warn!(
                                            "[Central] Radar notification: unknown page type {:?}",
                                            page.as_slice()
                                        );
                                    }
                                }
                            } else {
                                warn!(
                                "[Central] Radar notification: wrong length (got {}, expected 11)",
                                data.len()
                            );
                            }
                        }
                        Either::Second(_) => {
                            info!("[Central] Radar data timeout");
                            let value = page_buffer.get();
                            sender.send(value)
                        }
                    }
                }
            },
        )
        .await;

        page_buffer.cleanup();
    }
}

async fn battery_notification_task<'a, 'b, C, P, const MTU: usize, const MAX_SERVICES: usize>(
    listener: &mut NotificationListener<'b, MTU>,
    client: &'b GattClient<'a, C, P, MAX_SERVICES>,
    battery_characteristic: &Characteristic<u8>,
) where
    C: Controller,
    P: PacketPool,
{
    let mut receiver = CLIENT_STATE_WATCH
        .receiver()
        .expect("[Central] Watch receiver returned None - watch not initialized");
    let sender = BATTERY_DATA_WATCH.sender();

    loop {
        receiver
            .changed_and(|&value| value == ClientState::Connected)
            .await;

        let mut battery_buffer = [0u8; 64];

        let bytes_read = match client
            .read_characteristic(&battery_characteristic, &mut battery_buffer)
            .await
        {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("[Central] Couldnt read inital battery level: {:?}", e);
                sender.send(None);
                continue;
            }
        };

        if bytes_read == 1 {
            let mut battery_level: [u8; 1] = [0u8; 1];
            battery_level.copy_from_slice(&battery_buffer[..1]);

            sender.send(Some(battery_level));
        } else {
            warn!("[Central] Received malformed battery level buffer");
            sender.send(None);
            continue;
        }

        select(
            receiver.changed_and(|&value| value == ClientState::Disconnected),
            async {
                loop {
                    let notification = listener.next().await;
                    let data = notification.as_ref();
                    if data.len() == 1 {
                        let mut battery_level: [u8; 1] = [0u8; 1];
                        battery_level.copy_from_slice(&data[..]);

                        sender.send(Some(battery_level));
                    } else {
                        warn!(
                            "[Central] Battery notification: wrong length (got {}, expected 1)",
                            data.len()
                        );
                    }
                }
            },
        )
        .await;
        sender.send(None);
    }
}

async fn event_task<'a, P>(connection: &Connection<'a, P>)
where
    P: PacketPool,
{
    let reason = loop {
        match connection.next().await {
            ConnectionEvent::Disconnected { reason } => {
                break reason;
            }
            _ => {}
        }
    };
    info!(
        "[Central] Disconnected from source device, reason: {:?}",
        reason
    )
}

async fn subscription_task<'a, 'b, C, P, const MAX_SERVICES: usize>(
    client: &'b GattClient<'a, C, P, MAX_SERVICES>,
) -> Result<(), CentralError<<C as ErrorType>::Error>>
where
    C: Controller,
    P: PacketPool,
{
    let radarlight_services = client
        .services_by_uuid(&Uuid::from(RADARLIGHT_SERVICE))
        .await
        .map_err(|e| CentralError::ServicesEnumerationError("Radarlight", e))?;

    let radarlight_service = radarlight_services
        .first()
        .ok_or(CentralError::ServiceNotFoundError("Radarlight"))?
        .clone();

    let battery_services = client
        .services_by_uuid(&Uuid::from(BATTERY_SERVICE))
        .await
        .map_err(|e| CentralError::ServicesEnumerationError("Battery", e))?;

    let battery_service = battery_services
        .first()
        .ok_or(CentralError::ServiceNotFoundError("Battery"))?
        .clone();

    let radarlight_characteristic: Characteristic<[u8; 11]> = client
        .characteristic_by_uuid(&radarlight_service, &Uuid::from(RADARLIGHT_CHARACTERISTIC))
        .await
        .map_err(|e| CentralError::CharacteristicNotFoundError("Radarlight", e))?;

    let battery_characteristic: Characteristic<u8> = client
        .characteristic_by_uuid(&battery_service, &Uuid::from(BATTERY_LEVEL_CHARACTERISTIC))
        .await
        .map_err(|e| CentralError::CharacteristicNotFoundError("Battery", e))?;

    let mut radarlight_listener = client
        .subscribe(&radarlight_characteristic, false)
        .await
        .map_err(|e| CentralError::ListenerInstantiationError("Radarlight", e))?;

    let mut battery_listener = client
        .subscribe(&battery_characteristic, false)
        .await
        .map_err(|e| CentralError::ListenerInstantiationError("Battery", e))?;

    client
        .write_characteristic(&radarlight_characteristic, &RADAR_ACTIVATION_BYTES)
        .await
        .map_err(|e| CentralError::CharacteristicWriteError("Radarlight", e))?;

    let sender = SOURCE_STATE_WATCH.sender();
    sender.send(SourceState::Connected);

    match select(
        radarlight_notification_task(&mut radarlight_listener),
        battery_notification_task(&mut battery_listener, &client, &battery_characteristic),
    )
    .await
    {
        Either::First(_) => {
            info!("[Central] Radar notification task has ended.")
        }
        Either::Second(_) => {
            info!("[Central] Battery notification task has ended.")
        }
    }
    Ok(())
}

pub async fn ble_central_task<'a, 'server, C, P>(
    central: Central<'a, C, P>,
    stack: &'a Stack<'a, C, P>,
) where
    C: Controller + ControllerCmdSync<LeSetScanParams>,
    P: PacketPool,
{
    let mut internal_central: Central<'a, C, P>;
    internal_central = central;

    let mut internal_target: Address;
    loop {
        match scan(internal_central).await {
            Ok((target, central)) => {
                internal_target = target;
                internal_central = central
            }

            Err((error, central)) => {
                internal_central = central;
                error!("{:?}", error);
                continue;
            }
        };

        info!(
            "[Central] Connecting to source device {:?}",
            internal_target.addr.into_inner()
        );

        let sender = SOURCE_STATE_WATCH.sender();
        sender.send(SourceState::Connecting);

        let connection_config = ConnectConfig {
            connect_params: Default::default(),
            scan_config: ScanConfig {
                filter_accept_list: &[(internal_target.kind, &internal_target.addr)],
                ..Default::default()
            },
        };

        let connection = match internal_central.connect(&connection_config).await {
            Ok(conn) => conn,
            Err(e) => {
                error!("[Central] Error instantiating source connection {:?}", e);
                continue;
            }
        };

        let client = match GattClient::<C, P, MAX_SERVICES>::new(&stack, &connection).await {
            Ok(client) => client,
            Err(e) => {
                error!("[Central] Error instantiating source client {:?}", e);
                continue;
            }
        };

        Timer::after(DISCOVERY_DELAY).await;
        match select3(
            client.task(),
            subscription_task(&client),
            event_task(&connection),
        )
        .await
        {
            Either3::First(result) => match result {
                Ok(_) => info!("[Central] Client runner has ended."),
                Err(e) => error!("[Central] Client runner encountered an error: {:?}", e),
            },
            Either3::Second(result) => match result {
                Ok(_) => info!("[Central] Subscription task has ended."),
                Err(e) => error!("[Central] Subscription task encountered an error: {:?}", e),
            },
            Either3::Third(_) => {
                info!("[Central] Event task ended.")
            }
        };

        RADAR_DATA_WATCH.sender().send(None);
        BATTERY_DATA_WATCH.sender().send(None);
        sender.send(SourceState::Disconnected);
    }
}
