use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tracing::{debug, error, info, warn};

use crate::core::{DslError, DslResult, RecoveryAction, RecoveryStrategy, RetryConfig};

#[derive(Clone)]
pub enum RecoveryPolicy {
    Immediate,   // Retry immediately
    FixedDelay,  // Fixed delay between retries
    Exponential, // Exponential backoff
    Custom(Box<dyn RecoveryStrategy>),
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,  // Number of failures to trip
    pub success_threshold: u32,  // Number of successes to reset
    pub timeout: Duration,       // Time before attempting reset
    pub half_open_attempts: u32, // Max attempts in half-open state
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(30),
            half_open_attempts: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum CircuitState {
    Closed,   // Normal operation
    Open,     // Blocking requests
    HalfOpen, // Testing recovery
}

struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            config,
        }
    }

    fn on_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.config.success_threshold {
                    info!("Circuit breaker transitioning to CLOSED");
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                }
            }
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            _ => {}
        }
    }

    fn on_failure(&mut self) {
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.config.failure_threshold {
                    warn!("Circuit breaker tripped - transitioning to OPEN");
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                warn!("Failure in half-open state - returning to OPEN");
                self.state = CircuitState::Open;
                self.failure_count = 0;
                self.success_count = 0;
            }
            _ => {}
        }
    }

    fn should_allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last_failure) = self.last_failure_time {
                    if Instant::now().duration_since(last_failure) > self.config.timeout {
                        info!("Circuit breaker timeout expired - transitioning to HALF-OPEN");
                        self.state = CircuitState::HalfOpen;
                        self.success_count = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => self.success_count < self.config.half_open_attempts,
        }
    }
}

#[derive(Debug, Clone)]
struct FailurePattern {
    timestamp: Instant,
    error_type: String,
    stream_name: String,
}

pub struct RecoveryManager {
    policies: Arc<DashMap<String, RecoveryPolicy>>,
    circuit_breakers: Arc<DashMap<String, Arc<Mutex<CircuitBreaker>>>>,
    retry_configs: Arc<DashMap<String, RetryConfig>>,
    failure_history: Arc<Mutex<VecDeque<FailurePattern>>>,
    telemetry: Arc<RecoveryTelemetry>,
}

struct RecoveryTelemetry {
    total_recoveries: Arc<Mutex<u64>>,
    failed_recoveries: Arc<Mutex<u64>>,
    circuit_trips: Arc<Mutex<u64>>,
    recovery_times: Arc<Mutex<Vec<Duration>>>,
}

