# DSL-RS (Deepstream Services Library for Rust)

A robust Rust implementation of multi-stream video processing built on GStreamer, prioritizing production reliability with automatic error recovery and zero-downtime operations.

## Overview

DSL-RS provides a rock-solid foundation for 24/7 video processing systems with:

- **Automatic Error Recovery**: Self-healing from network failures, file errors, and stream disruptions
- **Dynamic Stream Management**: Add/remove sources without pipeline restarts
- **Stream Isolation**: One stream's failure doesn't affect others
- **Production-Ready**: Designed for mission-critical deployments

## Features

### Core Capabilities
- File sources (MP4/MKV) with automatic loop restart
- RTSP sources with exponential backoff reconnection
- File sinks with rotation by size/time
- RTSP server sink for streaming
- Per-stream health monitoring and metrics
- Circuit breaker pattern for failure prevention
- Resource isolation and quota management

### Architecture Highlights
- **Zero-downtime** source modifications
- **Stream isolation** via GStreamer bins
- **Watchdog timers** for deadlock detection
- **Configurable retry** strategies
- **Memory and CPU** quota enforcement
- **Comprehensive metrics** via Prometheus export

## Installation

```toml
[dependencies]
dsl-rs = "0.1.0"
```

## Quick Start

```rust
use dsl_rs::{init_gstreamer, init_logging};
use dsl_rs::pipeline::robust_pipeline::{RobustPipeline, PipelineConfig};
use dsl_rs::source::rtsp_source_robust::RtspSourceRobust;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize
    init_logging();
    init_gstreamer()?;
    
    // Create pipeline
    let config = PipelineConfig::default();
    let mut pipeline = RobustPipeline::new(config)?;
    
    // Add RTSP source
    let source = Box::new(RtspSourceRobust::new(
        "camera_1".to_string(),
        "rtsp://camera.local/stream".to_string()
    )?);
    
    // Start pipeline
    pipeline.start()?;
    
    // Run...
    
    pipeline.stop()?;
    Ok(())
}
```

## Examples

See the `examples/` directory for complete examples:

- `robust_multistream.rs` - Multiple RTSP sources with recording

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Check code
cargo check

# Run example
cargo run --example robust_multistream
```

## Architecture

The system is organized into focused modules:

- **`core/`** - Foundation types and traits
- **`pipeline/`** - Robust pipeline management
- **`stream/`** - Dynamic stream lifecycle
- **`source/`** - File and RTSP sources
- **`sink/`** - File and RTSP sinks
- **`recovery/`** - Circuit breakers and retry strategies
- **`health/`** - Monitoring and metrics
- **`isolation/`** - Resource quota enforcement

## Configuration

### Pipeline Configuration
```rust
let config = PipelineConfig {
    max_streams: 32,
    enable_watchdog: true,
    watchdog_timeout: Duration::from_secs(10),
    ..Default::default()
};
```

### Retry Configuration
```rust
let mut retry = RetryConfig::default();
retry.max_attempts = 10;
retry.initial_delay = Duration::from_millis(100);
retry.exponential_base = 2.0;
```

## Error Handling

DSL-RS prioritizes reliability:

- Automatic reconnection for network sources
- File rotation on disk errors
- Circuit breakers prevent cascade failures
- Isolated streams prevent cross-contamination
- Comprehensive error classification and recovery

## Background

DSL-RS is a Rust port inspired by the DeepStream Services Library (DSL), originally written in C. This implementation focuses on robustness and production reliability while providing a safe, idiomatic Rust API.

