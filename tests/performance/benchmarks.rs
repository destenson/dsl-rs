//! Performance benchmarks for DSL-RS
//!
//! Establishes baselines for throughput, latency, scalability, and memory usage.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use dsl_rs::core::*;
use dsl_rs::pipeline::*;
use dsl_rs::stream::*;
use dsl_rs::recovery::*;
use std::time::Duration;

fn benchmark_stream_creation(c: &mut Criterion) {
    c.bench_function("stream_creation", |b| {
        b.iter(|| {
            let info = StreamInfo {
                name: "bench_stream".to_string(),
                source_type: "file".to_string(),
                sink_type: "rtsp".to_string(),
                created_at: std::time::SystemTime::now(),
                state: StreamState::Idle,
            };
            black_box(info);
        });
    });
}

fn benchmark_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_transitions");
    
    for state in &[StreamState::Idle, StreamState::Running, StreamState::Recovering] {
        group.bench_with_input(
            BenchmarkId::new("transition", format!("{:?}", state)),
            state,
            |b, s| {
                b.iter(|| {
                    s.next_state(TransitionCondition::OnSuccess)
                });
            },
        );
    }
    group.finish();
}

fn benchmark_recovery_decisions(c: &mut Criterion) {
    let manager = RecoveryManager::new(3, Duration::from_secs(30));
    
    c.bench_function("recovery_decision", |b| {
        b.iter(|| {
            manager.decide_recovery_action(
                "test",
                &DslError::Connection("test".to_string())
            )
        });
    });
}

fn benchmark_metrics_update(c: &mut Criterion) {
    c.bench_function("metrics_update", |b| {
        b.iter(|| {
            let metrics = StreamMetrics {
                frames_processed: 1000,
                bytes_processed: 1024 * 1024,
                latency_ms: 25.0,
                dropped_frames: 0,
            };
            black_box(metrics);
        });
    });
}

fn benchmark_concurrent_streams(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_streams");
    
    for count in &[1, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            count,
            |b, &count| {
                b.iter(|| {
                    let manager = StreamManager::new(count).unwrap();
                    for i in 0..count {
                        let info = StreamInfo {
                            name: format!("stream_{}", i),
                            source_type: "test".to_string(),
                            sink_type: "test".to_string(),
                            created_at: std::time::SystemTime::now(),
                            state: StreamState::Idle,
                        };
                        let _ = manager.add_stream(info);
                    }
                });
            },
        );
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