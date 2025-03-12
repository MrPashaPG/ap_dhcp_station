use embassy_executor::Spawner;
use embassy_net::{Runner, StackResources};
use embassy_time::{Duration, Timer};
use esp_println::println;
use esp_wifi::wifi::{WifiDevice, WifiStaDevice};

macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write($val);
        x
    }};
}

#[embassy_executor::task]
pub async fn run_station(spawner: Spawner, wifi_interface: WifiDevice<'static, WifiStaDevice>) {
    let config = embassy_net::Config::dhcpv4(Default::default());

    let seed = 0x12345678_u64;

    let (stack, runner) = embassy_net::new(
        wifi_interface.into(),
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );

    spawner.spawn(net_task(runner)).ok();

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(cfg) = stack.config_v4() {
            println!("Got IP: {}", cfg.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    loop {
        Timer::after(Duration::from_millis(1_000)).await;
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    runner.run().await
}
