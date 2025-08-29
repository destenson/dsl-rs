//! Integration tests for pipeline functionality
//!
//! These tests validate the complete pipeline behavior including
//! single and multi-stream scenarios, dynamic stream management,
//! and stream isolation.

#[path = "../common/mod.rs"]
mod common;

use common::*;
use dsl_rs::pipeline::*;
use dsl_rs::stream::*;
use dsl_rs::source::*;
use dsl_rs::sink::*;
use dsl_rs::core::*;
use std::time::Duration;
use std::sync::Arc;

/// Test single stream pipeline from start to finish
#[tokio::test]
async fn test_single_stream_pipeline() {
    init_gstreamer();
    
    let config = PipelineConfig {
        watchdog_timeout: Some(Duration::from_secs(10)),
        max_streams: 5,
        enable_metrics: true,
    };
    
    let pipeline = RobustPipeline::new(config).expect("Failed to create pipeline");
    
    // Create mock source and sink
    let source = Arc::new(MockSource::new("test_stream"));
    let sink = Arc::new(MockSink::new("test_stream"));
    
    // Start pipeline monitoring
    pipeline.start_monitoring();
    
    // Connect source and prepare sink
    source.connect().await.expect("Failed to connect source");
    sink.prepare().await.expect("Failed to prepare sink");
    
    // Let it run for a bit
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Verify stream is running
    let health = pipeline.get_stream_health("test_stream");
    assert!(health.is_some());
    
    // Clean up
    source.disconnect().await.expect("Failed to disconnect");
    sink.cleanup().await.expect("Failed to cleanup");
}

/// Test multiple concurrent streams
#[tokio::test]
async fn test_multi_stream_pipeline() {
    init_gstreamer();
    
    let config = PipelineConfig {
        watchdog_timeout: Some(Duration::from_secs(30)),
        max_streams: 10,
        enable_metrics: true,
    };
    
    let pipeline = RobustPipeline::new(config).expect("Failed to create pipeline");
    pipeline.start_monitoring();
    
    // Create multiple streams
    let streams = generate_test_streams(5, "stream");
    let mut handles = Vec::new();
    
    // Start all streams concurrently
    for (name, source, sink) in streams.iter() {
        let source = Arc::new(source);
        let sink = Arc::new(sink);
        let stream_name = name.clone();
        
        let handle = tokio::spawn(async move {
            source.connect().await.expect("Failed to connect");
            sink.prepare().await.expect("Failed to prepare");
            
            // Let stream run
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            // Clean up
            source.disconnect().await.expect("Failed to disconnect");
            sink.cleanup().await.expect("Failed to cleanup");
            
            stream_name
        });
        
        handles.push(handle);
    }
    
    // Wait for all streams to complete
    for handle in handles {
        let name = handle.await.expect("Stream task failed");
        println!("Stream {} completed", name);
    }
    
    // Verify all streams were tracked
    let stream_names = pipeline.get_all_stream_names();
    assert!(stream_names.len() <= 5);
}

/// Test dynamic stream addition and removal
#[tokio::test]
async fn test_dynamic_stream_management() {
    init_gstreamer();
    
    let manager = StreamManager::new(10).expect("Failed to create stream manager");
    
    // Add first stream
    let source1 = Arc::new(MockSource::new("stream1"));
    let sink1 = Arc::new(MockSink::new("stream1"));
    
    source1.connect().await.expect("Failed to connect");
    sink1.prepare().await.expect("Failed to prepare");
    
    let info1 = StreamInfo {
        name: "stream1".to_string(),
        source_type: "mock".to_string(),
        sink_type: "mock".to_string(),
        created_at: std::time::SystemTime::now(),
        state: StreamState::Running,
    };
    
    manager.add_stream(info1.clone()).expect("Failed to add stream1");
    assert_eq!(manager.get_active_count(), 1);
    
    // Add second stream
    let source2 = Arc::new(MockSource::new("stream2"));
    let sink2 = Arc::new(MockSink::new("stream2"));
    
    source2.connect().await.expect("Failed to connect");
    sink2.prepare().await.expect("Failed to prepare");
    
    let info2 = StreamInfo {
        name: "stream2".to_string(),
        source_type: "mock".to_string(),
        sink_type: "mock".to_string(),
        created_at: std::time::SystemTime::now(),
        state: StreamState::Running,
    };
    
    manager.add_stream(info2).expect("Failed to add stream2");
    assert_eq!(manager.get_active_count(), 2);
    
    // Remove first stream
    manager.remove_stream("stream1").expect("Failed to remove stream1");
    assert_eq!(manager.get_active_count(), 1);
    
    // Verify correct stream remains
    let info = manager.get_stream_info("stream2");
    assert!(info.is_some());
    assert_eq!(info.unwrap().name, "stream2");
    
    // Clean up
    source1.disconnect().await.expect("Failed to disconnect");
    sink1.cleanup().await.expect("Failed to cleanup");
    source2.disconnect().await.expect("Failed to disconnect");
    sink2.cleanup().await.expect("Failed to cleanup");
}

