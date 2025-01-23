// examples/complex_usage.rs

use channel_tracer::{hook_channel, init_with_multi_progress};
use tokio::sync::mpsc;
use tokio::time;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Initialize the monitoring system with progress bars
    init_with_multi_progress();

    // Example 1: Multiple producers and a single consumer
    let (tx, mut rx) = mpsc::channel(20);
    hook_channel(tx.clone(), "multi_producer_channel");

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
            println!("Single Consumer received: {}", msg);
            time::sleep(time::Duration::from_millis(200)).await;
        }
    });

    // Example 2: Burst traffic with delayed consumer
    let (tx_burst, mut rx_burst) = mpsc::channel(20);
    hook_channel(tx_burst.clone(), "burst_traffic_channel");

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
            println!("Delayed Consumer received: {}", msg);
        }
    });

    // Example 3: Shared state between producers
    let (tx_shared, mut rx_shared) = mpsc::channel(20);
    hook_channel(tx_shared.clone(), "shared_state_channel");

    let shared_state = Arc::new(tokio::sync::Mutex::new(0));

    // Producer updating shared state
    let shared_state_clone = shared_state.clone();
    let tx_shared_clone = tx_shared.clone();
    tokio::spawn(async move {
        for _ in 0..10 {
            let mut state = shared_state_clone.lock().await;
            *state += 1;
            tx_shared_clone.send(format!("Updated State: {}", *state)).await.unwrap();
            time::sleep(time::Duration::from_millis(100)).await;
        }
    });

    // Consumer reading messages
    tokio::spawn(async move {
        while let Some(msg) = rx_shared.recv().await {
            println!("Consumer with Shared State received: {}", msg);
        }
    });

    // Allow tasks to complete
    time::sleep(time::Duration::from_secs(10)).await;
}
