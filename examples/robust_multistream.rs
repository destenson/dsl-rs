use std::time::Duration;

use dsl_rs::{init_gstreamer, init_logging, DslResult, core::PipelineConfig};
use dsl_rs::core::PipelineConfig;
use dsl_rs::pipeline::Pipeline;
use dsl_rs::stream::stream_manager::StreamManager;
use dsl_rs::source::{FileSource, RtspSource};
use dsl_rs::sink::FileSink;
use dsl_rs::health::health_monitor::{HealthMonitor, MonitorConfig};
use dsl_rs::recovery::recovery_manager::RecoveryManager;

use std::sync::Arc;
use std::path::PathBuf;
use tracing::info;

fn main() -> DslResult<()> {
    // Initialize logging and GStreamer
    init_logging();
    init_gstreamer()?;
    
    info!("Starting DSL-RS Robust Multi-Stream Example");
    
    // Create pipeline with configuration
    let pipeline_config = PipelineConfig{
        name: "multistream_demo".to_string(),
        max_streams: 8,
        ..Default::default()
    };
    
    let mut pipeline = Pipeline::new(pipeline_config)?;
    
    // Create stream manager and health monitor
    let stream_manager = Arc::new(StreamManager::new(Arc::new(pipeline)));
    let health_monitor = HealthMonitor::new(MonitorConfig::default());
    
    // Start health monitoring
    health_monitor.start_monitoring();
    
    // Add multiple sources - mix of file and RTSP
    let sources = vec![
        // Simulated RTSP sources (would be real cameras in production)
        ("camera_1", "rtsp://localhost:8554/stream1"),
        ("camera_2", "rtsp://localhost:8554/stream2"),
        ("camera_3", "rtsp://localhost:8554/stream3"),
    ];
    
    // Add RTSP sources
    for (name, uri) in sources {
        info!("Adding RTSP source: {} from {}", name, uri);

        let source = Box::new(RtspSource::new(
            name.to_string(),
            uri.to_string()
        )?);
        
        // Add sink for recording
        let rotation_config = RotationConfig{
            base_filename: name.to_string(),
            directory: PathBuf::from("recordings"),
            enable_size_rotation: true,
            max_file_size: 100 * 1024 * 1024, // 100MB
            ..Default::default()
        };
        
        let sink = Box::new(FileSink::new(
            format!("{}_recorder", name),
            rotation_config
        )?);
        
        // Add stream to manager (simplified for now)
        // In full implementation, would connect source to sink through stream manager
    }
    
    // Start the pipeline
    info!("Starting pipeline");
    pipeline.start()?;
    
    // Simulate running for a period
    info!("Pipeline running... Press Ctrl+C to stop");
    
    // Main loop
    let main_loop = gstreamer::glib::MainLoop::new(None, false);
    
    // Set up signal handler for graceful shutdown
    let main_loop_clone = main_loop.clone();
    ctrlc::set_handler(move || {
        info!("Shutting down...");
        main_loop_clone.quit();
    }).expect("Error setting Ctrl-C handler");
    
    // Run main loop
    main_loop.run();
    
    // Cleanup
    info!("Stopping pipeline");
    pipeline.stop()?;
    health_monitor.stop_monitoring();
    
    // Print final health report
    let report = health_monitor.generate_report();
    info!("Final health report:");
    info!("  Overall status: {:?}", report.overall_health);
    info!("  Total streams: {}", report.system_metrics.total_streams);
    info!("  Active streams: {}", report.system_metrics.active_streams);
    info!("  Pipeline uptime: {:?}", report.system_metrics.pipeline_uptime);
    
    info!("DSL-RS example completed successfully");
    Ok(())
}
