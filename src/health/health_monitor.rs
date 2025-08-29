use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use metrics::{counter, gauge, histogram};
use tracing::{debug, error, info, warn};

use crate::core::{DslError, DslResult, StreamHealth, StreamMetrics, StreamState};

#[derive(Debug, Clone)]
pub struct StreamHealthMetrics {
    pub name: String,
    pub state: StreamState,
    pub fps: f64,
    pub bitrate: u64,
    pub frames_processed: u64,
    pub frames_dropped: u64,
    pub errors: u64,
    pub uptime: Duration,
    pub last_activity: Instant,
    pub memory_usage: u64,
    pub cpu_usage: f32,
}

impl Default for StreamHealthMetrics {
    fn default() -> Self {
        Self {
            name: String::new(),
            state: StreamState::Idle,
            fps: 0.0,
            bitrate: 0,
            frames_processed: 0,
            frames_dropped: 0,
            errors: 0,
            uptime: Duration::ZERO,
            last_activity: Instant::now(),
            memory_usage: 0,
            cpu_usage: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthReport {
    pub timestamp: SystemTime,
    pub overall_health: HealthStatus,
    pub stream_health: HashMap<String, StreamHealthMetrics>,
    pub system_metrics: SystemMetrics,
    pub alerts: Vec<HealthAlert>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
}

#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub total_streams: usize,
    pub active_streams: usize,
    pub failed_streams: usize,
    pub total_memory_mb: u64,
    pub total_cpu_percent: f32,
    pub pipeline_uptime: Duration,
}

#[derive(Debug, Clone)]
pub struct HealthAlert {
    pub timestamp: Instant,
    pub severity: AlertSeverity,
    pub stream: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub check_interval: Duration,
    pub deadlock_timeout: Duration,
    pub memory_threshold_mb: u64,
    pub cpu_threshold_percent: f32,
    pub fps_threshold: f64,
    pub error_threshold: u64,
    pub event_log_size: usize,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(1),
            deadlock_timeout: Duration::from_secs(10),
            memory_threshold_mb: 1024, // 1GB
            cpu_threshold_percent: 80.0,
            fps_threshold: 10.0,
            error_threshold: 100,
            event_log_size: 1000,
        }
    }
}

pub struct HealthMonitor {
    config: MonitorConfig,
    streams: Arc<DashMap<String, Arc<Mutex<StreamHealth>>>>,
    event_log: Arc<Mutex<VecDeque<HealthAlert>>>,
    start_time: Instant,
    last_check: Arc<Mutex<Instant>>,
    running: Arc<Mutex<bool>>,
}

impl HealthMonitor {
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            config,
            streams: Arc::new(DashMap::new()),
            event_log: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            start_time: Instant::now(),
            last_check: Arc::new(Mutex::new(Instant::now())),
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn register_stream(&self, name: String, health: Arc<Mutex<StreamHealth>>) {
        self.streams.insert(name.clone(), health);
        info!("Registered stream {name} for health monitoring");
        self.log_event(HealthAlert {
            timestamp: Instant::now(),
            severity: AlertSeverity::Info,
            stream: Some(name),
            message: "Stream registered for monitoring".to_string(),
        });
    }

    pub fn unregister_stream(&self, name: &str) {
        if self.streams.remove(name).is_some() {
            info!("Unregistered stream {name} from health monitoring");
            self.log_event(HealthAlert {
                timestamp: Instant::now(),
                severity: AlertSeverity::Info,
                stream: Some(name.to_string()),
                message: "Stream unregistered from monitoring".to_string(),
            });
        }
    }

