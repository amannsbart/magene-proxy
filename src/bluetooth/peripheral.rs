use embassy_futures::select::{select3, Either3};
use embedded_io::ErrorType;
use log::*;
use trouble_host::{
    gatt::{GattConnection, GattConnectionEvent},
    prelude::{
        AdStructure, Advertisement, DefaultPacketPool, Peripheral, BR_EDR_NOT_SUPPORTED,
        LE_GENERAL_DISCOVERABLE,
    },
    Controller, PacketPool,
};

use crate::{
    config::{Server, BATTERY_SERVICE},
    errors::PeripheralError,
    messages::{ClientState, BATTERY_DATA_WATCH, CLIENT_STATE_WATCH, RADAR_DATA_WATCH},
};

async fn advertise<'values, 'server, C>(
    name: &'values str,
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<
    GattConnection<'values, 'server, DefaultPacketPool>,
    PeripheralError<<C as ErrorType>::Error>,
>
where
    C: Controller,
{
    let mut advertiser_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[BATTERY_SERVICE.to_le_bytes()]),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )
    .map_err(|_| PeripheralError::AdStructureError)?;

    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &[],
            },
        )
        .await
        .map_err(|e| PeripheralError::AdvertiserError(e))?;

    info!("[Peripheral] BLE advertising started...");

    let connection = advertiser
        .accept()
        .await
        .map_err(|e| PeripheralError::ConnectionError(e))?;

    let gatt_connection = connection
        .with_attribute_server(server)
        .map_err(|e| PeripheralError::GattConnectionError(e))?;

    let sender = CLIENT_STATE_WATCH.sender();
    sender.send(ClientState::Connected);

    info!("[Peripheral] Client device connection established");
    Ok(gatt_connection)
}

async fn gatt_events_task<'a, 'server, P: PacketPool>(gatt_connection: &GattConnection<'_, '_, P>) {
    let reason = loop {
        match gatt_connection.next().await {
            GattConnectionEvent::Disconnected { reason } => {
                let sender = CLIENT_STATE_WATCH.sender();
                sender.send(ClientState::Disconnected);
                break reason;
            }
            GattConnectionEvent::Gatt { event } => {
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => warn!("[Peripheral] Error sending GATT response: {:?}", e),
                };
            }
            _ => {}
        }
    };
    info!("[Peripheral] GATT connection disconnected: {:?}", reason);
}

async fn gatt_battery_task<P: PacketPool>(
    server: &Server<'_>,
    gatt_connection: &GattConnection<'_, '_, P>,
) {
    let mut receiver = BATTERY_DATA_WATCH
        .receiver()
        .expect("[Peripheral] Watch receiver returned None - watch not initialized");
    loop {
        let level = match receiver.changed().await {
            Some(level) => level,
            None => [0u8; 1],
        };

        if let Err(e) = server
            .battery_service
            .battery_level
            .notify(gatt_connection, &level)
            .await
        {
            error!("[Peripheral] Could not send battery notification: {:?}", e);
        }
    }
}

async fn gatt_radar_task<P: PacketPool>(
    server: &Server<'_>,
    gatt_connection: &GattConnection<'_, '_, P>,
) {
    let mut receiver = RADAR_DATA_WATCH
        .receiver()
        .expect("[Peripheral] Watch receiver returned None - watch not initialized");

    loop {
        let data = match receiver.changed().await {
            Some(data) => data,
            None => [0u8; 16],
        };

        if let Err(e) = server
            .radar_service
            .radar_data
            .notify(gatt_connection, &data)
            .await
        {
            error!("[Peripheral] Could not send battery notification: {:?}", e);
        }
    }
}

pub async fn ble_peripheral_task<'a, 'server, C>(
    server: &'server Server<'a>,
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
) where
    C: Controller,
{
    info!("[Peripheral] Starting advertising and GATT service");
    loop {
        match advertise("RadarProxy", peripheral, &server).await {
            Ok(gatt_connection) => {
                match select3(
                    gatt_events_task(&gatt_connection),
                    gatt_radar_task(&server, &gatt_connection),
                    gatt_battery_task(&server, &gatt_connection),
                )
                .await
                {
                    Either3::First(_) => {
                        info!("[Peripheral] Gatt Event Task ended.")
                    }
                    Either3::Second(_) => {
                        info!("[Peripheral] Gatt Radar Task ended.")
                    }
                    Either3::Third(_) => {
                        info!("[Peripheral] Gatt battery Task ended.")
                    }
                }
            }
            Err(e) => {
                error!("{:?}", e);
                continue;
            }
        }
    }
}
