use async_compat::CompatExt;
use embedded_svc::mqtt::client::asynch::{Client, Connection, Publish, QoS};
use embedded_svc::mqtt::client::Event;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Timer};
use edge_mqtt::io::{AsyncClient, MqttClient, MqttConnection, MqttOptions};
use log::*;
use futures_lite::future::block_on;

const MQTT_HOST: &str = "broker.emqx.io";
const MQTT_PORT: u16 = 1883;
const MQTT_CLIENT_ID: &str = "edge-mqtt-demo";
const MQTT_TOPIC: &str = "edge-mqtt-demo";

pub async fn run_mqtt_client() -> Result<(), anyhow::Error> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let (client, conn) = mqtt_create(MQTT_CLIENT_ID, MQTT_HOST, MQTT_PORT)?;

    run(client, conn, MQTT_TOPIC).compat().await
}

async fn run<M, C>(mut client: M, mut connection: C, topic: &str) -> Result<(), anyhow::Error>
where
    M: Client + Publish + 'static,
    M::Error: std::error::Error + Send + Sync + 'static,
    C: Connection + 'static,
{
    info!("About to start the MQTT client");
    info!("MQTT client started");
    client.subscribe(topic, QoS::AtMostOnce).await?;
    info!("Subscribed to topic \"{topic}\"");

    let res = select(
        async {
            info!("MQTT Listening for messages");
            while let Ok(event) = connection.next().await {
                info!("[Queue] Event: {}", event.payload());
            }
            info!("Connection closed");
            Ok(())
        },
        async {
            // Give time for the first published message to be received
            Timer::after(Duration::from_millis(500)).await;
            let payload = "Hello from edge-mqtt-demo!";
            loop {
                client
                    .publish(topic, QoS::AtMostOnce, false, payload.as_bytes())
                    .await?;
                info!("Published \"{payload}\" to topic \"{topic}\"");
                info!("Now sleeping for 2s...");
                Timer::after(Duration::from_secs(2)).await;
            }
        },
    )
    .await;

    match res {
        Either::First(res) => res,
        Either::Second(res) => res,
    }
}

fn mqtt_create(
    client_id: &str,
    host: &str,
    port: u16,
) -> Result<(MqttClient, MqttConnection), anyhow::Error> {
    let mut mqtt_options = MqttOptions::new(client_id, host, port);
    mqtt_options.set_keep_alive(core::time::Duration::from_secs(10));
    let (rumqttc_client, rumqttc_eventloop) = AsyncClient::new(mqtt_options, 10);
    let mqtt_client = MqttClient::new(rumqttc_client);
    let mqtt_conn = MqttConnection::new(rumqttc_eventloop);
    Ok((mqtt_client, mqtt_conn))
}
