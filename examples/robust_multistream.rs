use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use dsl_rs::core::{DslResult, PipelineConfig};
use dsl_rs::pipeline::robust_pipeline::RobustPipeline;
use dsl_rs::sink::file_sink_robust::{FileSinkRobust, RotationConfig};
use dsl_rs::source::file_source_robust::FileSourceRobust;
use dsl_rs::stream::stream_manager::{StreamConfig, StreamManager};
use dsl_rs::{init_gstreamer, init_logging};
use tracing::{info, warn};

fn main() -> DslResult<()> {
    // Initialize logging and GStreamer
    init_logging();
    init_gstreamer()?;

    info!("Starting DSL-RS Robust Multi-Stream Example");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let source_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        // Default to current directory or a test path
        PathBuf::from("./test_videos")
    };

    info!("Using source path: {:?}", source_path);

    // Create pipeline with configuration
    let pipeline_config = PipelineConfig {
        name: "multistream_demo".to_string(),
        max_streams: 8,
        enable_watchdog: true,
        watchdog_timeout: Duration::from_secs(10),
        ..Default::default()
    };

    let pipeline = Arc::new(RobustPipeline::new(pipeline_config)?);

    // Create stream manager
    let stream_manager = Arc::new(StreamManager::new(pipeline.clone()));

    // Start the pipeline
    pipeline.start()?;

    // Collect video files if directory, or use single file
    let mut video_files = Vec::new();

    if source_path.is_dir() {
        info!("Processing directory: {:?}", source_path);

        // Read all video files from directory
        for entry in fs::read_dir(&source_path)
            .map_err(|e| dsl_rs::core::DslError::FileIo(format!("Failed to read directory: {e}")))?
        {
            let entry = entry.map_err(|e| {
                dsl_rs::core::DslError::FileIo(format!("Failed to read entry: {e}"))
            })?;
            let path = entry.path();

            // Check if it's a video file (by extension)
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_str().unwrap_or("").to_lowercase();
                    if matches!(
                        ext_str.as_str(),
                        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "ts" | "m4v"
                    ) {
                        video_files.push(path);
                    }
                }
            }
        }

        if video_files.is_empty() {
            warn!("No video files found in directory: {:?}", source_path);
            warn!("Will create test pattern source instead");
            video_files.push(PathBuf::from("test_pattern.mp4"));
        } else {
            info!("Found {} video files to process", video_files.len());
        }
    } else if source_path.is_file() {
        info!("Processing single file: {:?}", source_path);
        video_files.push(source_path);
    } else {
        warn!("Path does not exist: {:?}", source_path);
        warn!("Will create test pattern source instead");
        video_files.push(PathBuf::from("test_pattern.mp4"));
    }

    // Process each video file
    let mut stream_ids = Vec::new();

    for (index, video_path) in video_files.iter().enumerate() {
        let file_name = video_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&format!("stream_{index}"))
            .to_string();
        info!("Setting up pipeline for: {file_name}");

        // Create file source for this video
        let stream_name = format!("stream_{file_name}");
        let file_source = Box::new(FileSourceRobust::new(
            stream_name.clone(),
            video_path.clone(),
        )?);

        // Add the source to the stream manager
        let stream_config = StreamConfig {
            name: stream_name.clone(),
            ..Default::default()
        };

        // Use futures::executor to run async code in sync context
        let stream_id =
            futures::executor::block_on(stream_manager.add_source(file_source, stream_config))?;

        info!("Added source stream: {file_name} (ID: {stream_id})");
        stream_ids.push(stream_id.clone());

        // Create a file sink for recording/transcoding
        let recording_config = RotationConfig {
            directory: PathBuf::from("./recordings"),
            base_filename: format!("output_{file_name}"),
            enable_size_rotation: true,
            max_file_size: 100 * 1024 * 1024, // 100MB per file
            enable_time_rotation: false,
            rotation_interval: Duration::from_secs(300),
            max_files: Some(10),
        };

        let file_sink = Box::new(FileSinkRobust::new(
            format!("{stream_name}_sink"),
            recording_config,
        )?);

        // Connect the sink to the stream
        futures::executor::block_on(stream_manager.add_sink(file_sink, &stream_id))?;

        info!("Connected sink to stream: {file_name}");
    }

    info!(
        "All pipelines setup complete. Processing {} streams...",
        stream_ids.len()
    );
    info!("Pipeline is running. Press Ctrl+C to stop");

    // Monitor loop
    let mut iteration = 0;
    loop {
        thread::sleep(Duration::from_secs(2));
        iteration += 1;

        // Check health of all streams
        info!("=== Status Update (iteration {iteration}) ===");
        for (index, stream_id) in stream_ids.iter().enumerate() {
            if let Some(health) = stream_manager.get_stream_health(stream_id) {
                info!(
                    "  Stream {}: State={:?}, Errors={}, Recovery Attempts={}",
                    index, health.state, health.consecutive_errors, health.recovery_attempts
                );

                // Show metrics if available
                if health.metrics.fps > 0.0 || health.metrics.bitrate > 0 {
                    info!(
                        "    Metrics: FPS={:.1}, Bitrate={:.1} kbps",
                        health.metrics.fps,
                        health.metrics.bitrate as f64 / 1024.0
                    );
                }
            }
        }

        // Run for a limited time in this example
        // In production, you'd handle signals properly
        if iteration >= 30 {
            // Run for about 60 seconds
            info!("Demo time limit reached, shutting down...");
            break;
        }
    }

    info!("Shutting down all pipelines...");

    // Clean shutdown of all streams
    for stream_id in stream_ids {
        match futures::executor::block_on(stream_manager.remove_source(&stream_id)) {
            Ok(_) => info!("Successfully removed stream: {stream_id}"),
            Err(e) => warn!("Error removing stream {stream_id}: {:?}", e),
        }
    }

    // Stop the pipeline
    pipeline.stop()?;

    info!("Shutdown complete");
    Ok(())
}
