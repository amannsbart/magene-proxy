#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_futures::select::{select4, Either4};
use magene_proxy::bluetooth::{ble_manager_task, ScanEventHandler};
use magene_proxy::config::{Server, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX};
use magene_proxy::led::led_task;

use bt_hci::{controller::ExternalController, uuid::appearance};

use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Pull};
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{rmt::Rmt, time::Rate};

use esp_hal_smartled::{smart_led_buffer, SmartLedsAdapter};

use esp_wifi::ble::controller::BleConnector;

use log::*;

use embassy_executor::Spawner;

use esp_backtrace as _;
use trouble_host::Address;
use trouble_host::{
    gap::{GapConfig, PeripheralConfig},
    prelude::DefaultPacketPool,
    Host, HostResources,
};

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::_80MHz);
    let peripherals = esp_hal::init(config);

    let input_config = InputConfig::default().with_pull(Pull::Up);
    let mut user_button = Input::new(peripherals.GPIO41, input_config);

    let mut led = {
        let frequency = Rate::from_mhz(80);
        let rmt = Rmt::new(peripherals.RMT, frequency).expect("[Main] Failed to initialize RMT0");
        SmartLedsAdapter::new(rmt.channel0, peripherals.GPIO35, smart_led_buffer!(1))
    };

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);
    let rng = esp_hal::rng::Rng::new(peripherals.RNG);
    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let wifi_init = esp_wifi::init(timer1.timer0, rng)
        .expect("[Main] Failed to initialize WIFI/BLE controller");

    let transport = BleConnector::new(&wifi_init, peripherals.BT);
    let controller = ExternalController::<_, 20>::new(transport);
    let address = Address::random([0xff, 0x8f, 0x1b, 0x05, 0xe4, 0xff]);

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);

    let server = match Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "TrouBLE",
        appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
    })) {
        Ok(result) => result,
        Err(e) => {
            error!("[Main] Failed to setup GATT server: {:?}", e);
            return;
        }
    };
    info!("[Main] Setup complete....");
    loop {
        user_button.wait_for_falling_edge().await;
        info!("[Main] Starting main application");
        let Host {
            mut runner,
            central,
            mut peripheral,
            ..
        } = stack.build();

        match select4(
            runner.run_with_handler(&ScanEventHandler),
            led_task(&mut led),
            ble_manager_task(central, &stack, &server, &mut peripheral),
            user_button.wait_for_falling_edge(),
        )
        .await
        {
            Either4::First(result) => match result {
                Ok(()) => info!("[Main] Runner Task ended."),
                Err(e) => error!("[Main] Runner task encounterd an error: {:?}", e),
            },
            Either4::Second(_) => {
                info!("[Main] Led Task ended.")
            }
            Either4::Third(_) => {
                info!("[Main] BLE Manager Task ended.")
            }
            Either4::Fourth(_) => {
                info!("[Main] User button pressed")
            }
        };
        info!("[Main] Stopping main application - byebye");
    }
}
