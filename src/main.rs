use dashmap::DashMap;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task;
use tracing::{debug, info, Level};

const DEFAULT_CHANNEL_SIZE: usize = 20;

/// Trait for channel metadata
trait ChannelInfo: Send + Sync {
    fn get_queue_depth(&self) -> (String, usize);
}

/// Generic channel metadata implementation
struct ChannelMetadata<T> {
    id: String,
    sender: mpsc::Sender<T>,
}

impl<T: Send + Sync + 'static> ChannelInfo for ChannelMetadata<T> {
    fn get_queue_depth(&self) -> (String, usize) {
        let capacity = self.sender.capacity();
        let qdepth = DEFAULT_CHANNEL_SIZE - capacity;
        (self.id.clone(), qdepth)
    }
}

/// Global registry for tracking channels
static CHANNELS: Lazy<DashMap<String, Arc<dyn ChannelInfo>>> = Lazy::new(DashMap::new);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Generic hook to register an mpsc::Sender with a custom ID
pub fn hook_channel<T: Send + Sync + 'static>(sender: mpsc::Sender<T>, id: &str) {
    let metadata = ChannelMetadata {
        id: id.to_string(),
        sender,
    };
    let metadata_arc: Arc<dyn ChannelInfo> = Arc::new(metadata);
    CHANNELS.insert(id.to_string(), metadata_arc);
}

/// Spawns a background task that logs queue depth periodically for all registered channels
pub fn init_raw() {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }
    task::spawn(async move {
        loop {
            for entry in CHANNELS.iter() {
                let (id, qdepth) = entry.value().get_queue_depth();
                info!(id, qdepth, "Queue depth check");
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}

/// Spawns a background task that monitors queue depth with multiple progress bars
pub fn init_with_multi_progress() {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    // MultiProgress setup
    let multi = Arc::new(MultiProgress::new());
    let style = ProgressStyle::with_template("{msg}: [{wide_bar}] {pos}/{len}")
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏  ");

    let multi_clone = multi.clone();

    task::spawn(async move {
        let progress_bars = DashMap::new();

        loop {
            // If new channels are added, create progress bars for them
            CHANNELS.iter().for_each(|entry| {
                let id = entry.key();
                if !progress_bars.contains_key(id) {
                    let pb = multi_clone.add(ProgressBar::new(DEFAULT_CHANNEL_SIZE as u64));
                    pb.set_style(style.clone());
                    pb.set_message(format!("Channel: {}", id));
                    progress_bars.insert(id.clone(), pb);
                }
            });

            // Update progress bars
            for item in progress_bars.iter_mut() {
                let channel_id = item.key();
                if let Some(metadata) = CHANNELS.get(channel_id) {
                    let (_, qdepth) = metadata.value().get_queue_depth();
                    item.value().set_position(qdepth as u64);
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Initialize the monitoring background task
    init_with_multi_progress();

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