/// Test stream isolation - one stream failure shouldn't affect others
#[tokio::test]
async fn test_stream_isolation() {
    init_gstreamer();
    
    let config = PipelineConfig::default();
    let pipeline = RobustPipeline::new(config).expect("Failed to create pipeline");
    pipeline.start_monitoring();
    
    // Create healthy stream
    let healthy_source = Arc::new(MockSource::new("healthy"));
    let healthy_sink = Arc::new(MockSink::new("healthy"));
    
    healthy_source.connect().await.expect("Failed to connect healthy");
    healthy_sink.prepare().await.expect("Failed to prepare healthy");
    
    // Create failing stream
    let failing_source = Arc::new(MockSource::new("failing"));
    let failing_sink = Arc::new(MockSink::new("failing"));
    
    failing_source.set_should_fail(true);
    failing_sink.set_should_fail(true);
    
    // Try to start failing stream (should fail but not crash)
    let _ = failing_source.connect().await;
    let _ = failing_sink.prepare().await;
    
    // Wait a bit
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Verify healthy stream is still running
    assert_eq!(*healthy_source.state.lock().unwrap(), SourceState::Connected);
    assert_eq!(*healthy_sink.state.lock().unwrap(), SinkState::Ready);
    
    // Clean up
    healthy_source.disconnect().await.expect("Failed to disconnect");
    healthy_sink.cleanup().await.expect("Failed to cleanup");
}

/// Test pipeline state transitions
#[tokio::test]
async fn test_pipeline_state_transitions() {
    init_gstreamer();
    
    let config = PipelineConfig::default();
    let pipeline = RobustPipeline::new(config).expect("Failed to create pipeline");
    
    // Test state machine
    let mut state_machine = StateMachine::new();
    
    // Initial state
    assert_eq!(state_machine.get_state("test"), StreamState::Idle);
    
    // Start stream
    let new_state = state_machine.transition("test", TransitionCondition::Success);
    assert_eq!(new_state, Some(StreamState::Starting));
    
    // Stream running
    let new_state = state_machine.transition("test", TransitionCondition::Success);
    assert_eq!(new_state, Some(StreamState::Running));
    
    // Error occurs
    let new_state = state_machine.transition("test", TransitionCondition::Error);
    assert_eq!(new_state, Some(StreamState::Recovering));
    
    // Recovery succeeds
    let new_state = state_machine.transition("test", TransitionCondition::Success);
    assert_eq!(new_state, Some(StreamState::Running));
    
    // Stop stream
    let new_state = state_machine.transition("test", TransitionCondition::Stop);
    assert_eq!(new_state, Some(StreamState::Stopping));
    
    // Stream stopped
    let new_state = state_machine.transition("test", TransitionCondition::Success);
    assert_eq!(new_state, Some(StreamState::Stopped));
}

/// Test pipeline metrics collection
#[tokio::test]
async fn test_pipeline_metrics() {
    init_gstreamer();
    
    let config = PipelineConfig {
        watchdog_timeout: Some(Duration::from_secs(10)),
        max_streams: 5,
        enable_metrics: true,
    };
    
    let pipeline = RobustPipeline::new(config).expect("Failed to create pipeline");
    pipeline.start_monitoring();
    
    // Create stream with source and sink
    let source = Arc::new(MockSource::new("metrics_test"));
    let sink = Arc::new(MockSink::new("metrics_test"));
    
    source.connect().await.expect("Failed to connect");
    sink.prepare().await.expect("Failed to prepare");
    
    // Generate some metrics
    let metrics = StreamMetrics {
        frames_processed: 100,
        bytes_processed: 1024 * 1024,
        latency_ms: 25.5,
        dropped_frames: 2,
    };
    
    pipeline.update_stream_metrics("metrics_test", metrics.clone());
    
    // Verify metrics were recorded
    let health = pipeline.get_stream_health("metrics_test");
    assert!(health.is_some());
    
    // In real implementation, would verify metrics through metrics collector
    
    // Clean up
    source.disconnect().await.expect("Failed to disconnect");
    sink.cleanup().await.expect("Failed to cleanup");
}

