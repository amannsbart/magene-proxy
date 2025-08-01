use embassy_futures::select::{select3, Either3};
use embassy_time::{Duration, Timer};
use esp_hal::rmt::{RawChannelAccess, TxChannelInternal};
use esp_hal_smartled::SmartLedsAdapter;
use smart_leds::{
    brightness,
    colors::{self},
    SmartLedsWrite as _, RGB,
};

use crate::messages::{ClientState, SourceState, CLIENT_STATE_WATCH, SOURCE_STATE_WATCH};

struct LedDropGuard<'a, TX, const BUFFER_SIZE: usize>
where
    TX: RawChannelAccess + TxChannelInternal + 'static,
{
    led: &'a mut SmartLedsAdapter<TX, BUFFER_SIZE>,
}

impl<'a, TX, const BUFFER_SIZE: usize> LedDropGuard<'a, TX, BUFFER_SIZE>
where
    TX: RawChannelAccess + TxChannelInternal + 'static,
{
    fn new(led: &'a mut SmartLedsAdapter<TX, BUFFER_SIZE>) -> Self {
        Self { led }
    }

    fn led(&mut self) -> &mut SmartLedsAdapter<TX, BUFFER_SIZE> {
        self.led
    }
}

impl<'a, TX, const BUFFER_SIZE: usize> Drop for LedDropGuard<'a, TX, BUFFER_SIZE>
where
    TX: RawChannelAccess + TxChannelInternal + 'static,
{
    fn drop(&mut self) {
        let _ = self.led.write(brightness([colors::BLACK].into_iter(), 0));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LEDPattern {
    client_state: ClientState,
    source_state: SourceState,
}

impl LEDPattern {
    pub fn new() -> Self {
        Self {
            client_state: ClientState::Disconnected,
            source_state: SourceState::Disconnected,
        }
    }

    pub fn set_client_state(&mut self, client_state: ClientState) {
        self.client_state = client_state
    }

    pub fn set_source_state(&mut self, source_state: SourceState) {
        self.source_state = source_state
    }

    pub fn get_color(&self) -> RGB<u8> {
        match self.client_state {
            ClientState::Connected => colors::AZURE,
            ClientState::Disconnected => colors::YELLOW,
        }
    }

    pub fn get_timer(&self) -> Timer {
        match self.source_state {
            SourceState::Disconnected => Timer::after(Duration::from_secs(3600)),
            SourceState::Scanning => Timer::after(Duration::from_millis(500)),
            SourceState::Connecting => Timer::after(Duration::from_millis(1000)),
            SourceState::Connected => Timer::after(Duration::from_millis(2500)),
        }
    }

    pub fn get_level(&self) -> u8 {
        match self.source_state {
            SourceState::Disconnected => 31,
            SourceState::Scanning => 31,
            SourceState::Connecting => 31,
            SourceState::Connected => 31,
        }
    }
}

pub async fn led_task<TX, const BUFFER_SIZE: usize>(led: &mut SmartLedsAdapter<TX, BUFFER_SIZE>)
where
    TX: RawChannelAccess + TxChannelInternal + 'static,
{
    // Create the drop guard - this will automatically turn off LED when function exits
    let mut led_guard = LedDropGuard::new(led);

    let mut current_pattern = LEDPattern::new();
    let mut client_receiver = CLIENT_STATE_WATCH
        .receiver()
        .expect("[LED] Client Watch receiver returned None - watch not initialized");

    let mut source_receiver = SOURCE_STATE_WATCH
        .receiver()
        .expect("[LED]Source Watch receiver returned None - watch not initialized");

    loop {
        match select3(
            client_receiver.changed(),
            source_receiver.changed(),
            current_pattern.get_timer(),
        )
        .await
        {
            Either3::First(state) => {
                current_pattern.set_client_state(state);
            }
            Either3::Second(state) => {
                current_pattern.set_source_state(state);
            }
            Either3::Third(_) => {
                let color = current_pattern.get_color();
                let brightness_level = current_pattern.get_level();

                led_guard
                    .led()
                    .write(brightness([color].into_iter(), brightness_level))
                    .unwrap();
                Timer::after(Duration::from_millis(200)).await;
                led_guard
                    .led()
                    .write(brightness([colors::BLACK].into_iter(), brightness_level))
                    .unwrap();
            }
        }
    }
}
