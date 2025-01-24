use dashmap::DashMap;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task;
use tracing::info;

/// Trait for channel metadata
trait ChannelInfo: Send + Sync {
    fn get_queue_depth(&self) -> (String, usize);
    fn get_channel_length(&self) -> usize;
}

/// Generic channel metadata implementation
struct ChannelMetadata<T> {
    id: String,
    len: usize,
    sender: mpsc::Sender<T>,
}

impl<T: Send + Sync + 'static> ChannelInfo for ChannelMetadata<T> {
    fn get_queue_depth(&self) -> (String, usize) {
        let capacity = self.sender.capacity();
        let qdepth = self.len - capacity;
        (self.id.clone(), qdepth)
    }

    fn get_channel_length(&self) -> usize {
        self.len
    }
}

/// Global registry for tracking channels
static CHANNELS: Lazy<DashMap<String, Arc<dyn ChannelInfo>>> = Lazy::new(DashMap::new);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Generic hook to register an mpsc::Sender with a custom ID
pub fn hook_channel<T: Send + Sync + 'static>(sender: mpsc::Sender<T>, id: &str, len: usize) {
    let metadata = ChannelMetadata {
        id: id.to_string(),
        len,
        sender,
    };
    let metadata_arc: Arc<dyn ChannelInfo> = Arc::new(metadata);
    CHANNELS.insert(id.to_string(), metadata_arc);
}

/// Spawns a background task that logs queue depth periodically for all registered channels
/// interval: ms interval that progress bars are updated
pub fn init_raw(interval: u64) {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }
    task::spawn(async move {
        loop {
            for entry in CHANNELS.iter() {
                let (id, qdepth) = entry.value().get_queue_depth();
                info!(id, qdepth, "Queue depth check");
            }
            tokio::time::sleep(Duration::from_millis(interval)).await;
        }
    });
}

/// Spawns a background task that monitors queue depth with multiple progress bars
/// interval: ms interval that progress bars are updated
pub fn init_with_multi_progress(interval: u64) {
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
                    let pb = multi_clone.add(ProgressBar::new(entry.value() .get_channel_length() as u64));
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

            tokio::time::sleep(Duration::from_millis(interval)).await;
        }
    });
}

#[tokio::test]
async fn test_hook_channel() {
    let size = 10;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<usize>(size);
    hook_channel(tx.clone(), "test_channel", size);

    let metadata = CHANNELS.get("test_channel").expect("Channel not registered");
    let (id, qdepth) = metadata.get_queue_depth();
    assert_eq!(id, "test_channel");
    assert!(qdepth <= size); // Ensure depth is within expected range
    assert_eq!(metadata.get_channel_length(), size);

    tx.send(1).await.unwrap();
    let (_, qdepth) = metadata.get_queue_depth();
    assert_eq!(qdepth, 1); // Expect 1 message in the queue

    rx.recv().await;
    let (_, qdepth) = metadata.get_queue_depth();
    assert_eq!(qdepth, 0); // Expect the queue is empty
}
