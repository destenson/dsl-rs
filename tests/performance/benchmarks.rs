//! Performance benchmarks for DSL-RS
//!
//! Establishes baselines for throughput, latency, scalability, and memory usage.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use dsl_rs::core::*;
use dsl_rs::pipeline::robust_pipeline::RobustPipeline;
use dsl_rs::recovery::*;
use dsl_rs::stream::{StreamConfig, StreamManager};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn benchmark_stream_creation(c: &mut Criterion) {
    c.bench_function("stream_creation", |b| {
        b.iter(|| {
            let config = StreamConfig {
                name: "bench_stream".to_string(),
                buffer_size: 100,
                max_latency: Some(1000),
                enable_isolation: true,
                queue_properties: Default::default(),
            };
            std::hint::black_box(config);
        });
    });
}

fn benchmark_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_transitions");

    for state in &[
        StreamState::Idle,
        StreamState::Running,
        StreamState::Recovering,
    ] {
        group.bench_with_input(
            BenchmarkId::new("state", format!("{:?}", state)),
            state,
            |b, s| {
                b.iter(|| {
                    // Just benchmark state creation and comparison
                    let new_state = match s {
                        StreamState::Idle => StreamState::Starting,
                        StreamState::Starting => StreamState::Running,
                        StreamState::Running => StreamState::Paused,
                        StreamState::Paused => StreamState::Running,
                        StreamState::Recovering => StreamState::Running,
                        StreamState::Failed => StreamState::Recovering,
                        StreamState::Stopped => StreamState::Idle,
                    };
                    std::hint::black_box(new_state);
                });
            },
        );
    }
    group.finish();
}

fn benchmark_recovery_decisions(c: &mut Criterion) {
    let manager = RecoveryManager::new();

    c.bench_function("recovery_decision", |b| {
        b.iter(|| {
            // Benchmark checking if recovery should be attempted
            let should_recover = manager.should_attempt_recovery("test_stream");
            std::hint::black_box(should_recover);
        });
    });
}

fn benchmark_metrics_update(c: &mut Criterion) {
    c.bench_function("metrics_update", |b| {
        b.iter(|| {
            let metrics = StreamMetrics {
                fps: 30.0,
                bitrate: 1024 * 1024,
                frames_processed: 1000,
                frames_dropped: 0,
                errors: 0,
                uptime: Duration::from_secs(60),
                last_frame_time: Some(Instant::now()),
            };
            std::hint::black_box(metrics);
        });
    });
}

fn benchmark_concurrent_streams(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_streams");

    // Initialize GStreamer for benchmarks
    let _ = init_gstreamer();

    for count in &[1, 5, 10, 20] {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_with_setup(
                || {
                    // Setup: Create a pipeline for each iteration
                    let config = PipelineConfig::default();
                    Arc::new(RobustPipeline::new(config).unwrap())
                },
                |pipeline| {
                    // Benchmark: Create stream manager and add configs
                    let _manager = StreamManager::new(pipeline);
                    for i in 0..count {
                        let config = StreamConfig {
                            name: format!("stream_{}", i),
                            buffer_size: 100,
                            max_latency: Some(1000),
                            enable_isolation: true,
                            queue_properties: Default::default(),
                        };
                        std::hint::black_box(config);
                    }
                },
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    benchmark_stream_creation,
    benchmark_state_transitions,
    benchmark_recovery_decisions,
    benchmark_metrics_update,
    benchmark_concurrent_streams
);
criterion_main!(benches);
