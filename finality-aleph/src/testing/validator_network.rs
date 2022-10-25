use std::sync::Once;

use tokio::time::Duration;

use crate::testing::mocks::validator_network::scenario;

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        env_logger::init();
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn normal_conditions() {
    setup();
    let n_peers: usize = 10;
    let n_msg: usize = 30;
    let broken_connection_interval: usize = 100000;
    let large_message_interval: usize = 100000;
    let corrupted_message_interval: usize = 100000;
    let status_report_interval: Duration = Duration::from_secs(1);
    scenario(
        n_peers,
        n_msg,
        broken_connection_interval,
        large_message_interval,
        corrupted_message_interval,
        status_report_interval,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn connections_break() {
    setup();
    let n_peers: usize = 10;
    let n_msg: usize = 30;
    let broken_connection_interval: usize = 10;
    let large_message_interval: usize = 100000;
    let corrupted_message_interval: usize = 100000;
    let status_report_interval: Duration = Duration::from_secs(1);
    scenario(
        n_peers,
        n_msg,
        broken_connection_interval,
        large_message_interval,
        corrupted_message_interval,
        status_report_interval,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn large_messages_being_sent() {
    setup();
    let n_peers: usize = 10;
    let n_msg: usize = 30;
    let broken_connection_interval: usize = 100000;
    let large_message_interval: usize = 10;
    let corrupted_message_interval: usize = 100000;
    let status_report_interval: Duration = Duration::from_secs(1);
    scenario(
        n_peers,
        n_msg,
        broken_connection_interval,
        large_message_interval,
        corrupted_message_interval,
        status_report_interval,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn corrupted_messages_being_sent() {
    setup();
    let n_peers: usize = 10;
    let n_msg: usize = 30;
    let broken_connection_interval: usize = 100000;
    let large_message_interval: usize = 100000;
    let corrupted_message_interval: usize = 10;
    let status_report_interval: Duration = Duration::from_secs(1);
    scenario(
        n_peers,
        n_msg,
        broken_connection_interval,
        large_message_interval,
        corrupted_message_interval,
        status_report_interval,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn everything_fails_all_the_time() {
    setup();
    let n_peers: usize = 3;
    let n_msg: usize = 20;
    let broken_connection_interval: usize = 5;
    let large_message_interval: usize = 8;
    let corrupted_message_interval: usize = 10;
    let status_report_interval: Duration = Duration::from_secs(1);
    scenario(
        n_peers,
        n_msg,
        broken_connection_interval,
        large_message_interval,
        corrupted_message_interval,
        status_report_interval,
    )
    .await;
}
