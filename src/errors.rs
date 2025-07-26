use thiserror::Error;
use trouble_host::{BleHostError, Error};

#[derive(Error, Debug)]
pub enum CentralError<E>
where
    E: core::fmt::Debug, // Bound only on the inner error type
{
    #[error("Failed to instantiate scanner")]
    ScanInstantiationError(),

    #[error("Failed to enumerate services for {0}: {1:?}")]
    ServicesEnumerationError(&'static str, BleHostError<E>),

    #[error("{0} service not found")]
    ServiceNotFoundError(&'static str),

    #[error("{0} characteristic not found: {1:?}")]
    CharacteristicNotFoundError(&'static str, BleHostError<E>),

    #[error("Could not write to {0} characteristic: {1:?}")]
    CharacteristicWriteError(&'static str, BleHostError<E>),

    #[error("Could not instantiate {0} listener: {1:?}")]
    ListenerInstantiationError(&'static str, BleHostError<E>),
}

#[derive(Error, Debug)]
pub enum PeripheralError<E>
where
    E: core::fmt::Debug, // Bound only on the inner error type
{
    #[error("Failed to create advertiser: {0:?}")]
    AdvertiserError(BleHostError<E>),

    #[error("Failed to parse AdStruture data")]
    AdStructureError,

    #[error("Failed to create connection: {0:?}")]
    ConnectionError(Error),

    #[error("Failed to create connection: {0:?}")]
    GattConnectionError(Error),
}
