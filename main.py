"""
BLE Proxy for Magene L508 Radar Device - Enhanced with Graceful Disconnect Handling
Raspberry Pi Pico W version using correct aioble patterns
"""

import sys
sys.path.append("")

import asyncio
import aioble
import bluetooth
import struct
import ubinascii
import gc
from machine import Pin, Timer

# Configuration
TARGET_NAME = "34660-5"  
SCAN_TIMEOUT = 10000      
RETRY_DELAY = 5000        
RADAR_DATA_PAGE_TIMEOUT = 2000
LED_PIN = "LED"

# Target Service and characteristic UUIDs
TARGET_RADAR_SERVICE = bluetooth.UUID('f3641400-00b0-4240-ba50-05ca45bf8abc')
TARGET_BATTERY_SERVICE = bluetooth.UUID('0000180f-0000-1000-8000-00805f9b34fb')
TARGET_RADAR_DATA_CHARACTERISTIC = bluetooth.UUID('f3641401-00b0-4240-ba50-05ca45bf8abc')
TARGET_BATTERY_LEVEL_CHARACTERISTIC = bluetooth.UUID('00002a19-0000-1000-8000-00805f9b34fb')

# Client Services & Characteristics
DEVICE_INFO_SERVICE = bluetooth.UUID(0x180a)
BATTERY_SERVICE = bluetooth.UUID(0x180f)
DEVICE_NAME_CHARACTERISTIC = bluetooth.UUID(0x2a00)
FIRMWARE_VERSION_CHARACTERISTIC = bluetooth.UUID(0x2a26)
MANUFACTURER_CHARACTERISTIC = bluetooth.UUID(0x2a29)
MODEL_CHARACTERISTIC = bluetooth.UUID(0x2a24)
BATTERY_LEVEL_CHARACTERISTIC = bluetooth.UUID(0x2a19)
RADARLIGHT_SERVICE = bluetooth.UUID('8ce5cc01-0a4d-11e9-ab14-d663bd873d93')
RADARLIGHT_CHARACTERISTIC = bluetooth.UUID('8ce5cc02-0a4d-11e9-ab14-d663bd873d93')

# Magic bytes for radar activation
RADAR_MAGIC_BYTES = b'\x57\x09\x01'

# Expected device validation
EXPECTED_MANUFACTURER = 'Qingdao Magene Intelligence Technology Co., Ltd'
EXPECTED_MODEL = '320'

# Advertisement interval
_ADV_INTERVAL_MS = 250_000

# Global state variables
current_radar_data_page1 = bytearray([0x30,00,00,00,00,00,00,00])
current_radar_data_page2 = bytearray([0x31,00,00,00,00,00,00,00])
radar_data_page1_timer = Timer(-1)
radar_data_page2_timer = Timer(-1)
current_battery_level = 0
source_connection = None
running = True
connection_state = "scanning"  # "scanning", "connected", "disconnected"
led_blink_interval = 0.5
led_blink_event = asyncio.Event()

#Initialize LED
led = Pin(LED_PIN, Pin.OUT)

# Register GATT server (global scope like the example)
radar_service = aioble.Service(TARGET_RADAR_SERVICE)
radar_characteristic = aioble.Characteristic(
    radar_service, 
    TARGET_RADAR_DATA_CHARACTERISTIC, 
    read=True, 
    notify=True
)

battery_service = aioble.Service(TARGET_BATTERY_SERVICE)
battery_characteristic = aioble.Characteristic(
    battery_service, 
    TARGET_BATTERY_LEVEL_CHARACTERISTIC, 
    read=True, 
    notify=True
)

# Register services globally
aioble.register_services(radar_service, battery_service)

print("BLE peripheral services registered")

# Helper functions
def clear_radar_data_page(number: int):
    global current_radar_data_page1, current_radar_data_page2
    """Clear radar data page"""
    if number == 1:
        current_radar_data_page1 = bytearray([0x30,00,00,00,00,00,00,00])
    elif number == 2:
        current_radar_data_page2 = bytearray([0x31,00,00,00,00,00,00,00])
    return 


