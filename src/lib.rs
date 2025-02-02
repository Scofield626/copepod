//! An instrumentation tool to track and visualize queue depths of bounded channels
//! in tokio runtime. Provides real-time progress bars to monitor channel queuing
//! behaviour, helping developers to diagnose system issues and identify bottlenecks.

use dashmap::DashMap;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use once_cell::sync::Lazy;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::mpsc,
    task,
};

/// Trait for channel metadata.
///
/// This trait provides methods to retrieve information about a channel,
/// such as its queue depth and total length.
pub trait ChannelInfo: Send + Sync {
    /// Returns the channel's ID and its current queue depth.
    ///
    /// The queue depth represents the number of items currently in the channel's queue.
    fn get_queue_depth(&self) -> (String, usize);

    /// Returns the total length (capacity) of the channel.
    fn get_channel_length(&self) -> usize;
}

/// Generic implementation of channel metadata.
///
/// This struct stores metadata for a channel, including its ID, length (capacity),
/// and the sender half of the channel.
struct ChannelMetadata<T> {
    id: String,
    len: usize,
    sender: mpsc::Sender<T>,
}

impl<T: Send + Sync + 'static> ChannelInfo for ChannelMetadata<T> {
    /// Calculates the queue depth by comparing the channel's length with its remaining capacity.
    fn get_queue_depth(&self) -> (String, usize) {
        let capacity = self.sender.capacity();
        let qdepth = self.len - capacity;
        (self.id.clone(), qdepth)
    }

    /// Returns the total length (capacity) of the channel.
    fn get_channel_length(&self) -> usize {
        self.len
    }
}

/// Global registry for tracking channels.
///
/// This registry uses a `DashMap` to store channel metadata, allowing concurrent access.
static CHANNELS: Lazy<DashMap<String, Arc<dyn ChannelInfo>>> = Lazy::new(DashMap::new);

/// Atomic flag to ensure the monitoring task is initialized only once.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Registers a channel with the global registry.
///
/// This function associates an `mpsc::Sender` with a unique ID and its length (capacity).
/// The channel's metadata is stored in the global registry for monitoring.
///
/// # Arguments
/// - `sender`: The sender half of the channel.
/// - `id`: A unique identifier for the channel.
/// - `len`: The total length (capacity) of the channel.
pub fn hook_channel<T: Send + Sync + 'static>(sender: mpsc::Sender<T>, id: &str, len: usize) {
    let metadata = ChannelMetadata {
        id: id.to_string(),
        len,
        sender,
    };
    let metadata_arc: Arc<dyn ChannelInfo> = Arc::new(metadata);
    CHANNELS.insert(id.to_string(), metadata_arc);
}

/// Initializes a background task to monitor channel queue depths.
///
/// This function spawns a Tokio task that periodically updates progress bars
/// to reflect the queue depth of registered channels.
///
/// # Arguments
/// - `interval`: The time interval (in milliseconds) at which progress bars are updated.
///
/// # Panics
/// This function panics if the progress bar style template is invalid.
pub fn init(interval: u64) {
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
                    let pb = multi_clone.add(ProgressBar::new(entry.value().get_channel_length() as u64));
                    pb.set_style(style.clone());
                    pb.set_message(format!("Copepod::{}", id));
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
