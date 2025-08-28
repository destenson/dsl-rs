use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use gstreamer as gst;
use thiserror::Error;
use tracing::{debug, error, info, warn};

#[derive(Error, Debug, Clone)]
pub enum DslError {
    #[error("Pipeline error: {0}")]
    Pipeline(String),
    
    #[error("Stream error: {0}")]
    Stream(String),
    
    #[error("Source error: {0}")]
    Source(String),
    
    #[error("Sink error: {0}")]
    Sink(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("File I/O error: {0}")]
    FileIo(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("State transition error: {0}")]
    StateTransition(String),
    
    #[error("Resource exhaustion: {0}")]
    ResourceExhaustion(String),
    
    #[error("Recovery failed: {0}")]
    RecoveryFailed(String),
    
    #[error("GStreamer error: {0}")]
    GStreamer(#[from] gst::glib::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}

pub type DslResult<T> = Result<T, DslError>;

pub fn init_logging() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();
    
    info!("DSL-RS logging initialized");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Idle,
    Starting,
    Running,
    Paused,
    Recovering,
    Failed,
    Stopped,
}

impl fmt::Display for StreamState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamState::Idle => write!(f, "Idle"),
            StreamState::Starting => write!(f, "Starting"),
            StreamState::Running => write!(f, "Running"),
            StreamState::Paused => write!(f, "Paused"),
            StreamState::Recovering => write!(f, "Recovering"),
            StreamState::Failed => write!(f, "Failed"),
            StreamState::Stopped => write!(f, "Stopped"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamMetrics {
    pub fps: f64,
    pub bitrate: u64,
    pub frames_processed: u64,
    pub frames_dropped: u64,
    pub errors: u64,
    pub uptime: Duration,
    pub last_frame_time: Option<std::time::Instant>,
}

impl Default for StreamMetrics {
    fn default() -> Self {
        Self {
            fps: 0.0,
            bitrate: 0,
            frames_processed: 0,
            frames_dropped: 0,
            errors: 0,
            uptime: Duration::ZERO,
            last_frame_time: None,
        }
    }
}

#[async_trait]
pub trait Source: Send + Sync {
    fn name(&self) -> &str;
    
    fn element(&self) -> &gst::Element;
    
    async fn connect(&mut self) -> DslResult<()>;
    
    async fn disconnect(&mut self) -> DslResult<()>;
    
    fn state(&self) -> StreamState;
    
    fn metrics(&self) -> StreamMetrics;
    
    fn set_retry_config(&mut self, config: RetryConfig);
    
    async fn handle_error(&mut self, error: DslError) -> DslResult<RecoveryAction>;
}

#[async_trait]
pub trait Sink: Send + Sync {
    fn name(&self) -> &str;
    
    fn element(&self) -> &gst::Element;
    
    async fn prepare(&mut self) -> DslResult<()>;
    
    async fn cleanup(&mut self) -> DslResult<()>;
    
    fn state(&self) -> StreamState;
    
    fn metrics(&self) -> StreamMetrics;
    
    async fn handle_error(&mut self, error: DslError) -> DslResult<RecoveryAction>;
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f64,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            exponential_base: 2.0,
            jitter: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecoveryAction {
    Retry,
    Restart,
    Replace,
    Remove,
    Ignore,
    Escalate,
}

pub trait RecoveryStrategy: Send + Sync {
    fn decide_action(&self, error: &DslError, attempt: u32) -> RecoveryAction;
    
    fn calculate_delay(&self, attempt: u32) -> Duration;
    
    fn should_circuit_break(&self, recent_failures: u32) -> bool;
}

#[derive(Debug)]
pub struct StreamHealth {
    pub state: StreamState,
    pub metrics: StreamMetrics,
    pub last_error: Option<DslError>,
    pub consecutive_errors: u32,
    pub recovery_attempts: u32,
}

impl StreamHealth {
    pub fn new() -> Self {
        Self {
            state: StreamState::Idle,
            metrics: StreamMetrics::default(),
            last_error: None,
            consecutive_errors: 0,
            recovery_attempts: 0,
        }
    }
    
    pub fn is_healthy(&self) -> bool {
        matches!(self.state, StreamState::Running | StreamState::Paused) 
            && self.consecutive_errors < 3
    }
}

pub fn init_gstreamer() -> DslResult<()> {
    gst::init().map_err(|e| DslError::GStreamer(e))?;
    info!("GStreamer initialized successfully");
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub name: String,
    pub enable_watchdog: bool,
    pub watchdog_timeout: Duration,
    pub max_streams: usize,
    pub enable_metrics: bool,
    pub metrics_interval: Duration,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            name: "dsl-pipeline".to_string(),
            enable_watchdog: true,
            watchdog_timeout: Duration::from_secs(10),
            max_streams: 32,
            enable_metrics: true,
            metrics_interval: Duration::from_secs(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_state_display() {
        assert_eq!(format!("{}", StreamState::Running), "Running");
        assert_eq!(format!("{}", StreamState::Failed), "Failed");
    }

    #[test]
    fn test_retry_config_defaults() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 10);
        assert_eq!(config.initial_delay, Duration::from_millis(100));
    }

    #[test]
    fn test_stream_health_healthy_check() {
        let mut health = StreamHealth::new();
        assert!(!health.is_healthy());
        
        health.state = StreamState::Running;
        assert!(health.is_healthy());
        
        health.consecutive_errors = 5;
        assert!(!health.is_healthy());
    }
}