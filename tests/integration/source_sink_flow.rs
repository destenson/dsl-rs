use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

use dsl_rs::core::{DslResult, StreamState};
use dsl_rs::pipeline::robust_pipeline::{PipelineConfig, RobustPipeline};
use dsl_rs::sink::file_sink_robust::{FileSinkRobust, RotationConfig};
use dsl_rs::sink::rtsp_sink_robust::{RtspServerConfig, RtspSinkRobust};
use dsl_rs::source::file_source_robust::FileSourceRobust;
use dsl_rs::source::rtsp_source_robust::{RtspConfig, RtspSourceRobust};
use dsl_rs::stream::stream_manager::StreamManager;
use dsl_rs::init_gstreamer;

/// Test basic source to sink connectivity
#[tokio::test]
async fn test_source_to_sink_linking() -> DslResult<()> {
    init_gstreamer().ok();

    // Create pipeline
    let pipeline_config = PipelineConfig {
        name: "test_pipeline".to_string(),
        max_streams: 2,
        enable_watchdog: false,
        ..Default::default()
    };

    let pipeline = Arc::new(RobustPipeline::new(pipeline_config)?);
    let stream_manager = Arc::new(StreamManager::new(pipeline.clone()));

    // Start pipeline
    pipeline.start()?;

    // Create a test video source (using videotestsrc internally)
    let stream_name = "test_stream";
    let file_source = Box::new(FileSourceRobust::new(
        stream_name.to_string(),
        PathBuf::from("test.mp4"), // Will use test pattern since file doesn't exist
    )?);

    // Add source to stream
    stream_manager.add_stream(stream_name, file_source).await?;

    // Create a temporary directory for output
    let temp_dir = tempdir().unwrap();
    let recording_config = RotationConfig {
        directory: temp_dir.path().to_path_buf(),
        base_filename: "test_recording".to_string(),
        max_file_size: None,
        max_duration: None,
        max_files: None,
        min_free_space: 1024 * 1024, // 1MB
    };

    // Create and connect file sink
    let file_sink = Box::new(FileSinkRobust::new(
        "test_sink".to_string(),
        recording_config,
    )?);

    stream_manager.add_sink(stream_name, file_sink).await?;

    // Let it run briefly
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check stream health
    let health = stream_manager
        .get_stream_health(stream_name)
        .expect("Stream should exist");

    // Verify stream is running or at least idle (not in error)
    assert!(
        matches!(health.state, StreamState::Running | StreamState::Idle | StreamState::Starting),
        "Stream should not be in error state: {:?}",
        health.state
    );

    // Clean up
    stream_manager.remove_source(stream_name).await?;
    pipeline.stop()?;

    Ok(())
}

/// Test RTSP source to RTSP sink flow
#[tokio::test]
async fn test_rtsp_to_rtsp_flow() -> DslResult<()> {
    init_gstreamer().ok();

    // Create pipeline
    let pipeline_config = PipelineConfig {
        name: "rtsp_test_pipeline".to_string(),
        max_streams: 2,
        enable_watchdog: false,
        ..Default::default()
    };

    let pipeline = Arc::new(RobustPipeline::new(pipeline_config)?);
    let stream_manager = Arc::new(StreamManager::new(pipeline.clone()));

    // Start pipeline
    pipeline.start()?;

    // Create RTSP source
    let stream_name = "rtsp_stream";
    let rtsp_config = RtspConfig {
        uri: "rtsp://localhost:8554/test".to_string(),
        protocols: 0x00000004, // TCP
        latency: 100,
        timeout: 1_000_000, // 1 second for testing
        reconnect_timeout: 1_000_000,
        tcp_timeout: 1_000_000,
        buffer_mode: 3, // auto
        ntp_sync: false,
        retry_on_401: false,
        user_agent: None,
        user_id: None,
        user_password: None,
    };

    let rtsp_source = Box::new(RtspSourceRobust::with_config(
        stream_name.to_string(),
        rtsp_config,
    )?);

    // Add source
    stream_manager.add_stream(stream_name, rtsp_source).await?;

    // Create RTSP server sink
    let server_config = RtspServerConfig {
        port: 8559, // Different port for testing
        mount_point: "/test_output".to_string(),
        protocols: 0x00000007, // All protocols
        max_clients: Some(5),
        enable_authentication: false,
        username: None,
        password: None,
        multicast_address: None,
        enable_rate_adaptation: true,
        key_frame_interval: 2,
    };

    let rtsp_sink = Box::new(RtspSinkRobust::new(
        "rtsp_test_sink".to_string(),
        server_config,
    )?);

    // Connect sink
    stream_manager.add_sink(stream_name, rtsp_sink).await?;

    // Let it run briefly
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Check that stream was added
    let streams = stream_manager.list_streams();
    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0], stream_name);

    // Clean up
    stream_manager.remove_source(stream_name).await?;
    pipeline.stop()?;

    Ok(())
}

