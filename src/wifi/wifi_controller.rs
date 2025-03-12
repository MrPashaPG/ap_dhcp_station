use embassy_time::{Duration, Timer};
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use esp_wifi::wifi::{
    AccessPointConfiguration, ClientConfiguration, Configuration, WifiController, WifiEvent,
    WifiState,
};
use esp_wifi::{init, EspWifiController};

use super::access_point::run_ap;
use super::station::run_station;

macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write($val);
        x
    }};
}

#[embassy_executor::task]
pub async fn init_wifi(
    spawner: embassy_executor::Spawner,
    wifi: esp_hal::peripherals::WIFI,
    timg1: esp_hal::peripherals::TIMG1,
    rng: esp_hal::rng::Rng,
    radio_clk: esp_hal::peripherals::RADIO_CLK,
) {
    let timg1 = TimerGroup::new(timg1);

    let wifi: esp_hal::peripherals::WIFI = wifi;

    let init = &*mk_static!(
        EspWifiController<'static>,
        init(timg1.timer0, rng, radio_clk).unwrap()
    );

    let (ap_interface, sta_interface, ap_sta_controller) =
        esp_wifi::wifi::new_ap_sta(&init, wifi).expect("Failed to init AP/STA mode");

    spawner.spawn(connection(ap_sta_controller)).unwrap();

    spawner.spawn(run_station(spawner, sta_interface)).unwrap();
    spawner.spawn(run_ap(spawner, ap_interface)).unwrap();

    // spawner.spawn(mqtt_client::run_mqtt_client()).unwrap();
    loop {
        Timer::after(Duration::from_millis(5000)).await;
    }
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    println!("Start wifi connection task");
    println!(
        "Device capabilities: {:?}",
        controller.capabilities().unwrap()
    );
    loop {
        match esp_wifi::wifi::wifi_state() {
            WifiState::StaConnected => {
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await;
            }
            WifiState::ApStarted => {
                controller.wait_for_event(WifiEvent::ApStop).await;
                Timer::after(Duration::from_millis(5000)).await;
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let ap_config = Configuration::AccessPoint(AccessPointConfiguration {
                ssid: "esp-wifi".try_into().unwrap(),
                password: "12345678".try_into().unwrap(),
                auth_method: esp_wifi::wifi::AuthMethod::WPA2Personal,
                ..Default::default()
            });
            controller.set_configuration(&ap_config).unwrap();
            println!("Access Point configuration set!");
            controller.start_async().await.unwrap();
            println!("WiFi started!");
        }
        if !matches!(controller.is_connected(), Ok(true)) {
            Timer::after(Duration::from_millis(5000)).await;
            let (desired_ssid, desired_password) = get_wifi_credentials();

            let mut found_ap = None;
            match controller.scan_n_async::<8>().await {
                Ok(scan_result) => {
                    println!("Available networks:");
                    for ap in scan_result.0 {
                        println!(
                            "SSID: {}, AuthMethod: {:#?}, SignalStrength: {}",
                            ap.ssid,
                            ap.auth_method.unwrap(),
                            gui_signal_strength(ap.signal_strength),
                        );
                        if ap.ssid == desired_ssid {
                            found_ap = Some(ap);
                        }
                    }
                }
                Err(e) => println!("Failed to scan for networks: {e:?}"),
            }
            if let Some(ap_info) = found_ap {
                println!(
                    "Desired network '{}' found with signal strength: {}",
                    desired_ssid,
                    gui_signal_strength(ap_info.signal_strength)
                );
                let client_config = Configuration::Client(ClientConfiguration {
                    ssid: desired_ssid.try_into().unwrap(),
                    password: desired_password.try_into().unwrap(),
                    ..Default::default()
                });
                controller.set_configuration(&client_config).unwrap();
                println!("Connecting to WiFi...");
            } else {
                println!("Desired network '{}' not found in scan list.", desired_ssid);
                continue;
            }
        }
        println!("About to connect...");
        match controller.connect_async().await {
            Ok(_) => {
                println!("WiFi connected!");

                Timer::after(Duration::from_millis(1000)).await;
            }
            Err(e) => {
                println!("Failed to connect to WiFi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await;
            }
        }
    }
}

fn get_wifi_credentials() -> (&'static str, &'static str) {
    ("wifi_name", "wifi_password")
}

fn gui_signal_strength(signal_strength: i8) -> &'static str {
    let adjusted_signal_strength = signal_strength / -30;
    let signal_gui = ["    ", "▁   ", "▁▃  ", "▁▃▅ ", "▁▃▅▇"];
    signal_gui[4 - adjusted_signal_strength as usize]
}
