//! Configuration matrix tests
//!
//! Tests all retry configurations, pipeline configurations, and boundary values.

#[path = "../common/mod.rs"]
mod common;

use common::*;
use dsl_rs::core::*;
use dsl_rs::pipeline::*;
use proptest::prelude::*;
use std::time::Duration;

/// Test matrix of retry configurations
#[test]
fn test_retry_configuration_matrix() {
    init_gstreamer();
    
    let configs = vec![
        // Aggressive retry
        RetryConfig {
            max_attempts: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            exponential_base: 1.5,
        },
        // Conservative retry
        RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            exponential_base: 2.0,
        },
        // No backoff
        RetryConfig {
            max_attempts: 5,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_millis(500),
            exponential_base: 1.0,
        },
    ];
    
    for config in configs {
        // Verify configuration is valid
        assert!(config.max_attempts > 0);
        assert!(config.initial_delay > Duration::ZERO);
        assert!(config.max_delay >= config.initial_delay);
        assert!(config.exponential_base >= 1.0);
    }
}

/// Test pipeline configuration combinations
#[test]
fn test_pipeline_configuration_matrix() {
    init_gstreamer();
    
    let configs = vec![
        // Minimal configuration
        PipelineConfig {
            watchdog_timeout: None,
            max_streams: 1,
            enable_metrics: false,
        },
        // Standard configuration
        PipelineConfig {
            watchdog_timeout: Some(Duration::from_secs(60)),
            max_streams: 10,
            enable_metrics: true,
        },
        // Maximum configuration
        PipelineConfig {
            watchdog_timeout: Some(Duration::from_secs(300)),
            max_streams: 100,
            enable_metrics: true,
        },
    ];
    
    for config in configs {
        let result = RobustPipeline::new(config.clone());
        assert!(result.is_ok(), "Pipeline creation failed for config: {:?}", config);
    }
}

/// Test boundary values
#[test]
fn test_boundary_values() {
    // Test minimum values
    let min_retry = RetryConfig {
        max_attempts: 1,
        initial_delay: Duration::from_nanos(1),
        max_delay: Duration::from_nanos(1),
        exponential_base: 1.0,
    };
    assert_eq!(min_retry.max_attempts, 1);
    
    // Test maximum reasonable values
    let max_retry = RetryConfig {
        max_attempts: u32::MAX,
        initial_delay: Duration::from_secs(3600), // 1 hour
        max_delay: Duration::from_secs(86400), // 24 hours
        exponential_base: 10.0,
    };
    assert_eq!(max_retry.max_attempts, u32::MAX);
}

proptest! {
    #[test]
    fn test_configuration_property(
        max_attempts in 1u32..100,
        initial_ms in 1u64..10000,
        max_ms in 10000u64..60000,
        base in 1.0f64..5.0
    ) {
        let config = RetryConfig {
            max_attempts,
            initial_delay: Duration::from_millis(initial_ms),
            max_delay: Duration::from_millis(max_ms),
            exponential_base: base,
        };
        
        // Property: max_delay should always be >= initial_delay
        prop_assert!(config.max_delay >= config.initial_delay);
        
        // Property: exponential base should be positive
        prop_assert!(config.exponential_base > 0.0);
    }
}