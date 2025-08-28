use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::thread;

use dsl_rs::core::{DslResult, PipelineConfig};
use dsl_rs::pipeline::robust_pipeline::RobustPipeline;
use dsl_rs::sink::file_sink_robust::{FileSinkRobust, RotationConfig};
use dsl_rs::source::file_source_robust::FileSourceRobust;
use dsl_rs::stream::stream_manager::{StreamManager, StreamConfig};
use dsl_rs::{init_gstreamer, init_logging};
use tracing::info;

fn main() -> DslResult<()> {
    // Initialize logging and GStreamer
    init_logging();
    init_gstreamer()?;

    info!("Starting DSL-RS Robust Multi-Stream Example");

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

    info!("Setting up video pipeline with file source and sink...");
    
    // Create a test video source (will use test pattern since file doesn't exist)
    let stream_name = "test_stream";
    let file_source = Box::new(FileSourceRobust::new(
        stream_name.to_string(),
        PathBuf::from("/path/to/video.mp4"), // This would be a real file
    )?);

    // Add the source to the stream manager
    let stream_config = StreamConfig {
        name: stream_name.to_string(),
        ..Default::default()
    };
    
    // Use futures::executor to run async code in sync context
    let stream_id = futures::executor::block_on(
        stream_manager.add_source(file_source, stream_config)
    )?;
    info!("Added source stream: {}", stream_id);

    // Create a file sink for recording
    let recording_config = RotationConfig {
        directory: PathBuf::from("./recordings"),
        base_filename: "recording".to_string(),
        enable_size_rotation: true,
        max_file_size: 100 * 1024 * 1024, // 100MB
        enable_time_rotation: false,
        rotation_interval: Duration::from_secs(300),
        max_files: Some(10),
    };

    let file_sink = Box::new(FileSinkRobust::new(
        format!("{}_sink", stream_name),
        recording_config,
    )?);

    // Connect the sink to the stream
    futures::executor::block_on(
        stream_manager.add_sink(file_sink, &stream_id)
    )?;
    info!("Connected sink to stream");

    info!("Pipeline is running. Press Ctrl+C to stop");
    
    // Simple loop to keep the pipeline running
    // In production, you'd use a proper event loop or signal handler
    loop {
        thread::sleep(Duration::from_secs(1));
        
        // Check stream health
        if let Some(health) = stream_manager.get_stream_health(&stream_id) {
            info!("Stream health: {:?}", health.state);
        }
        
        // Check for Ctrl+C (simplified - in production use proper signal handling)
        // For now, just run for 10 seconds as a demonstration
        static mut COUNTER: u32 = 0;
        unsafe {
            COUNTER += 1;
            if COUNTER >= 10 {
                break;
            }
        }
    }
    
    info!("Shutting down pipeline...");

    // Clean shutdown
    futures::executor::block_on(
        stream_manager.remove_source(&stream_id)
    )?;
    
    // Stop the pipeline
    pipeline.stop()?;

    info!("Shutdown complete");
    Ok(())
}