    pub fn start_monitoring(&self) {
        *self.running.lock().unwrap() = true;

        let running = Arc::clone(&self.running);
        let streams = Arc::clone(&self.streams);
        let event_log = Arc::clone(&self.event_log);
        let last_check = Arc::clone(&self.last_check);
        let config = self.config.clone();

        gstreamer::glib::timeout_add(self.config.check_interval, move || {
            if !*running.lock().unwrap() {
                return gstreamer::glib::ControlFlow::Break;
            }

            let now = Instant::now();
            let last = *last_check.lock().unwrap();

            // Check each stream
            for entry in streams.iter() {
                let health = entry.value().lock().unwrap();

                // Check for deadlock
                if let Some(last_frame) = health.metrics.last_frame_time {
                    if now.duration_since(last_frame) > config.deadlock_timeout {
                        warn!("Possible deadlock detected in stream {}", entry.key());
                        let alert = HealthAlert {
                            timestamp: now,
                            severity: AlertSeverity::Critical,
                            stream: Some(entry.key().clone()),
                            message: format!(
                                "No activity for {:?}",
                                now.duration_since(last_frame)
                            ),
                        };
                        Self::log_event_static(Arc::clone(&event_log), alert);
                    }
                }

                // Check FPS
                if health.state == StreamState::Running && health.metrics.fps < config.fps_threshold
                {
                    debug!(
                        "Low FPS detected in stream {}: {:.2}",
                        entry.key(),
                        health.metrics.fps
                    );
                    let alert = HealthAlert {
                        timestamp: now,
                        severity: AlertSeverity::Warning,
                        stream: Some(entry.key().clone()),
                        message: format!("Low FPS: {:.2}", health.metrics.fps),
                    };
                    Self::log_event_static(Arc::clone(&event_log), alert);
                }

                // Check error rate
                if health.metrics.errors > config.error_threshold {
                    warn!(
                        "High error count in stream {}: {}",
                        entry.key(),
                        health.metrics.errors
                    );
                    let alert = HealthAlert {
                        timestamp: now,
                        severity: AlertSeverity::Error,
                        stream: Some(entry.key().clone()),
                        message: format!("High error count: {}", health.metrics.errors),
                    };
                    Self::log_event_static(Arc::clone(&event_log), alert);
                }

                // Update metrics
                counter!("stream_health_checks", "stream" => entry.key().clone()).increment(1);
                gauge!("stream_fps", "stream" => entry.key().clone()).set(health.metrics.fps);
                gauge!("stream_errors", "stream" => entry.key().clone())
                    .set(health.metrics.errors as f64);
            }

            *last_check.lock().unwrap() = now;
            gstreamer::glib::ControlFlow::Continue
        });

        info!("Health monitoring started");
    }

    pub fn stop_monitoring(&self) {
        *self.running.lock().unwrap() = false;
        info!("Health monitoring stopped");
    }