def _encode_battery_level(level):
    """Encode battery level as single byte"""
    return struct.pack('B', level)

def update_connection_state(new_state):
    """Update connection state with logging"""
    global connection_state
    global led_blink_interval
    if connection_state != new_state:
        connection_state = new_state
        if new_state == "scanning":
            led_blink_interval = 0.5  
        elif new_state == "connected":
            led_blink_interval = 1
        elif new_state == "disconnected":
            led_blink_interval = 2.5

        led_blink_event.set()

async def scan_for_device():
    """Scan for target device"""
    print(f"Scanning for device: {TARGET_NAME}")
    update_connection_state("scanning")
    
    try:
        async with aioble.scan(
            duration_ms=SCAN_TIMEOUT,
            interval_us=30000,
            window_us=30000,
            active=True
        ) as scanner:
            async for result in scanner:
                device_name = result.name()
                if device_name and device_name == TARGET_NAME:
                    print(f"Found target device: {device_name} ({result.device})")
                    return result.device
                    
    except Exception as e:
        print(f"Scan error: {e}")
        
    return None

async def connect_to_source(device):
    """Connect to source device with enhanced error handling"""
    global source_connection
    
    print(f"Connecting to: {device}")
    
    try:
        # Reset connection state
        await disconnect_source()
        
        source_connection = await device.connect()
        update_connection_state("connected")
        print(f"Connected to source device: {device}")
        return True
        
    except Exception as e:
        print(f"Connection error: {e}")
        await disconnect_source()
        return False

async def validate_device():
    """Validate the connected device"""
    if not source_connection:
        return False
    
    try:
        # Get device info service
        device_info_service = await source_connection.service(DEVICE_INFO_SERVICE)
        
        if device_info_service is None:
            print("Device info service not available - cannot validate device")
            return False
        
        # Read and validate manufacturer
        try:
            manufacturer_char = await device_info_service.characteristic(MANUFACTURER_CHARACTERISTIC)
            if manufacturer_char is None:
                print("Manufacturer characteristic not found")
                return False
                
            manufacturer_data = await manufacturer_char.read()
            manufacturer = str(manufacturer_data, 'utf-8')
            
            if manufacturer != EXPECTED_MANUFACTURER:
                print(f"Manufacturer validation failed: expected '{EXPECTED_MANUFACTURER}', got '{manufacturer}'")
                return False
            else:
                print(f"Manufacturer validated: {manufacturer}")
                
        except Exception as e:
            print(f"Could not read manufacturer: {e}")
            return False
        
        # Read and validate model
        try:
            model_char = await device_info_service.characteristic(MODEL_CHARACTERISTIC)
            if model_char is None:
                print("Model characteristic not found")
                return False
                
            model_data = await model_char.read()
            model = str(model_data, 'utf-8').strip()
            
            if model != EXPECTED_MODEL:
                print(f"Model validation failed: expected '{EXPECTED_MODEL}', got '{model}'")
                return False
            else:
                print(f"Model validated: {model}")
                
        except Exception as e:
            print(f"Could not read model: {e}")
            return False
        
        print("Device validation completed successfully")
        return True
        
    except Exception as e:
        print(f"Device validation failed: {e}")
        return False