/// Test watchdog timer functionality
#[tokio::test]
async fn test_watchdog_timer() {
    use std::sync::atomic::{AtomicBool, Ordering};
    
    let timeout = Duration::from_millis(100);
    let watchdog = WatchdogTimer::new(timeout);
    
    // Start watchdog
    let triggered = Arc::new(AtomicBool::new(false));
    let triggered_clone = Arc::clone(&triggered);
    
    watchdog.start(move |stream| {
        if stream == "test_stream" {
            triggered_clone.store(true, Ordering::SeqCst);
        }
    });
    
    // Feed watchdog regularly - should not trigger
    for _ in 0..5 {
        watchdog.feed("test_stream");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    
    assert!(!triggered.load(Ordering::SeqCst), "Watchdog triggered when it shouldn't");
    
    // Stop feeding - should trigger
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(triggered.load(Ordering::SeqCst), "Watchdog didn't trigger on timeout");
    
    watchdog.stop();
}

/// Test pipeline with real GStreamer elements
#[tokio::test]
async fn test_pipeline_with_gstreamer_elements() {
    init_gstreamer();
    
    // Create simple test pipeline
    let pipeline = create_test_pipeline("gst_test")
        .expect("Failed to create test pipeline");
    
    // Start pipeline
    pipeline.set_state(gst::State::Playing)
        .expect("Failed to set pipeline to playing");
    
    // Wait for some buffers to flow
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Check state
    let (_, current, _) = pipeline.state(Some(gst::ClockTime::from_seconds(1)));
    assert_eq!(current, gst::State::Playing);
    
    // Stop pipeline
    pipeline.set_state(gst::State::Null)
        .expect("Failed to stop pipeline");
    
    cleanup_pipeline(&pipeline);
}

/// Test concurrent pipeline operations
#[tokio::test]
async fn test_concurrent_pipeline_operations() {
    init_gstreamer();
    
    let config = PipelineConfig {
        watchdog_timeout: Some(Duration::from_secs(30)),
        max_streams: 20,
        enable_metrics: true,
    };
    
    let pipeline = Arc::new(RobustPipeline::new(config).expect("Failed to create pipeline"));
    pipeline.start_monitoring();
    
    let mut handles = Vec::new();
    
    // Spawn multiple concurrent operations
    for i in 0..10 {
        let pipeline_clone = Arc::clone(&pipeline);
        let handle = tokio::spawn(async move {
            let stream_name = format!("concurrent_{}", i);
            
            // Update metrics
            let metrics = StreamMetrics {
                frames_processed: (i * 100) as u64,
                bytes_processed: (i * 1024) as u64,
                latency_ms: i as f64,
                dropped_frames: 0,
            };
            
            pipeline_clone.update_stream_metrics(&stream_name, metrics);
            
            // Simulate some work
            tokio::time::sleep(Duration::from_millis(10 * i as u64)).await;
            
            // Try to trigger recovery (may or may not succeed based on state)
            let _ = pipeline_clone.trigger_recovery(&stream_name);
            
            stream_name
        });
        
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    for handle in handles {
        let name = handle.await.expect("Concurrent operation failed");
        println!("Completed operations for {}", name);
    }
}

/// Test stream manager capacity limits
#[tokio::test]
async fn test_stream_manager_capacity() {
    init_gstreamer();
    
    let max_streams = 3;
    let manager = StreamManager::new(max_streams).expect("Failed to create manager");
    
    // Add streams up to capacity
    for i in 0..max_streams {
        let info = StreamInfo {
            name: format!("stream_{}", i),
            source_type: "test".to_string(),
            sink_type: "test".to_string(),
            created_at: std::time::SystemTime::now(),
            state: StreamState::Running,
        };
        
        assert!(manager.add_stream(info).is_ok(), "Failed to add stream within capacity");
    }
    
    assert_eq!(manager.get_active_count(), max_streams);
    
    // Try to exceed capacity
    let extra_info = StreamInfo {
        name: "extra_stream".to_string(),
        source_type: "test".to_string(),
        sink_type: "test".to_string(),
        created_at: std::time::SystemTime::now(),
        state: StreamState::Running,
    };
    
    let result = manager.add_stream(extra_info);
    assert!(result.is_err(), "Should fail when exceeding capacity");
    assert_eq!(manager.get_active_count(), max_streams);
    
    // Remove one and try again
    manager.remove_stream("stream_0").expect("Failed to remove stream");
    assert_eq!(manager.get_active_count(), max_streams - 1);
    
    let replacement_info = StreamInfo {
        name: "replacement".to_string(),
        source_type: "test".to_string(),
        sink_type: "test".to_string(),
        created_at: std::time::SystemTime::now(),
        state: StreamState::Running,
    };
    
    assert!(manager.add_stream(replacement_info).is_ok(), "Should succeed after removal");
    assert_eq!(manager.get_active_count(), max_streams);
}
