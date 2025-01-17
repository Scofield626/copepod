use once_cell::sync::Lazy;
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;
use tokio::task;
use tracing::{debug, info, Level};

const DEFAULT_CHANNEL_SIZE: usize = 20;

/// Helper struct to store metadata and sender information
struct ChannelMetadata<T> {
    id: String,
    sender: mpsc::Sender<T>,
}

/// Global registry for tracking channels with custom identifiers
static CHANNELS: Lazy<Arc<AsyncMutex<Vec<Box<dyn Any + Send + Sync>>>>> =
    Lazy::new(|| Arc::new(AsyncMutex::new(Vec::new())));
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Generic hook to register an mpsc::Sender with a custom ID
pub fn hook_channel<T: Send + Sync + 'static>(sender: mpsc::Sender<T>, id: &str) {
    let metadata = ChannelMetadata {
        id: id.to_string(),
        sender,
    };
    tokio::spawn(async move {
        let mut channels = CHANNELS.lock().await;
        channels.push(Box::new(metadata));
    });
}

/// Spawns a background task that logs queue depth periodically for all registered channels
pub fn init() {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }
    task::spawn(async move {
        loop {
            let channels_guard = CHANNELS.lock().await;
            for channel in channels_guard.iter() {
                // Attempt to downcast the channel metadata to its original type
                if let Some(metadata) = channel.downcast_ref::<ChannelMetadata<String>>() {
                    let capacity = metadata.sender.capacity();
                    let qdepth = DEFAULT_CHANNEL_SIZE - capacity;
                    info!(channel_id = %metadata.id, qdepth, "Queue depth check");
                }
            }
            drop(channels_guard); // Explicitly drop lock before sleeping
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Initialize the monitoring background task
    init();

    // Create a channel with a buffer of 10
    let (tx, mut rx) = mpsc::channel(DEFAULT_CHANNEL_SIZE);

    // Hook the channel to monitor its queue depth
    hook_channel(tx.clone(), "test_1");

    // Case 1: Single Producer, Slow Consumer
    tokio::spawn(async move {
        for i in 0..15 {
            tx.send(format!("Message {}", i)).await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await; // Simulate workload
        }
    });

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            debug!(received = ?msg, "Slow Consumer received message");
            tokio::time::sleep(Duration::from_millis(300)).await; // Simulate processing time
        }
        drop(rx);
    });

    // Case 2: Multiple Producers, Single Consumer
    let (tx2, mut rx2) = mpsc::channel(DEFAULT_CHANNEL_SIZE);
    hook_channel(tx2.clone(), "test_2");

    let ptx2 = tx2.clone();
    tokio::spawn(async move {
        for i in 0..10 {
            ptx2.send(format!("Producer1 Message {}", i)).await.unwrap();
            tokio::time::sleep(Duration::from_millis(150)).await; // Simulate workload
        }
    });

    tokio::spawn(async move {
        for i in 0..10 {
            tx2.send(format!("Producer2 Message {}", i)).await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await; // Simulate faster workload
        }
    });

    tokio::spawn(async move {
        while let Some(msg) = rx2.recv().await {
            debug!(received = ?msg, "Single Consumer received message");
            tokio::time::sleep(Duration::from_millis(200)).await; // Simulate processing time
        }
    });

    // Case 3: Burst Traffic and Paused Consumer
    let (tx3, mut rx3) = mpsc::channel(DEFAULT_CHANNEL_SIZE);
    hook_channel(tx3.clone(), "test_3");

    tokio::spawn(async move {
        for i in 0..30 {
            tx3.send(format!("Burst Message {}", i)).await.unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await; // Rapid workload
        }
    });

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await; // Pause consumer for 2 seconds
        while let Some(msg) = rx3.recv().await {
            debug!(received = ?msg, "Burst Consumer received message");
        }
    });

    tokio::time::sleep(Duration::from_secs(10)).await; // Allow all tasks to complete
}
