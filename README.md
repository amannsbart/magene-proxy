# BLE Radar Proxy for ESP32-S3 

A Bluetooth Low Energy (BLE) proxy that masks a Magene L508 rear bike radar as a different radar device with alternative Bluetooth implementation. This allows compatibility with cycling computers and apps that may not natively support the Magene L508's proprietary BLE protocol.

## Overview

This proxy acts as a BLE bridge, connecting to a Magene L508 radar device and re-advertising its data using a the radar protocol of the Bryton Gardia. The ESP32-S3 serves as an intermediary, translating between the Magene's proprietary format and a more universally compatible BLE radar implementation.

## Hardware Requirements

- **ESP32-S3 board**
    - Tested with M5Stack ATOM S3 Lite.
- **Magene L508 rear bike radar**
- **USB cable & power source**

*Note: While the README is written with ESP32-S3 in mind, the `esp-hal` framework is portable to other chips supported by [esp-hal](https://github.com/esp-rs/esp-hal) and [embassy](https://github.com/embassy-rs/embassy).*

## Development

*This project was automatically set up with the `esp-generate` template and follow its [setup instructions](https://github.com/esp-rs/esp-generate).*

1. **Clone this repository**
    ```
    git clone https://github.com/amannsbart/magene-proxy
    cd magene-proxy
    ```

2. **Launch the devcontainer & build for ESP32-S3**
    ```
    cargo build --release
    ```

3. **Connect ESP32-S3 development board**
    Make sure that you have sufficient permissions for the usb serial device (udev rule, etc.)


4. **Flash to device & monitor debug output**
    ```
    cargo run --release
    ```
    *(Substitute the correct USB port for your system)*


## License

This project is provided as-is for educational and personal use under the GPL v3 License. Please ensure compliance with local regulations regarding BLE device modification and cycling safety equipment.

Happy riding!