/// Test multiple streams with different sinks
#[tokio::test]
async fn test_multiple_streams() -> DslResult<()> {
    init_gstreamer().ok();

    // Create pipeline with support for multiple streams
    let pipeline_config = PipelineConfig {
        name: "multi_stream_pipeline".to_string(),
        max_streams: 4,
        enable_watchdog: false,
        ..Default::default()
    };

    let pipeline = Arc::new(RobustPipeline::new(pipeline_config)?);
    let stream_manager = Arc::new(StreamManager::new(pipeline.clone()));

    // Start pipeline
    pipeline.start()?;

    // Add multiple streams
    let streams = vec![
        ("stream1", "test1.mp4"),
        ("stream2", "test2.mp4"),
        ("stream3", "test3.mp4"),
    ];

    for (stream_name, file_path) in &streams {
        // Add source
        let source = Box::new(FileSourceRobust::new(
            stream_name.to_string(),
            PathBuf::from(file_path),
        )?);

        stream_manager.add_stream(stream_name, source).await?;

        // Add a simple file sink for each
        let temp_dir = tempdir().unwrap();
        let config = RotationConfig {
            directory: temp_dir.path().to_path_buf(),
            base_filename: stream_name.to_string(),
            ..Default::default()
        };

        let sink = Box::new(FileSinkRobust::new(
            format!("{}_sink", stream_name),
            config,
        )?);

        stream_manager.add_sink(stream_name, sink).await?;
    }

    // Verify all streams are present
    let active_streams = stream_manager.list_streams();
    assert_eq!(active_streams.len(), 3);

    // Let them run
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Remove one stream and verify others continue
    stream_manager.remove_source("stream2").await?;
    let remaining_streams = stream_manager.list_streams();
    assert_eq!(remaining_streams.len(), 2);
    assert!(remaining_streams.contains(&"stream1".to_string()));
    assert!(remaining_streams.contains(&"stream3".to_string()));

    // Clean up remaining streams
    for stream_name in remaining_streams {
        stream_manager.remove_source(&stream_name).await?;
    }

    pipeline.stop()?;

    Ok(())
}

/// Test stream recovery after error
#[tokio::test]
async fn test_stream_recovery() -> DslResult<()> {
    init_gstreamer().ok();

    // Create pipeline
    let pipeline_config = PipelineConfig {
        name: "recovery_test_pipeline".to_string(),
        max_streams: 2,
        enable_watchdog: true,
        watchdog_timeout: Duration::from_secs(5),
        ..Default::default()
    };

    let pipeline = Arc::new(RobustPipeline::new(pipeline_config)?);
    let stream_manager = Arc::new(StreamManager::new(pipeline.clone()));

    // Start pipeline
    pipeline.start()?;

    // Add a stream that will fail (non-existent RTSP source)
    let stream_name = "failing_stream";
    let rtsp_config = RtspConfig {
        uri: "rtsp://non-existent-host:554/stream".to_string(),
        protocols: 0x00000004,
        latency: 100,
        timeout: 1_000_000, // Quick timeout for testing
        reconnect_timeout: 1_000_000,
        tcp_timeout: 1_000_000,
        buffer_mode: 3,
        ntp_sync: false,
        retry_on_401: false,
        user_agent: None,
        user_id: None,
        user_password: None,
    };

    let rtsp_source = Box::new(RtspSourceRobust::with_config(
        stream_name.to_string(),
        rtsp_config,
    )?);

    // Add the failing source
    stream_manager.add_stream(stream_name, rtsp_source).await?;

    // Give it time to fail and attempt recovery
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Try to handle the error
    let result = stream_manager.handle_stream_error(stream_name).await;
    
    // Error handling should succeed (even if stream doesn't recover)
    assert!(result.is_ok(), "Error handling should not panic");

    // Clean up
    stream_manager.remove_source(stream_name).await.ok();
    pipeline.stop()?;

    Ok(())
}