use core::{net::Ipv4Addr, str::FromStr};
use edge_dhcp::{
    io::{self, DEFAULT_SERVER_PORT},
    server::{Server, ServerOptions},
};
use edge_nal::UdpBind;
use edge_nal_embassy::{Udp, UdpBuffers};
use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources, StaticConfigV4};
use embassy_time::{Duration, Timer};
use esp_println::println;
use esp_wifi::wifi::{WifiApDevice, WifiDevice};

use super::http_server::run_http_server;

macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write($val);
        x
    }};
}

const GW_IP_ADDR_ENV: Option<&'static str> = option_env!("GATEWAY_IP");

#[embassy_executor::task]
pub async fn run_ap(spawner: Spawner, wifi_interface: WifiDevice<'static, WifiApDevice>) {
    let gw_ip_addr_str = GW_IP_ADDR_ENV.unwrap_or("192.168.2.1");
    let gw_ip_addr = Ipv4Addr::from_str(gw_ip_addr_str).expect("failed to parse gateway ip");

    let config = embassy_net::Config::ipv4_static(StaticConfigV4 {
        address: embassy_net::Ipv4Cidr::new(gw_ip_addr, 24),
        gateway: Some(gw_ip_addr),
        dns_servers: Default::default(),
    });

    let seed = 0x87654321_u64;

    let (stack, runner) = embassy_net::new(
        wifi_interface,
        config,
        mk_static!(StackResources<6>, StackResources::<6>::new()),
        seed,
    );

    spawner.spawn(net_task(runner)).ok();
    spawner.spawn(run_dhcp(stack, gw_ip_addr_str)).ok();

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
    println!(
        "Connect to the AP `esp-wifi` and point your browser to http://{gw_ip_addr_str}:8080/"
    );
    println!("DHCP is enabled so there's no need to configure a static IP, just in case:");
    while !stack.is_config_up() {
        Timer::after(Duration::from_millis(100)).await;
    }
    stack
        .config_v4()
        .inspect(|c| println!("ipv4 config: {c:?}"));

    match run_http_server(&stack).await {
        Ok(_) => println!("HTTP server completed successfully"),
        Err(_) => println!("HTTP server failed, please restart the device"),
    }

    loop {
        Timer::after(Duration::from_secs(10)).await;
    }
}

#[embassy_executor::task]
async fn run_dhcp(stack: Stack<'static>, gw_ip_addr: &'static str) {
    use core::net::{Ipv4Addr, SocketAddrV4};

    let ip = Ipv4Addr::from_str(gw_ip_addr).expect("dhcp task failed to parse gw ip");

    let mut buf = [0u8; 600];
    let mut gw_buf = [Ipv4Addr::UNSPECIFIED];
    let buffers = UdpBuffers::<2, 512, 512, 5>::new();
    let unbound_socket = Udp::new(stack, &buffers);
    let mut bound_socket = unbound_socket
        .bind(core::net::SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED,
            DEFAULT_SERVER_PORT,
        )))
        .await
        .unwrap();

    loop {
        _ = io::server::run(
            &mut Server::<_, 64>::new_with_et(ip),
            &ServerOptions::new(ip, Some(&mut gw_buf)),
            &mut bound_socket,
            &mut buf,
        )
        .await
        .inspect_err(|e| log::warn!("DHCP server error: {e:?}"));
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static, WifiApDevice>>) {
    runner.run().await
}
