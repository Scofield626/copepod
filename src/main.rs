use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task;
use tracing::{info, instrument, Level};
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex as AsyncMutex;

/// Global registry for tracking mpsc::Sender queue depths
static CHANNELS: Lazy<Arc<AsyncMutex<Vec<mpsc::Sender<String>>>>> = Lazy::new(|| Arc::new(AsyncMutex::new(Vec::new())));
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Hooks into the Tokio mpsc channel creation process
#[instrument]
pub fn hook_channel(sender: &mpsc::Sender<String>) {
    let sender = sender.clone();
    tokio::spawn(async move {
        let mut channels = CHANNELS.lock().await;
        channels.push(sender);
    });
}

/// Spawns a background task that logs queue depth periodically
pub fn init() {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }
    let channels = Arc::clone(&CHANNELS);
    task::spawn(async move {
        loop {
            let channels_guard = channels.lock().await;
            for sender in channels_guard.iter() {
                let capacity = sender.capacity();
                info!(queue_depth = capacity, "Queue depth check");
            }
            drop(channels_guard); // Explicitly drop lock before sleeping
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    
    init();
    let (tx, mut rx) = mpsc::channel(10);
    hook_channel(&tx);
    
    // Example usage with multiple sends
    for i in 0..5 {
        tx.send(format!("Message {}", i)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await; // Simulate workload
    }
    
    // Receive messages
    while let Some(msg) = rx.recv().await {
        info!(received = ?msg, "Received message");
    }
}
