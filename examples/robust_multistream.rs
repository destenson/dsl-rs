use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_std::task;
use dsl_rs::core::{DslResult, StreamState};
use dsl_rs::health::health_monitor::{HealthMonitor, MonitorConfig};
use dsl_rs::pipeline::robust_pipeline::{PipelineConfig, RobustPipeline};
use dsl_rs::recovery::recovery_manager::{RecoveryManager, RecoveryPolicy};
use dsl_rs::sink::file_sink_robust::{FileSinkRobust, RotationConfig};
use dsl_rs::sink::rtsp_sink_robust::{RtspServerConfig, RtspSinkRobust};
use dsl_rs::source::file_source_robust::FileSourceRobust;
use dsl_rs::source::rtsp_source_robust::{RtspConfig, RtspSourceRobust};
use dsl_rs::stream::stream_manager::StreamManager;
use dsl_rs::{init_gstreamer, init_logging};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> DslResult<()> {
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

    // Create stream manager, recovery manager, and health monitor
    let stream_manager = Arc::new(StreamManager::new(pipeline.clone()));
    let recovery_manager = Arc::new(RecoveryManager::new());
    let health_monitor = Arc::new(HealthMonitor::new(MonitorConfig::default()));

    // Start the pipeline
    pipeline.start()?;

    // Start health monitoring
    let monitor_handle = health_monitor.start_monitoring();

    // Example 1: Add a test video file source with recording sink
    info!("Setting up file source with recording sink...");
    {
        let stream_name = "file_stream";
        
        // Create a test pattern source (since we don't have a real video file)
        // In production, you'd use a real file path
        let file_source = Box::new(FileSourceRobust::new(
            stream_name.to_string(),
            PathBuf::from("/path/to/video.mp4"), // This would be a real file
        )?);

        // Add the source to the stream manager
        stream_manager.add_stream(stream_name, file_source).await?;

        // Register with health monitor
        health_monitor.register_stream(stream_name.to_string());

        // Create a file sink for recording
        let recording_config = RotationConfig {
            directory: PathBuf::from("./recordings"),
            base_filename: "recording".to_string(),
            max_file_size: Some(100 * 1024 * 1024), // 100MB
            max_duration: Some(Duration::from_secs(300)), // 5 minutes
            max_files: Some(10),
            ..Default::default()
        };

        let file_sink = Box::new(FileSinkRobust::new(
            format!("{}_sink", stream_name),
            recording_config,
        )?);

        // Connect the sink to the stream
        stream_manager.add_sink(stream_name, file_sink).await?;

        info!("File pipeline setup complete");
    }

    // Example 2: Add RTSP sources with RTSP server sinks
    info!("Setting up RTSP sources with server sinks...");
    
    // For demonstration, we'll use test sources since real RTSP cameras may not be available
    // In production, these would be real camera URLs
    let rtsp_sources = vec![
        ("camera_1", "rtsp://localhost:8554/test1", 8555),
        ("camera_2", "rtsp://localhost:8554/test2", 8556),
        ("camera_3", "rtsp://localhost:8554/test3", 8557),
    ];

    for (stream_name, uri, server_port) in rtsp_sources {
        info!("Adding RTSP source: {} from {}", stream_name, uri);

        // Configure RTSP source with retry logic
        let rtsp_config = RtspConfig {
            uri: uri.to_string(),
            protocols: 0x00000004, // TCP
            latency: 100,
            timeout: 5_000_000,
            reconnect_timeout: 5_000_000,
            tcp_timeout: 5_000_000,
            buffer_mode: 3, // auto
            ntp_sync: false,
            retry_on_401: true,
            user_agent: Some("dsl-rs/1.0".to_string()),
            user_id: None,
            user_password: None,
        };

        let rtsp_source = Box::new(RtspSourceRobust::with_config(
            stream_name.to_string(),
            rtsp_config,
        )?);

        // Set recovery policy for this source
        recovery_manager.set_recovery_policy(
            stream_name,
            RecoveryPolicy::ExponentialBackoff {
                initial_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(30),
                multiplier: 2.0,
            },
        );

        // Add the source
        stream_manager.add_stream(stream_name, rtsp_source).await?;

        // Register with health monitor
        health_monitor.register_stream(stream_name.to_string());

        // Create RTSP server sink
        let server_config = RtspServerConfig {
            port: server_port,
            mount_point: format!("/{}", stream_name),
            protocols: 0x00000007, // TCP + UDP + UDP_MCAST
            max_clients: Some(10),
            enable_authentication: false,
            username: None,
            password: None,
            multicast_address: None,
            enable_rate_adaptation: true,
            key_frame_interval: 2,
        };

        let rtsp_sink = Box::new(RtspSinkRobust::new(
            format!("{}_server", stream_name),
            server_config,
        )?);

        // Connect the sink
        stream_manager.add_sink(stream_name, rtsp_sink).await?;

        info!("RTSP pipeline for {} setup complete", stream_name);
    }

    // Let the pipelines run for a while
    info!("Pipelines running... Press Ctrl+C to stop");

    // Monitor health and handle errors
    let manager_clone = stream_manager.clone();
    let recovery_clone = recovery_manager.clone();
    let health_clone = health_monitor.clone();
    
    let monitor_task = task::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        
        loop {
            interval.tick().await;
            
            // Check health of all streams
            for stream_name in manager_clone.list_streams() {
                if let Some(health) = manager_clone.get_stream_health(&stream_name) {
                    match health.state {
                        StreamState::Error => {
                            warn!("Stream {} is in error state", stream_name);
                            
                            // Attempt recovery
                            if let Err(e) = manager_clone.handle_stream_error(&stream_name).await {
                                error!("Failed to recover stream {}: {}", stream_name, e);
                            }
                        }
                        StreamState::Stopped => {
                            warn!("Stream {} is stopped, attempting restart", stream_name);
                            // Could implement restart logic here
                        }
                        _ => {
                            // Stream is healthy
                            info!(
                                "Stream {} health: state={:?}, fps={:.1}, bitrate={:.1} kbps",
                                stream_name,
                                health.state,
                                health.metrics.fps,
                                health.metrics.bitrate as f32 / 1024.0
                            );
                        }
                    }
                }
            }
            
            // Get overall health report
            let report = health_clone.get_health_report();
            info!(
                "Overall health: {} healthy streams, {} unhealthy",
                report.healthy_streams, report.unhealthy_streams
            );
        }
    });

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    
    info!("Shutting down pipelines...");

    // Clean shutdown
    for stream_name in stream_manager.list_streams() {
        if let Err(e) = stream_manager.remove_source(&stream_name).await {
            error!("Error removing source {}: {}", stream_name, e);
        }
    }

    // Stop the pipeline
    pipeline.stop()?;

    // Stop health monitoring
    health_monitor.stop_monitoring();

    info!("Shutdown complete");
    Ok(())
}
