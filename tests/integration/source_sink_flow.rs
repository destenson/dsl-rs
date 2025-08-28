use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::thread;
use tempfile::tempdir;

use dsl_rs::core::{DslResult, StreamState, PipelineConfig};
use dsl_rs::pipeline::robust_pipeline::RobustPipeline;
use dsl_rs::sink::file_sink_robust::{FileSinkRobust, RotationConfig};
use dsl_rs::source::file_source_robust::FileSourceRobust;
use dsl_rs::stream::stream_manager::{StreamManager, StreamConfig};
use dsl_rs::init_gstreamer;
use futures::executor::block_on;

/// Test basic source to sink connectivity
#[test]
fn test_source_to_sink_linking() -> DslResult<()> {
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
    let stream_config = StreamConfig {
        name: stream_name.to_string(),
        ..Default::default()
    };
    let stream_id = block_on(stream_manager.add_source(file_source, stream_config))?;

    // Create a temporary directory for output
    let temp_dir = tempdir().unwrap();
    let recording_config = RotationConfig {
        directory: temp_dir.path().to_path_buf(),
        base_filename: "test_recording".to_string(),
        enable_size_rotation: false,
        max_file_size: 100 * 1024 * 1024, // 100MB
        enable_time_rotation: false,
        rotation_interval: Duration::from_secs(60),
        max_files: None,
    };

    // Create and connect file sink
    let file_sink = Box::new(FileSinkRobust::new(
        "test_sink".to_string(),
        recording_config,
    )?);

    block_on(stream_manager.add_sink(file_sink, &stream_id))?;

    // Let it run briefly
    thread::sleep(Duration::from_secs(2));

    // Check stream health
    let health = stream_manager
        .get_stream_health(&stream_id)
        .expect("Stream should exist");

    // Verify stream is running or at least idle (not in error)
    assert!(
        matches!(health.state, StreamState::Running | StreamState::Idle | StreamState::Starting),
        "Stream should not be in error state: {:?}",
        health.state
    );

    // Clean up
    block_on(stream_manager.remove_source(&stream_id))?;
    pipeline.stop()?;

    Ok(())
}

/// Test multiple streams with different sinks
#[test]
fn test_multiple_streams() -> DslResult<()> {
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

    let mut stream_ids = Vec::new();
    
    for (stream_name, file_path) in &streams {
        // Add source
        let source = Box::new(FileSourceRobust::new(
            stream_name.to_string(),
            PathBuf::from(file_path),
        )?);

        let stream_config = StreamConfig {
            name: stream_name.to_string(),
            ..Default::default()
        };
        let stream_id = block_on(stream_manager.add_source(source, stream_config))?;
        stream_ids.push(stream_id.clone());

        // Add a simple file sink for each
        let temp_dir = tempdir().unwrap();
        let config = RotationConfig {
            directory: temp_dir.path().to_path_buf(),
            base_filename: stream_name.to_string(),
            enable_size_rotation: false,
            max_file_size: 100 * 1024 * 1024,
            enable_time_rotation: false,
            rotation_interval: Duration::from_secs(60),
            max_files: None,
        };

        let sink = Box::new(FileSinkRobust::new(
            format!("{}_sink", stream_name),
            config,
        )?);

        block_on(stream_manager.add_sink(sink, &stream_id))?;
    }

    // Verify all streams are present
    let active_streams = stream_manager.list_streams();
    assert_eq!(active_streams.len(), 3);

    // Let them run
    thread::sleep(Duration::from_secs(1));

    // Remove one stream and verify others continue
    block_on(stream_manager.remove_source(&stream_ids[1]))?;
    let remaining_streams = stream_manager.list_streams();
    assert_eq!(remaining_streams.len(), 2);
    assert!(remaining_streams.contains(&stream_ids[0]));
    assert!(remaining_streams.contains(&stream_ids[2]));

    // Clean up remaining streams
    for stream_id in &[stream_ids[0].clone(), stream_ids[2].clone()] {
        block_on(stream_manager.remove_source(stream_id))?;
    }

    pipeline.stop()?;

    Ok(())
}