async def setup_notifications():
    """Setup notifications from source device"""
    global current_battery_level
    if not source_connection:
        return None, None
        
    radar_char = None
    battery_char = None
    
    try:
        # Setup radar notifications
        try:
            radar_service_src = await source_connection.service(RADARLIGHT_SERVICE)
            if radar_service_src:
                radar_char = await radar_service_src.characteristic(RADARLIGHT_CHARACTERISTIC)
                if radar_char:
                    await radar_char.subscribe(notify=True)
                    print("Radar notifications enabled")
                    
                    # Send radar activation command
                    await radar_char.write(RADAR_MAGIC_BYTES)
                    print("Radar activation command sent")
                else:
                    print("Radar characteristic not found")
            else:
                print("Radar service not found")
        except Exception as e:
            print(f"Failed to setup radar notifications: {e}")
            
        # Setup battery notifications
        try:
            battery_service_src = await source_connection.service(BATTERY_SERVICE)
            if battery_service_src:
                battery_char = await battery_service_src.characteristic(BATTERY_LEVEL_CHARACTERISTIC)
                if battery_char:
                    await battery_char.subscribe(notify=True)
                    print("Battery notifications enabled")
                    data = await battery_char.read()
                    try:
                        if len(data) == 1:
                            battery_level = data[0]
                            if battery_level != current_battery_level:
                                current_battery_level = battery_level
                                battery_characteristic.write(_encode_battery_level(battery_level), send_update=True)
                                print(f"Initial battery level: {battery_level}%")
                        else:
                            print(f"Unexpected battery data length: {len(data)}")
                            
                    except Exception as e:
                        print(f"Error handling battery notification: {e}")
                else:
                    print("Battery characteristic not found")
            else:
                print("Battery service not found")
        except Exception as e:
            print(f"Failed to setup battery notifications: {e}")
            
        return radar_char, battery_char
        
    except Exception as e:
        print(f"Failed to setup notifications: {e}")
        return None, None

async def handle_radar_notification(data):
    """Handle radar data notifications"""
    global current_radar_data_page1, current_radar_data_page2, radar_data_page1_timer, radar_data_page2_timer 
    
    try:
        # Validate radar frame - check if byte at position 3 is 0x30 or 0x31
        if len(data) >= 4 and (data[3] == 0x30 or data[3] == 0x31):
            # Cut the first 3 bytes and forward the rest
            processed_data = bytearray(data[3:])
           
            if processed_data[0] == 0x30:
                radar_data_page1_timer.deinit()
                current_radar_data_page1 = processed_data
                radar_data_page1_timer.init(mode=Timer.ONE_SHOT, period=RADAR_DATA_PAGE_TIMEOUT, callback=lambda t: clear_radar_data_page(1))
                
            elif processed_data[0] == 0x31:
                radar_data_page1_timer.deinit()
                current_radar_data_page2 = processed_data
                radar_data_page2_timer.init(mode=Timer.ONE_SHOT, period=RADAR_DATA_PAGE_TIMEOUT, callback=lambda t: clear_radar_data_page(2))
            
            radar_characteristic.write(current_radar_data_page1 + current_radar_data_page2, send_update=True)
        else:
            print(f"Non-radar frame received (length: {len(data)}): {ubinascii.hexlify(data).decode()}")
            
    except Exception as e:
        print(f"Error handling radar notification: {e}")

async def handle_battery_notification(data):
    """Handle battery level notifications"""
    global current_battery_level
    
    try:
        if len(data) == 1:
            battery_level = data[0]
            if battery_level != current_battery_level:
                current_battery_level = battery_level
                battery_characteristic.write(_encode_battery_level(battery_level), send_update=True)
                print(f"Battery level updated: {battery_level}%")
        else:
            print(f"Unexpected battery data length: {len(data)}")
            
    except Exception as e:
        print(f"Error handling battery notification: {e}")