impl RecoveryTelemetry {
    fn new() -> Self {
        Self {
            total_recoveries: Arc::new(Mutex::new(0)),
            failed_recoveries: Arc::new(Mutex::new(0)),
            circuit_trips: Arc::new(Mutex::new(0)),
            recovery_times: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn record_recovery(&self, duration: Duration, success: bool) {
        if success {
            *self.total_recoveries.lock().unwrap() += 1;
        } else {
            *self.failed_recoveries.lock().unwrap() += 1;
        }
        self.recovery_times.lock().unwrap().push(duration);
    }

    fn record_circuit_trip(&self) {
        *self.circuit_trips.lock().unwrap() += 1;
    }

    fn get_stats(&self) -> RecoveryStats {
        let times = self.recovery_times.lock().unwrap();
        let avg_recovery_time = if !times.is_empty() {
            let sum: Duration = times.iter().sum();
            Some(sum / times.len() as u32)
        } else {
            None
        };

        RecoveryStats {
            total_recoveries: *self.total_recoveries.lock().unwrap(),
            failed_recoveries: *self.failed_recoveries.lock().unwrap(),
            circuit_trips: *self.circuit_trips.lock().unwrap(),
            avg_recovery_time,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecoveryStats {
    pub total_recoveries: u64,
    pub failed_recoveries: u64,
    pub circuit_trips: u64,
    pub avg_recovery_time: Option<Duration>,
}

impl RecoveryManager {
    pub fn new() -> Self {
        Self {
            policies: Arc::new(DashMap::new()),
            circuit_breakers: Arc::new(DashMap::new()),
            retry_configs: Arc::new(DashMap::new()),
            failure_history: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            telemetry: Arc::new(RecoveryTelemetry::new()),
        }
    }

    pub fn set_policy(&self, stream_name: String, policy: RecoveryPolicy) {
        self.policies.insert(stream_name.clone(), policy);
        info!("Set recovery policy for stream: {}", stream_name);
    }

    pub fn set_retry_config(&self, stream_name: String, config: RetryConfig) {
        self.retry_configs.insert(stream_name, config);
    }

    pub fn enable_circuit_breaker(&self, stream_name: String, config: CircuitBreakerConfig) {
        let breaker = Arc::new(Mutex::new(CircuitBreaker::new(config)));
        self.circuit_breakers.insert(stream_name.clone(), breaker);
        info!("Enabled circuit breaker for stream: {}", stream_name);
    }

    pub fn should_attempt_recovery(&self, stream_name: &str) -> bool {
        if let Some(breaker) = self.circuit_breakers.get(stream_name) {
            let mut breaker = breaker.lock().unwrap();
            let allowed = breaker.should_allow_request();
            if !allowed {
                debug!("Circuit breaker preventing recovery for: {}", stream_name);
            }
            allowed
        } else {
            true
        }
    }

    pub async fn execute_recovery(
        &self,
        stream_name: &str,
        error: &DslError,
        attempt: u32,
    ) -> DslResult<RecoveryAction> {
        let start_time = Instant::now();

        // Check circuit breaker
        if !self.should_attempt_recovery(stream_name) {
            return Ok(RecoveryAction::Escalate);
        }

        // Record failure pattern
        self.record_failure(stream_name, error);

        // Get recovery policy
        let policy = self
            .policies
            .get(stream_name)
            .map(|p| p.clone())
            .unwrap_or(RecoveryPolicy::Exponential);

        // Determine action based on policy
        let action = match policy {
            RecoveryPolicy::Immediate => {
                debug!("Immediate recovery for {}", stream_name);
                RecoveryAction::Retry
            }
            RecoveryPolicy::FixedDelay => {
                let delay = Duration::from_millis(500);
                debug!("Fixed delay recovery for {} ({:?})", stream_name, delay);
                std::thread::sleep(delay);
                RecoveryAction::Retry
            }
            RecoveryPolicy::Exponential => {
                let config = self
                    .retry_configs
                    .get(stream_name)
                    .map(|c| c.clone())
                    .unwrap_or_default();

                let delay = self.calculate_exponential_delay(&config, attempt);
                debug!(
                    "Exponential backoff recovery for {} ({:?})",
                    stream_name, delay
                );
                std::thread::sleep(delay);

                if attempt >= config.max_attempts {
                    RecoveryAction::Escalate
                } else {
                    RecoveryAction::Retry
                }
            }
            RecoveryPolicy::Custom(ref strategy) => {
                let delay = strategy.calculate_delay(attempt);
                std::thread::sleep(delay);
                strategy.decide_action(error, attempt)
            }
        };

        // Update telemetry
        let duration = start_time.elapsed();
        let success = !matches!(action, RecoveryAction::Escalate | RecoveryAction::Remove);
        self.telemetry.record_recovery(duration, success);

        // Update circuit breaker
        if let Some(breaker) = self.circuit_breakers.get(stream_name) {
            let mut breaker = breaker.lock().unwrap();
            if success {
                breaker.on_success();
            } else {
                breaker.on_failure();
                if breaker.state == CircuitState::Open {
                    self.telemetry.record_circuit_trip();
                }
            }
        }

        Ok(action)
    }

    fn calculate_exponential_delay(&self, config: &RetryConfig, attempt: u32) -> Duration {
        let base = config.initial_delay.as_millis() as f64;
        let exponential = base * config.exponential_base.powi(attempt as i32);
        let clamped = exponential.min(config.max_delay.as_millis() as f64);

        let final_delay = if config.jitter {
            // Add random jitter (+/- 20%)
            let jitter = clamped * 0.2 * (2.0 * rand() - 1.0);
            (clamped + jitter).max(0.0)
        } else {
            clamped
        };

        Duration::from_millis(final_delay as u64)
    }

    fn record_failure(&self, stream_name: &str, error: &DslError) {
        let pattern = FailurePattern {
            timestamp: Instant::now(),
            error_type: format!("{:?}", error),
            stream_name: stream_name.to_string(),
        };

        let mut history = self.failure_history.lock().unwrap();
        history.push_back(pattern);

        // Keep only last 1000 failures
        while history.len() > 1000 {
            history.pop_front();
        }
    }

    pub fn get_failure_patterns(&self, stream_name: &str) -> Vec<String> {
        let history = self.failure_history.lock().unwrap();
        history
            .iter()
            .filter(|p| p.stream_name == stream_name)
            .map(|p| p.error_type.clone())
            .collect()
    }

    pub fn get_recent_failures(&self, duration: Duration) -> Vec<FailurePattern> {
        let cutoff = Instant::now() - duration;
        let history = self.failure_history.lock().unwrap();
        history
            .iter()
            .filter(|p| p.timestamp > cutoff)
            .cloned()
            .collect()
    }

    pub fn get_telemetry(&self) -> RecoveryStats {
        self.telemetry.get_stats()
    }

    pub fn reset_stream_state(&self, stream_name: &str) {
        if let Some(breaker) = self.circuit_breakers.get(stream_name) {
            let mut breaker = breaker.lock().unwrap();
            breaker.state = CircuitState::Closed;
            breaker.failure_count = 0;
            breaker.success_count = 0;
            info!("Reset circuit breaker for stream: {}", stream_name);
        }
    }

    pub fn get_circuit_state(&self, stream_name: &str) -> Option<CircuitState> {
        self.circuit_breakers
            .get(stream_name)
            .map(|b| b.lock().unwrap().state.clone())
    }
}

// Simple random function for jitter
fn rand() -> f64 {
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let seed = time.as_nanos() as f64;
    ((seed * 1103515245.0 + 12345.0) / 65536.0) % 1.0
}

// Default recovery strategy implementation
pub struct DefaultRecoveryStrategy {
    max_attempts: u32,
    base_delay: Duration,
}

impl DefaultRecoveryStrategy {
    pub fn new(max_attempts: u32, base_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
        }
    }
}

impl RecoveryStrategy for DefaultRecoveryStrategy {
    fn decide_action(&self, _error: &DslError, attempt: u32) -> RecoveryAction {
        if attempt < self.max_attempts {
            RecoveryAction::Retry
        } else {
            RecoveryAction::Escalate
        }
    }

    fn calculate_delay(&self, attempt: u32) -> Duration {
        self.base_delay * attempt
    }

    fn should_circuit_break(&self, recent_failures: u32) -> bool {
        recent_failures >= 5
    }
}

impl Clone for Box<dyn RecoveryStrategy> {
    fn clone(&self) -> Self {
        // This is a simplified clone for the trait object
        // In production, would use a proper cloneable trait
        Box::new(DefaultRecoveryStrategy::new(10, Duration::from_millis(100)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_state_transitions() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
            half_open_attempts: 3,
        };

        let mut breaker = CircuitBreaker::new(config);
        assert_eq!(breaker.state, CircuitState::Closed);

        // Trip the breaker
        breaker.on_failure();
        assert_eq!(breaker.state, CircuitState::Closed);
        breaker.on_failure();
        assert_eq!(breaker.state, CircuitState::Open);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(150));
        assert!(breaker.should_allow_request());
        assert_eq!(breaker.state, CircuitState::HalfOpen);

        // Success in half-open
        breaker.on_success();
        breaker.on_success();
        assert_eq!(breaker.state, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_recovery_manager_policies() {
        let manager = RecoveryManager::new();

        // Set immediate policy
        manager.set_policy("stream1".to_string(), RecoveryPolicy::Immediate);

        // Execute recovery
        let error = DslError::Network("test error".to_string());
        let action = manager
            .execute_recovery("stream1", &error, 0)
            .await
            .unwrap();
        assert_eq!(action, RecoveryAction::Retry);
    }

    #[test]
    fn test_exponential_delay_calculation() {
        let manager = RecoveryManager::new();
        let config = RetryConfig {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            exponential_base: 2.0,
            jitter: false,
            max_attempts: 5,
        };

        let delay0 = manager.calculate_exponential_delay(&config, 0);
        let delay1 = manager.calculate_exponential_delay(&config, 1);
        let delay2 = manager.calculate_exponential_delay(&config, 2);

        assert_eq!(delay0, Duration::from_millis(100));
        assert_eq!(delay1, Duration::from_millis(200));
        assert_eq!(delay2, Duration::from_millis(400));
    }

    #[test]
    fn test_failure_history() {
        let manager = RecoveryManager::new();

        // Record some failures
        manager.record_failure("stream1", &DslError::Network("error1".to_string()));
        manager.record_failure("stream2", &DslError::Network("error2".to_string()));
        manager.record_failure("stream1", &DslError::Network("error3".to_string()));

        // Get patterns for stream1
        let patterns = manager.get_failure_patterns("stream1");
        assert_eq!(patterns.len(), 2);
    }
}