    pub fn generate_report(&self) -> HealthReport {
        let mut stream_health = HashMap::new();
        let mut active_streams = 0;
        let mut failed_streams = 0;
        let mut total_memory = 0u64;
        let mut total_cpu = 0.0f32;

        for entry in self.streams.iter() {
            let health = entry.value().lock().unwrap();

            let metrics = StreamHealthMetrics {
                name: entry.key().clone(),
                state: health.state,
                fps: health.metrics.fps,
                bitrate: health.metrics.bitrate,
                frames_processed: health.metrics.frames_processed,
                frames_dropped: health.metrics.frames_dropped,
                errors: health.metrics.errors,
                uptime: health.metrics.uptime,
                last_activity: health.metrics.last_frame_time.unwrap_or(Instant::now()),
                memory_usage: 0, // Would calculate actual memory usage
                cpu_usage: 0.0,  // Would calculate actual CPU usage
            };

            match health.state {
                StreamState::Running | StreamState::Paused => active_streams += 1,
                StreamState::Failed => failed_streams += 1,
                _ => {}
            }

            stream_health.insert(entry.key().clone(), metrics);
        }

        let system_metrics = SystemMetrics {
            total_streams: self.streams.len(),
            active_streams,
            failed_streams,
            total_memory_mb: total_memory / 1_048_576,
            total_cpu_percent: total_cpu,
            pipeline_uptime: self.start_time.elapsed(),
        };

        let overall_health = if failed_streams > 0 || total_cpu > self.config.cpu_threshold_percent
        {
            HealthStatus::Critical
        } else if active_streams < self.streams.len() {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let alerts = self.event_log.lock().unwrap().iter().cloned().collect();

        HealthReport {
            timestamp: SystemTime::now(),
            overall_health,
            stream_health,
            system_metrics,
            alerts,
        }
    }

    pub fn get_stream_health(&self, name: &str) -> Option<StreamHealth> {
        self.streams
            .get(name)
            .map(|entry| entry.lock().unwrap().clone())
    }

    pub fn check_memory_usage(&self) -> DslResult<u64> {
        // Platform-specific memory check would go here
        // For now, return a placeholder
        Ok(100 * 1_048_576) // 100MB
    }

    pub fn detect_deadlock(&self, stream_name: &str) -> bool {
        if let Some(entry) = self.streams.get(stream_name) {
            let health = entry.lock().unwrap();
            if let Some(last_frame) = health.metrics.last_frame_time {
                return Instant::now().duration_since(last_frame) > self.config.deadlock_timeout;
            }
        }
        false
    }

    fn log_event(&self, alert: HealthAlert) {
        Self::log_event_static(Arc::clone(&self.event_log), alert);
    }

    fn log_event_static(event_log: Arc<Mutex<VecDeque<HealthAlert>>>, alert: HealthAlert) {
        let mut log = event_log.lock().unwrap();

        // Maintain ring buffer size
        while log.len() >= 1000 {
            log.pop_front();
        }

        match alert.severity {
            AlertSeverity::Info => info!(
                "{}: {}",
                alert.stream.as_deref().unwrap_or("system"),
                alert.message
            ),
            AlertSeverity::Warning => warn!(
                "{}: {}",
                alert.stream.as_deref().unwrap_or("system"),
                alert.message
            ),
            AlertSeverity::Error => error!(
                "{}: {}",
                alert.stream.as_deref().unwrap_or("system"),
                alert.message
            ),
            AlertSeverity::Critical => error!(
                "CRITICAL - {}: {}",
                alert.stream.as_deref().unwrap_or("system"),
                alert.message
            ),
        }

        log.push_back(alert);
    }

    pub fn get_recent_alerts(&self, count: usize) -> Vec<HealthAlert> {
        let log = self.event_log.lock().unwrap();
        log.iter().rev().take(count).cloned().collect()
    }

    pub fn clear_alerts(&self) {
        self.event_log.lock().unwrap().clear();
        info!("Health monitor alerts cleared");
    }
}

impl Clone for StreamHealth {
    fn clone(&self) -> Self {
        Self {
            state: self.state,
            metrics: self.metrics.clone(),
            last_error: self.last_error.clone(),
            consecutive_errors: self.consecutive_errors,
            recovery_attempts: self.recovery_attempts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_monitor_creation() {
        let config = MonitorConfig::default();
        let monitor = HealthMonitor::new(config);

        assert_eq!(monitor.streams.len(), 0);
    }

    #[test]
    fn test_stream_registration() {
        let monitor = HealthMonitor::new(MonitorConfig::default());
        let health = Arc::new(Mutex::new(StreamHealth::new()));

        monitor.register_stream("test_stream".to_string(), health);
        assert_eq!(monitor.streams.len(), 1);
        assert!(monitor.get_stream_health("test_stream").is_some());

        monitor.unregister_stream("test_stream");
        assert_eq!(monitor.streams.len(), 0);
    }

    #[test]
    fn test_health_report_generation() {
        let monitor = HealthMonitor::new(MonitorConfig::default());

        // Register some test streams
        for i in 0..3 {
            let mut health = StreamHealth::new();
            health.state = StreamState::Running;
            monitor.register_stream(format!("stream_{i}"), Arc::new(Mutex::new(health)));
        }

        let report = monitor.generate_report();
        assert_eq!(report.system_metrics.total_streams, 3);
        assert_eq!(report.system_metrics.active_streams, 3);
        assert_eq!(report.overall_health, HealthStatus::Healthy);
    }

    #[test]
    fn test_alert_logging() {
        let monitor = HealthMonitor::new(MonitorConfig::default());

        for i in 0..5 {
            monitor.log_event(HealthAlert {
                timestamp: Instant::now(),
                severity: AlertSeverity::Info,
                stream: Some(format!("stream_{i}")),
                message: "Test alert".to_string(),
            });
        }

        let alerts = monitor.get_recent_alerts(3);
        assert_eq!(alerts.len(), 3);

        monitor.clear_alerts();
        let alerts = monitor.get_recent_alerts(10);
        assert_eq!(alerts.len(), 0);
    }
}
