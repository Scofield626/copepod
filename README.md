# Copepod 

Copepod is a lightweight instrument tool to track and visualize queue depths of channels in tokio-rt.

## Why instrument queue depths in tokio applications?

It is a de-facto pattern to use channel-based message-passing to build applicationis atop tokio.
These channels bridge the internal components of the built system together.
Each component, as one of the stage, is in charge of some processing, and then probably communicates with the next stage via a channel.
Therefore, there are some sort of queuing happening between components.

The queuing behaviours in these channels are foundamentally critical to the overall system performance.
For example, one stage will have more and more pending tasks if its consuming throughput is slower than task arrival rate.
For a large system which involves many channels, queuing behaviour is much more complicated.
Figuring out which channels introduce the queuing first can help developers understand the system and identify the problematic parts. 

## Why Copepod?

Existing tokio instrument tools (@tokio-console) do not support track queue depths.
Copepod tracks the queue depths of channels and visualizes them via real-time progress bars (@indicatif).
Using Copepod, one can measure the system one level deeper (todo: ref jon's paper) and quickly identify system bottlenecks.

## Examples

To use Copepod, two APIs are needed with minimal application modification.
- First, use`Copepod::init(update_interval_ms: u64)` in the main fn;
- Second, add hook for each channel's tx: `Copepod::hook_channel(tx.clone(), "channel_name", channel_size)`
