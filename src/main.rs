#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, rng::Rng, timer::timg::TimerGroup};

mod wifi;
use wifi::wifi_controller;

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    let config: esp_hal::Config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(62 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let rng = Rng::new(peripherals.RNG);

    spawner
        .spawn(wifi_controller::init_wifi(
            spawner,
            peripherals.WIFI,
            peripherals.TIMG1,
            rng.clone(),
            peripherals.RADIO_CLK,
        ))
        .unwrap();

    loop {
        Timer::after(Duration::from_secs(5)).await;
    }
}
