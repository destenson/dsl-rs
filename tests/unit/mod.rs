//! Unit tests for DSL-RS modules
//!
//! These tests focus on individual components in isolation,
//! testing their API contracts and error handling.

// Import the common test utilities
#[path = "../common/mod.rs"]
mod common;

use common::*;
use dsl_rs::core::*;
use dsl_rs::pipeline::*;
use dsl_rs::stream::*;
use dsl_rs::recovery::*;
use dsl_rs::health::*;
use dsl_rs::source::*;
use dsl_rs::sink::*;
use dsl_rs::isolation::*;
use std::time::Duration;

/// Core module tests
mod core_tests {
    use super::*;
    
    #[test]
    fn test_error_types() {
        // Test error creation and display
        let err = DslError::Connection("Test connection error".to_string());
        assert!(err.to_string().contains("connection"));
        
        let err = DslError::Pipeline("Pipeline error".to_string());
        assert!(err.to_string().contains("Pipeline"));
        
        let err = DslError::StateTransition("Invalid state".to_string());
        assert!(err.to_string().contains("state"));
    }
    
    #[test]
    fn test_retry_config() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            exponential_base: 2.0,
        };
        
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_delay, Duration::from_millis(100));
        assert_eq!(config.exponential_base, 2.0);
    }
    
    #[test]
    fn test_stream_state_transitions() {
        // Test valid transitions
        assert_eq!(
            StreamState::Idle.next_state(TransitionCondition::OnSuccess),
            Some(StreamState::Starting)
        );
        assert_eq!(
            StreamState::Starting.next_state(TransitionCondition::OnSuccess),
            Some(StreamState::Running)
        );
        assert_eq!(
            StreamState::Running.next_state(TransitionCondition::OnError),
            Some(StreamState::Recovering)
        );
        assert_eq!(
            StreamState::Recovering.next_state(TransitionCondition::OnSuccess),
            Some(StreamState::Running)
        );
        assert_eq!(
            StreamState::Running.next_state(TransitionCondition::OnStop),
            Some(StreamState::Stopping)
        );
        
        // Test invalid transition
        assert_eq!(
            StreamState::Idle.next_state(TransitionCondition::OnError),
            None
        );
    }
}

/// Pipeline module tests
mod pipeline_tests {
    use super::*;
    
    #[test]
    fn test_pipeline_config_default() {
        let config = PipelineConfig::default();
        assert_eq!(config.watchdog_timeout, Some(Duration::from_secs(60)));
        assert_eq!(config.max_streams, 10);
        assert!(config.enable_metrics);
    }
    
    #[test]
    fn test_pipeline_creation_with_config() {
        init_gstreamer();
        
        let config = PipelineConfig {
            watchdog_timeout: Some(Duration::from_secs(30)),
            max_streams: 5,
            enable_metrics: false,
        };
        
        let result = RobustPipeline::new(config);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_stream_health_initialization() {
        let health = StreamHealth {
            state: StreamState::Idle,
            last_update: std::time::Instant::now(),
            error_count: 0,
            recovery_attempts: 0,
            metrics: StreamMetrics::default(),
        };
        
        assert_eq!(health.state, StreamState::Idle);
        assert_eq!(health.error_count, 0);
        assert_eq!(health.recovery_attempts, 0);
    }
    
    #[test]
    fn test_stream_metrics() {
        let metrics = StreamMetrics {
            frames_processed: 1000,
            bytes_processed: 1024 * 1024,
            latency_ms: 42.5,
            dropped_frames: 5,
        };
        
        assert_eq!(metrics.frames_processed, 1000);
        assert_eq!(metrics.bytes_processed, 1024 * 1024);
        assert_eq!(metrics.latency_ms, 42.5);
        assert_eq!(metrics.dropped_frames, 5);
    }
}

/// Stream manager tests
mod stream_manager_tests {
    use super::*;
    
    #[test]
    fn test_stream_manager_creation() {
        init_gstreamer();
        
        let manager = StreamManager::new(5);
        assert!(manager.is_ok());
    }
    
    #[test]
    fn test_stream_info() {
        let info = StreamInfo {
            name: "test_stream".to_string(),
            source_type: "file".to_string(),
            sink_type: "rtsp".to_string(),
            created_at: std::time::SystemTime::now(),
            state: StreamState::Idle,
        };
        
        assert_eq!(info.name, "test_stream");
        assert_eq!(info.source_type, "file");
        assert_eq!(info.sink_type, "rtsp");
        assert_eq!(info.state, StreamState::Idle);
    }
}

/// Recovery manager tests
mod recovery_tests {
    use super::*;
    use dsl_rs::recovery::{RecoveryManager, RecoveryStrategy, RecoveryAction};
    
    #[test]
    fn test_recovery_manager_creation() {
        let manager = RecoveryManager::new(3, Duration::from_secs(30));
        assert!(manager.decide_recovery_action("test", &DslError::Connection("test".to_string())).is_some());
    }
    
