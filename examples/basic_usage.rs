// examples/basic_usage.rs

use channel_tracer::{hook_channel, init};
use tokio::sync::mpsc;
use tokio::time;

/// This case emulates single producer and single consumer
/// where producer throughput is higher than consumer,
/// therefore, progress bars will show the queue accummulates until
/// reaching the channel limit, finally drains till the end.
#[tokio::main]
async fn main() {
    // Initialize the monitoring system with progress bars
    // and update bars every 5 millisecond
    init(5);

    // Create a channel with a buffer size of 20
    let size = 20;
    let (tx, mut rx) = mpsc::channel(size);

    // Hook the channel to monitor its queue depth
    hook_channel(tx.clone(), "example_channel", size);

    // Spawn a producer task
    tokio::spawn(async move {
        for i in 0..50 {
            tx.send(format!("Message {}", i)).await.unwrap();
            time::sleep(time::Duration::from_millis(100)).await; // Simulate work
        }
    });

    // Spawn a consumer task
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            tracing::debug!("Received: {}", msg);
            time::sleep(time::Duration::from_millis(300)).await; // Simulate processing time
        }
    });

    // Allow tasks to complete
    time::sleep(time::Duration::from_secs(20)).await;
}
