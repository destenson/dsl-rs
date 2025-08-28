use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use gstreamer as gst;
use gstreamer::prelude::*;
use tracing::{debug, error, info, warn};

use crate::core::{
    DslError, DslResult, RecoveryAction, RetryConfig, Source, StreamMetrics, StreamState,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

#[derive(Debug, Clone)]
pub struct RtspConfig {
    pub uri: String,
    pub protocols: u32,         // GstRTSPLowerTrans flags
    pub latency: u32,           // milliseconds
    pub timeout: u64,           // microseconds
    pub reconnect_timeout: u64, // microseconds
    pub tcp_timeout: u64,       // microseconds
    pub buffer_mode: i32,       // 0=none, 1=slave, 2=buffer, 3=auto, 4=synced
    pub ntp_sync: bool,
    pub retry_on_401: bool,
    pub user_agent: Option<String>,
    pub user_id: Option<String>,
    pub user_password: Option<String>,
}

impl Default for RtspConfig {
    fn default() -> Self {
        Self {
            uri: String::new(),
            protocols: 0x00000004, // TCP
            latency: 100,
            timeout: 5_000_000,           // 5 seconds
            reconnect_timeout: 5_000_000, // 5 seconds
            tcp_timeout: 5_000_000,       // 5 seconds
            buffer_mode: 3,               // auto
            ntp_sync: false,
            retry_on_401: true,
            user_agent: Some("dsl-rs/1.0".to_string()),
            user_id: None,
            user_password: None,
        }
    }
}

pub struct RtspSourceRobust {
    name: String,
    config: RtspConfig,
    element: gst::Element,
    state: Arc<Mutex<StreamState>>,
    connection_state: Arc<Mutex<ConnectionState>>,
    metrics: Arc<Mutex<StreamMetrics>>,
    retry_config: RetryConfig,
    last_connect_attempt: Arc<Mutex<Instant>>,
    consecutive_failures: Arc<Mutex<u32>>,
    total_reconnects: Arc<Mutex<u32>>,
}

impl RtspSourceRobust {
    pub fn new(name: String, uri: String) -> DslResult<Self> {
        let mut config = RtspConfig {
            uri,
            ..Default::default()
        };
        Self::with_config(name, config)
    }

    pub fn with_config(name: String, config: RtspConfig) -> DslResult<Self> {
        // Create rtspsrc element
        let rtspsrc = gst::ElementFactory::make("rtspsrc")
            .name(format!("{}_rtspsrc", name))
            .property("location", &config.uri)
            .property("latency", config.latency)
            .property("timeout", config.timeout)
            .property("tcp-timeout", config.tcp_timeout)
            .property("ntp-sync", config.ntp_sync)
            .property("connection-speed", 1000u64)
            .property("drop-on-latency", true)
            .property("do-retransmission", true)
            .build()
            .map_err(|_| DslError::Source("Failed to create rtspsrc".to_string()))?;

        // Set enum properties using string representation
        // TCP = 0x4, so we use "tcp" string
        rtspsrc.set_property_from_str("protocols", "tcp");
        // buffer-mode: 0=none, 1=slave, 2=buffer, 3=auto, 4=synced
        let buffer_mode_str = match config.buffer_mode {
            0 => "none",
            1 => "slave",
            2 => "buffer",
            3 => "auto",
            4 => "synced",
            _ => "auto",
        };
        rtspsrc.set_property_from_str("buffer-mode", buffer_mode_str);

        // Set optional properties
        if let Some(ref agent) = config.user_agent {
            rtspsrc.set_property("user-agent", agent);
        }
        if let Some(ref user) = config.user_id {
            rtspsrc.set_property("user-id", user);
        }
        if let Some(ref pass) = config.user_password {
            rtspsrc.set_property("user-pw", pass);
        }

        Ok(Self {
            name,
            config,
            element: rtspsrc,
            state: Arc::new(Mutex::new(StreamState::Idle)),
            connection_state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            metrics: Arc::new(Mutex::new(StreamMetrics::default())),
            retry_config: RetryConfig::default(),
            last_connect_attempt: Arc::new(Mutex::new(Instant::now())),
            consecutive_failures: Arc::new(Mutex::new(0)),
            total_reconnects: Arc::new(Mutex::new(0)),
        })
    }

    async fn setup_signal_handlers(&self) {
        let element = self.element.clone();
        let name = self.name.clone();
        let connection_state = Arc::clone(&self.connection_state);
        let metrics = Arc::clone(&self.metrics);

        // Handle pad-added signal for dynamic pads
        element.connect_pad_added(move |_src, pad| {
            debug!("New pad added for RTSP source {}: {}", name, pad.name());
            // In production, would link to appropriate downstream element
        });

        // Handle on-sdp signal for session info
        let name_sdp = self.name.clone();
        element.connect("on-sdp", false, move |_values| {
            info!("Received SDP for {}", name_sdp);
            None
        });

        // Handle select-stream signal
        let name_stream = self.name.clone();
        element.connect("select-stream", false, move |values| {
            if let (Some(num), Some(caps)) = (
                values[1].get::<u32>().ok(),
                values[2].get::<gst::Caps>().ok(),
            ) {
                debug!("Stream {} selected for {}: {:?}", num, name_stream, caps);
            }
            Some(true.to_value())
        });
    }

    async fn attempt_connection(&mut self) -> DslResult<()> {
        *self.connection_state.lock().unwrap() = ConnectionState::Connecting;
        *self.last_connect_attempt.lock().unwrap() = Instant::now();

        info!("Attempting to connect to RTSP source: {}", self.config.uri);

        // Set to playing state
        match self.element.set_state(gst::State::Playing) {
            Ok(_) => {
                // Wait a bit to see if connection succeeds
                std::thread::sleep(Duration::from_millis(100));

                // Check state
                let (_, current, _) = self.element.state(Some(gst::ClockTime::from_seconds(1)));
                if current == gst::State::Playing {
                    *self.connection_state.lock().unwrap() = ConnectionState::Connected;
                    *self.consecutive_failures.lock().unwrap() = 0;
                    info!("Successfully connected to RTSP source: {}", self.name);
                    Ok(())
                } else {
                    *self.connection_state.lock().unwrap() = ConnectionState::Failed;
                    Err(DslError::Network(format!(
                        "Failed to reach playing state for {}",
                        self.name
                    )))
                }
            }
            Err(e) => {
                *self.connection_state.lock().unwrap() = ConnectionState::Failed;
                *self.consecutive_failures.lock().unwrap() += 1;
                Err(DslError::Network(format!(
                    "Failed to connect to RTSP source {}: {}",
                    self.name, e
                )))
            }
        }
    }

    async fn reconnect_with_backoff(&mut self) -> DslResult<()> {
        let mut attempt = 0u32;

        while attempt < self.retry_config.max_attempts {
            *self.connection_state.lock().unwrap() = ConnectionState::Reconnecting;

            // Calculate delay with exponential backoff
            let delay = self.calculate_retry_delay(attempt);

            info!(
                "Reconnection attempt {} for {} in {:?}",
                attempt + 1,
                self.name,
                delay
            );

            std::thread::sleep(delay);

            // Try to reconnect
            match self.attempt_connection().await {
                Ok(()) => {
                    *self.total_reconnects.lock().unwrap() += 1;
                    return Ok(());
                }
                Err(e) => {
                    warn!(
                        "Reconnection attempt {} failed for {}: {:?}",
                        attempt + 1,
                        self.name,
                        e
                    );
                    attempt += 1;
                }
            }
        }

        *self.connection_state.lock().unwrap() = ConnectionState::Failed;
        Err(DslError::RecoveryFailed(format!(
            "Failed to reconnect after {} attempts",
            self.retry_config.max_attempts
        )))
    }

    fn calculate_retry_delay(&self, attempt: u32) -> Duration {
        let base_delay = self.retry_config.initial_delay.as_millis() as f64;
        let exp_delay = base_delay * self.retry_config.exponential_base.powi(attempt as i32);
        let clamped_delay = exp_delay.min(self.retry_config.max_delay.as_millis() as f64);

        let delay = if self.retry_config.jitter {
            // Add jitter: +/- 20%
            let jitter = clamped_delay * 0.2 * (rand::random::<f64>() - 0.5);
            (clamped_delay + jitter).max(0.0)
        } else {
            clamped_delay
        };

        Duration::from_millis(delay as u64)
    }

    pub fn get_connection_state(&self) -> ConnectionState {
        self.connection_state.lock().unwrap().clone()
    }

    pub fn get_total_reconnects(&self) -> u32 {
        *self.total_reconnects.lock().unwrap()
    }

    fn classify_network_error(&self, error_msg: &str) -> RecoveryAction {
        if error_msg.contains("401") && self.config.retry_on_401 {
            // Authentication error - might need new credentials
            RecoveryAction::Replace
        } else if error_msg.contains("timeout") || error_msg.contains("Timeout") {
            // Timeout - worth retrying
            RecoveryAction::Retry
        } else if error_msg.contains("404") {
            // Stream not found - no point retrying
            RecoveryAction::Remove
        } else if error_msg.contains("connection refused") {
            // Server down - retry with backoff
            RecoveryAction::Retry
        } else {
            // Unknown error - try restart
            RecoveryAction::Restart
        }
    }
}

#[async_trait]
impl Source for RtspSourceRobust {
    fn name(&self) -> &str {
        &self.name
    }

    fn element(&self) -> &gst::Element {
        &self.element
    }

    async fn connect(&mut self) -> DslResult<()> {
        *self.state.lock().unwrap() = StreamState::Starting;

        // Setup signal handlers
        self.setup_signal_handlers().await;

        // Attempt initial connection
        match self.attempt_connection().await {
            Ok(()) => {
                *self.state.lock().unwrap() = StreamState::Running;
                Ok(())
            }
            Err(e) => {
                *self.state.lock().unwrap() = StreamState::Failed;
                Err(e)
            }
        }
    }

    async fn disconnect(&mut self) -> DslResult<()> {
        *self.state.lock().unwrap() = StreamState::Stopped;
        *self.connection_state.lock().unwrap() = ConnectionState::Disconnected;

        // Stop the element
        self.element
            .set_state(gst::State::Null)
            .map_err(|_| DslError::Source("Failed to stop RTSP source".to_string()))?;

        info!("RTSP source {} disconnected", self.name);
        Ok(())
    }

    fn state(&self) -> StreamState {
        *self.state.lock().unwrap()
    }

    fn metrics(&self) -> StreamMetrics {
        self.metrics.lock().unwrap().clone()
    }

    fn set_retry_config(&mut self, config: RetryConfig) {
        self.retry_config = config;
    }

    async fn handle_error(&mut self, error: DslError) -> DslResult<RecoveryAction> {
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.errors += 1;
        }

        match error {
            DslError::Network(ref msg) => {
                warn!("Network error for {}: {}", self.name, msg);

                // Try to reconnect with backoff
                match self.reconnect_with_backoff().await {
                    Ok(()) => {
                        *self.state.lock().unwrap() = StreamState::Running;
                        Ok(RecoveryAction::Ignore)
                    }
                    Err(_) => {
                        *self.state.lock().unwrap() = StreamState::Failed;
                        Ok(self.classify_network_error(msg))
                    }
                }
            }
            _ => {
                // For other errors, attempt reconnection
                if let Ok(()) = self.reconnect_with_backoff().await {
                    *self.state.lock().unwrap() = StreamState::Running;
                    Ok(RecoveryAction::Ignore)
                } else {
                    *self.state.lock().unwrap() = StreamState::Failed;
                    Ok(RecoveryAction::Restart)
                }
            }
        }
    }
}