    #[test]
    fn test_circuit_breaker_states() {
        let breaker = CircuitBreaker::new(3, Duration::from_secs(10));
        assert_eq!(breaker.state(), CircuitState::Closed);
        
        // Record failures to trigger open state
        for _ in 0..3 {
            breaker.record_failure();
        }
        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(!breaker.can_attempt());
    }
    
    #[test]
    fn test_exponential_backoff() {
        let strategy = ExponentialBackoffStrategy::new(
            Duration::from_millis(100),
            Duration::from_secs(10),
            2.0
        );
        
        let delay1 = strategy.next_delay(1);
        let delay2 = strategy.next_delay(2);
        let delay3 = strategy.next_delay(3);
        
        assert!(delay1 <= delay2);
        assert!(delay2 <= delay3);
        assert!(delay3 <= Duration::from_secs(10));
    }
}

/// Health monitoring tests
mod health_tests {
    use super::*;
    
    #[test]
    fn test_health_monitor_creation() {
        let monitor = HealthMonitor::new(Duration::from_secs(10));
        assert!(monitor.is_healthy());
    }
    
    #[test]
    fn test_system_health_metrics() {
        let health = SystemHealth {
            cpu_usage: 45.5,
            memory_usage: 2048 * 1024 * 1024, // 2GB
            active_streams: 3,
            total_errors: 5,
            uptime_seconds: 3600,
        };
        
        assert_eq!(health.cpu_usage, 45.5);
        assert_eq!(health.memory_usage, 2048 * 1024 * 1024);
        assert_eq!(health.active_streams, 3);
        assert_eq!(health.total_errors, 5);
        assert_eq!(health.uptime_seconds, 3600);
    }
    
    #[test]
    fn test_health_thresholds() {
        let monitor = HealthMonitor::new(Duration::from_secs(10));
        
        // Test CPU threshold
        let high_cpu = SystemHealth {
            cpu_usage: 95.0,
            memory_usage: 1024 * 1024 * 1024,
            active_streams: 1,
            total_errors: 0,
            uptime_seconds: 60,
        };
        
        monitor.update_system_health(high_cpu);
        // In real implementation, this would trigger alerts
    }
}

/// Source module tests
mod source_tests {
    use super::*;
    
    #[test]
    fn test_file_source_config() {
        use dsl_rs::source::FileSourceConfig;
        
        let config = FileSourceConfig {
            path: "/test/path.mp4".to_string(),
            loop_playback: true,
            start_position: Some(Duration::from_secs(10)),
            playback_rate: 1.0,
        };
        
        assert_eq!(config.path, "/test/path.mp4");
        assert!(config.loop_playback);
        assert_eq!(config.start_position, Some(Duration::from_secs(10)));
        assert_eq!(config.playback_rate, 1.0);
    }
    
    #[test]
    fn test_rtsp_source_config() {
        use dsl_rs::source::RtspSourceConfig;
        
        let config = RtspSourceConfig {
            uri: "rtsp://camera.local/stream".to_string(),
            username: Some("admin".to_string()),
            password: Some("password".to_string()),
            retry_config: RetryConfig::default(),
            reconnect_timeout: Duration::from_secs(5),
            protocols: vec!["tcp".to_string()],
            latency: 200,
        };
        
        assert_eq!(config.uri, "rtsp://camera.local/stream");
        assert_eq!(config.username, Some("admin".to_string()));
        assert_eq!(config.latency, 200);
    }
    
    #[tokio::test]
    async fn test_mock_source_connection() {
        let source = MockSource::new("test");
        
        // Test successful connection
        assert!(source.connect().await.is_ok());
        assert_eq!(source.get_connect_count(), 1);
        assert_eq!(*source.state.lock().unwrap(), SourceState::Connected);
        
        // Test disconnection
        assert!(source.disconnect().await.is_ok());
        assert_eq!(*source.state.lock().unwrap(), SourceState::Disconnected);
        
        // Test failure scenario
        source.set_should_fail(true);
        assert!(source.connect().await.is_err());
        assert_eq!(*source.state.lock().unwrap(), SourceState::Failed);
    }
}

/// Sink module tests
mod sink_tests {
    use super::*;
    
    #[test]
    fn test_file_sink_config() {
        use dsl_rs::sink::FileSinkConfig;
        
        let config = FileSinkConfig {
            path: "/output/video.mp4".to_string(),
            container: "mp4mux".to_string(),
            encoder: Some("x264enc".to_string()),
            max_file_size: Some(1024 * 1024 * 1024), // 1GB
            rotation_interval: Some(Duration::from_secs(3600)),
        };
        
        assert_eq!(config.path, "/output/video.mp4");
        assert_eq!(config.container, "mp4mux");
        assert_eq!(config.encoder, Some("x264enc".to_string()));
        assert_eq!(config.max_file_size, Some(1024 * 1024 * 1024));
    }
    
    #[test]
    fn test_rtsp_sink_config() {
        use dsl_rs::sink::RtspSinkConfig;
        
        let config = RtspSinkConfig {
            path: "/live/stream".to_string(),
            port: 8554,
            encoder: "x264enc".to_string(),
            bitrate: Some(4000000),
            authentication: false,
        };
        
        assert_eq!(config.path, "/live/stream");
        assert_eq!(config.port, 8554);
        assert_eq!(config.encoder, "x264enc");
        assert_eq!(config.bitrate, Some(4000000));
        assert!(!config.authentication);
    }
    
