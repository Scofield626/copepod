# Copepod 

Copepod is a lightweight instrumentation tool that tracks and visualizes the queue depths of channels in the [tokio](https://github.com/tokio-rs/tokio) runtime.

## 🧱 Why instrument queue depths in tokio applications?

[Channel-based](https://tokio.rs/tokio/tutorial/channels) message passing is a common design pattern in building applications atop Tokio.
These channels serve as the communication bridges between internal system components.
Each component, acting as a processing stage, performs specific tasks and typically forwards the results to the next stage via a channel.

As a result, queueing behavior naturally emerges between components.
Monitoring and understanding this behavior is critical to the overall performance of the system.
For instance, if one stage's consumption throughput is slower than the task arrival rate, it will accumulate pending tasks, potentially creating bottlenecks.

In larger systems involving multiple interconnected channels, queuing behavior becomes more complex.
Identifying the channels where queuing first occurs helps developers diagnose system issues and pinpoint problematic components.

## 🏗️ Copepod comes to the rescue!

Existing tokio instrumentation tools (i.e., [tokio-console](https://github.com/tokio-rs/console)) do not support track queue depths.
Copepod tracks the queue depths of channels and visualizes them via real-time progress bars (using [indicatif](https://github.com/console-rs/indicatif)).
Using Copepod, one can [measure the system one level deeper](https://cacm.acm.org/research/always-measure-one-level-deeper/) and quickly identify system bottlenecks.

## ⚙️ Examples

Using Copepod requires minimal modifications to your application code. Two APIs are all you need:

- Initialize Copepod

  Add `Copepod::init(update_interval_ms: u64)` in your main function to set up Copepod with a specified update interval.

- Hook Channels

  For each channel, add a hook to its sender (tx) using:
  ```rust
  Copepod::hook_channel(tx.clone(), "channel_name", channel_size)
  ```