impl Drop for RtspSourceRobust {
    fn drop(&mut self) {
        let _ = self.element.set_state(gst::State::Null);
    }
}

// Helper function for random jitter (simple implementation)
mod rand {
    pub fn random<T>() -> T
    where
        T: From<f64>,
    {
        // Simple pseudo-random for jitter
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        let seed = time.as_nanos() as f64 / 1_000_000_000.0;
        T::from((seed * 1000.0) % 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtsp_config_defaults() {
        let config = RtspConfig::default();
        assert_eq!(config.protocols, 0x00000004); // TCP
        assert_eq!(config.latency, 100);
        assert_eq!(config.buffer_mode, 3); // auto
    }

    #[tokio::test]
    async fn test_rtsp_source_creation() {
        gst::init().ok();

        let source = RtspSourceRobust::new(
            "test_rtsp".to_string(),
            "rtsp://example.com/stream".to_string(),
        );

        assert!(source.is_ok());
        let source = source.unwrap();
        assert_eq!(source.name(), "test_rtsp");
        assert_eq!(source.state(), StreamState::Idle);
        assert_eq!(source.get_connection_state(), ConnectionState::Disconnected);
    }

    #[test]
    fn test_retry_delay_calculation() {
        gst::init().ok();

        let source = RtspSourceRobust::new("test".to_string(), "rtsp://test".to_string()).unwrap();

        // Test exponential backoff
        let delay0 = source.calculate_retry_delay(0);
        let delay1 = source.calculate_retry_delay(1);
        let delay2 = source.calculate_retry_delay(2);

        assert!(delay1 > delay0);
        assert!(delay2 > delay1);
        assert!(delay2 <= source.retry_config.max_delay);
    }

    #[test]
    fn test_network_error_classification() {
        gst::init().ok();

        let source = RtspSourceRobust::new("test".to_string(), "rtsp://test".to_string()).unwrap();

        assert_eq!(
            source.classify_network_error("401 Unauthorized"),
            RecoveryAction::Replace
        );
        assert_eq!(
            source.classify_network_error("timeout occurred"),
            RecoveryAction::Retry
        );
        assert_eq!(
            source.classify_network_error("404 Not Found"),
            RecoveryAction::Remove
        );
        assert_eq!(
            source.classify_network_error("connection refused"),
            RecoveryAction::Retry
        );
    }
}