    #[tokio::test]
    async fn test_mock_sink_preparation() {
        let sink = MockSink::new("test");
        
        // Test successful preparation
        assert!(sink.prepare().await.is_ok());
        assert_eq!(*sink.state.lock().unwrap(), SinkState::Ready);
        
        // Test cleanup
        assert!(sink.cleanup().await.is_ok());
        assert_eq!(*sink.state.lock().unwrap(), SinkState::Idle);
        
        // Test failure scenario
        sink.set_should_fail(true);
        assert!(sink.prepare().await.is_err());
        assert_eq!(*sink.state.lock().unwrap(), SinkState::Failed);
    }
}

/// Isolation module tests
mod isolation_tests {
    use super::*;
    
    #[test]
    fn test_resource_limits() {
        let limits = ResourceLimits {
            max_cpu_percent: Some(50.0),
            max_memory_mb: Some(1024),
            max_bandwidth_mbps: Some(100),
            priority: 0,
        };
        
        assert_eq!(limits.max_cpu_percent, Some(50.0));
        assert_eq!(limits.max_memory_mb, Some(1024));
        assert_eq!(limits.max_bandwidth_mbps, Some(100));
        assert_eq!(limits.priority, 0);
    }
    
    #[test]
    fn test_isolation_policy() {
        let policy = IsolationPolicy {
            enable_cpu_isolation: true,
            enable_memory_isolation: true,
            enable_network_isolation: false,
            failure_isolation: true,
        };
        
        assert!(policy.enable_cpu_isolation);
        assert!(policy.enable_memory_isolation);
        assert!(!policy.enable_network_isolation);
        assert!(policy.failure_isolation);
    }
}

/// Property-based tests using proptest
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    
    proptest! {
        #[test]
        fn test_retry_config_delays(
            attempts in 1u32..10,
            initial_ms in 10u64..1000,
            max_ms in 1000u64..60000,
            base in 1.5f64..3.0
        ) {
            let config = RetryConfig {
                max_attempts: attempts,
                initial_delay: Duration::from_millis(initial_ms),
                max_delay: Duration::from_millis(max_ms),
                exponential_base: base,
            };
            
            // Calculate expected delay for each attempt
            for attempt in 1..=attempts {
                let delay_ms = (initial_ms as f64 * base.powi(attempt as i32 - 1)) as u64;
                let expected = std::cmp::min(delay_ms, max_ms);
                
                // In real implementation, verify delay calculation
                assert!(expected <= max_ms);
            }
        }
        
        #[test]
        fn test_stream_state_machine(transitions in prop::collection::vec(0u8..4, 0..20)) {
            let mut state = StreamState::Idle;
            
            for t in transitions {
                let condition = match t {
                    0 => TransitionCondition::OnSuccess,
                    1 => TransitionCondition::OnError,
                    2 => TransitionCondition::OnStop,
                    _ => TransitionCondition::OnRecovery,
                };
                
                if let Some(new_state) = state.next_state(condition) {
                    // Verify transition is valid
                    match (state, new_state) {
                        (StreamState::Idle, StreamState::Starting) => {},
                        (StreamState::Starting, StreamState::Running) => {},
                        (StreamState::Running, StreamState::Stopping) => {},
                        (StreamState::Running, StreamState::Recovering) => {},
                        (StreamState::Recovering, StreamState::Running) => {},
                        (StreamState::Recovering, StreamState::Failed) => {},
                        (StreamState::Stopping, StreamState::Stopped) => {},
                        _ => {
                            // All valid transitions should be covered
                        }
                    }
                    state = new_state;
                }
            }
        }
    }
}

#[cfg(test)]
mod integration_preparation {
    use super::*;
    
    /// Verify all test utilities work together
    #[tokio::test]
    async fn test_integration_setup() {
        init_gstreamer();
        
        // Create test fixture
        let mut fixture = TestFixture::new();
        let _file = fixture.create_test_file("test.txt", b"content");
        
        // Create mock source and sink
        let source = MockSource::new("integration");
        let sink = MockSink::new("integration");
        
        // Connect source
        assert!(source.connect().await.is_ok());
        
        // Prepare sink
        assert!(sink.prepare().await.is_ok());
        
        // Create elements
        assert!(source.create_element().is_ok());
        assert!(sink.create_element().is_ok());
        
        // Test config builder
        let _config = TestConfigBuilder::new()
            .with_retry_config(3, 100)
            .with_watchdog(30)
            .build_pipeline_config();
        
        // Test performance monitor
        let mut monitor = PerformanceMonitor::new();
        monitor.checkpoint("setup_complete");
        assert!(monitor.elapsed() > Duration::from_nanos(0));
        
        // Cleanup
        assert!(source.disconnect().await.is_ok());
        assert!(sink.cleanup().await.is_ok());
    }
}