async def source_device_task():
    """Handle connection to source radar device and data forwarding"""
    global source_connection, running, connection_state
    
    while running:
        try:
            device = None
            while (not device): 
                device = await scan_for_device()
                if not device:
                    print(f"Device not found, retrying in {RETRY_DELAY//1000} seconds...")
                    await asyncio.sleep_ms(RETRY_DELAY)

                
            # Connect to source
            connected = False
            while not connected:
                connected = await connect_to_source(device)
                if not connected:
                    print(f"Connection failed, retrying in {RETRY_DELAY//1000} seconds...")
                    await asyncio.sleep_ms(RETRY_DELAY)
                
            # Validate device
            if not await validate_device():
                print("Device validation failed")
                await disconnect_source()
                continue
                
            # Setup notifications
            radar_char, battery_char = await setup_notifications()
            if not radar_char or not battery_char:
                print("Failed to setup notifications")
                await disconnect_source()
                continue
            
            print("Source device operational - monitoring notifications...")
            
            # Monitor notifications with enhanced error handling
            try:
                while source_connection and source_connection.is_connected() and running:
                    try:
                        # Check for radar data
                        if radar_char and source_connection and source_connection.is_connected():
                            try:
                                radar_data = await asyncio.wait_for(
                                    radar_char.notified(), 
                                    timeout=0.1
                                )
                                await handle_radar_notification(radar_data)
                            except asyncio.TimeoutError:
                                pass
                            except (aioble.DeviceDisconnectedError, OSError) as e:
                                print(f"Radar characteristic disconnected: {e}")
                                break
                            except Exception as e:
                                print(f"Radar notification error: {e}")
                        
                        # Check for battery data
                        if battery_char and source_connection and source_connection.is_connected():
                            try:
                                battery_data = await asyncio.wait_for(
                                    battery_char.notified(), 
                                    timeout=0.1
                                )
                                await handle_battery_notification(battery_data)
                            except asyncio.TimeoutError:
                                pass
                            except (aioble.DeviceDisconnectedError, OSError) as e:
                                print(f"Battery characteristic disconnected: {e}")
                                break
                            except Exception as e:
                                print(f"Battery notification error: {e}")
                                
                        await asyncio.sleep_ms(50)
                        
                    except (aioble.DeviceDisconnectedError, OSError) as e:
                        print(f"Source device disconnected: {e}")
                        break
                    except Exception as e:
                        print(f"Unexpected error in notification loop: {e}")
                        break
                        
            except Exception as e:
                print(f"Error in notification monitoring: {e}")
            
            print("Source connection lost, falling back to scanning mode...")
            
        except Exception as e:
            print(f"Error in source device task: {e}")
        finally:
            await disconnect_source()
            await asyncio.sleep_ms(RETRY_DELAY)
            gc.collect()

async def disconnect_source():
    """Disconnect from source device with enhanced cleanup"""
    global source_connection
    
    if source_connection:
        try:
            print("Cleaning up...")
            await source_connection.disconnect()
            radar_characteristic.write(b'', send_update=True) 
            battery_characteristic.write(b'', send_update=True)
        except Exception as e:
            print(f"Error during disconnect cleanup: {e}")
        finally:
            source_connection = None
    
    update_connection_state("disconnected")

async def peripheral_task():
    """Handle BLE peripheral advertising and client connections"""
    while running:
        try:
            print("Starting BLE peripheral advertising...")
            
            async with await aioble.advertise(
                _ADV_INTERVAL_MS,
                name="RadarProxy",
                services=[TARGET_RADAR_SERVICE],
            ) as connection:
                print(f"Client connected: {connection.device}")
                
                await connection.disconnected(timeout_ms=None)
                
                print("Client disconnected")
                
        except Exception as e:
            print(f"Error in peripheral task: {e}")
            await asyncio.sleep_ms(1000)

async def led_blink_task():
    global led_blink_interval
    while True:
        led.on()
        await asyncio.sleep(0.2)
        led.off()
        try:
            await asyncio.wait_for(led_blink_event.wait(), timeout=led_blink_interval)
            led_blink_event.clear()
        except asyncio.TimeoutError:
            pass  # Timeout means no state change; just blink again

async def main():
    """Main function - run both tasks concurrently"""
    print("Starting BLE Radar Proxy with Enhanced Disconnect Handling...")

    # Initialize characteristics
    radar_characteristic.write(b'')
    battery_characteristic.write(_encode_battery_level(0))
    
    # Create tasks for source device handling and peripheral advertising
    t1 = asyncio.create_task(source_device_task())
    t2 = asyncio.create_task(peripheral_task())
    t3 = asyncio.create_task(led_blink_task())
    
    try:
        # Run both tasks concurrently
        await asyncio.gather(t1, t2, t3)
    except KeyboardInterrupt:
        print("Proxy stopped by user")
    finally:
        global running
        running = False
        await disconnect_source()

# Run the proxy
if __name__ == "__main__":
    asyncio.run(main())
