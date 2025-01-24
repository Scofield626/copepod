// examples/complex_usage.rs

use channel_tracer::{hook_channel, init};
use tokio::sync::mpsc;
use tokio::time;

#[tokio::main]
async fn main() {
    // Initialize the monitoring system with progress bars
    // and update bars every 5 millisecond
    init(5);

    // Example 1: Multiple producers and a single consumer
    let size = 20;
    let (tx, mut rx) = mpsc::channel(size);
    hook_channel(tx.clone(), "multi_producer_channel", size);

    // Producer 1
    let tx_clone1 = tx.clone();
    tokio::spawn(async move {
        for i in 0..10 {
            tx_clone1.send(format!("Producer1 Message {}", i)).await.unwrap();
            time::sleep(time::Duration::from_millis(100)).await;
        }
    });

    // Producer 2
    let tx_clone2 = tx.clone();
    tokio::spawn(async move {
        for i in 0..10 {
            tx_clone2.send(format!("Producer2 Message {}", i)).await.unwrap();
            time::sleep(time::Duration::from_millis(150)).await;
        }
    });

    // Consumer
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            tracing::debug!("Single Consumer received: {}", msg);
            time::sleep(time::Duration::from_millis(200)).await;
        }
    });

    // Example 2: Burst traffic with delayed consumer
    let (tx_burst, mut rx_burst) = mpsc::channel(size);
    hook_channel(tx_burst.clone(), "burst_traffic_channel", size);

    // Burst producer
    tokio::spawn(async move {
        for i in 0..30 {
            tx_burst.send(format!("Burst Message {}", i)).await.unwrap();
            time::sleep(time::Duration::from_millis(50)).await;
        }
    });

    // Delayed consumer
    tokio::spawn(async move {
        time::sleep(time::Duration::from_secs(2)).await;
        while let Some(msg) = rx_burst.recv().await {
            tracing::debug!("Delayed Consumer received: {}", msg);
        }
    });

    // Allow tasks to complete
    time::sleep(time::Duration::from_secs(10)).await;
}
