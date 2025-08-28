use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;

use dashmap::DashMap;
use gstreamer as gst;
use gstreamer::prelude::*;
use tracing::{debug, error, info, warn};

use crate::core::{
    DslError, DslResult, PipelineConfig, StreamState, StreamHealth, StreamMetrics
};

#[derive(Debug, Clone)]
pub enum PipelineEvent {
    StreamAdded(String),
    StreamRemoved(String),
    StreamStateChanged(String, StreamState),
    StreamError(String, String),
    StreamRecovered(String),
    WatchdogTimeout(String),
    MetricsUpdate(String, StreamMetrics),
}

pub struct RobustPipeline {
    pipeline: gst::Pipeline,
    config: PipelineConfig,
    streams: Arc<DashMap<String, StreamInfo>>,
    watchdog: Option<WatchdogTimer>,
    state_machine: Arc<Mutex<StateMachine>>,
    metrics_collector: Arc<MetricsCollector>,
    event_bus: gst::Bus,
    main_loop: Option<gstreamer::glib::MainLoop>,
}

struct StreamInfo {
    name: String,
    bin: gst::Bin,
    health: Arc<Mutex<StreamHealth>>,
    last_activity: Arc<Mutex<Instant>>,
}

struct WatchdogTimer {
    timeout: Duration,
    streams: Arc<DashMap<String, StreamInfo>>,
    running: Arc<Mutex<bool>>,
}

impl WatchdogTimer {
    fn new(timeout: Duration, streams: Arc<DashMap<String, StreamInfo>>) -> Self {
        Self {
            timeout,
            streams,
            running: Arc::new(Mutex::new(false)),
        }
    }

    fn start(&self) {
        let running = Arc::clone(&self.running);
        let streams = Arc::clone(&self.streams);
        let timeout = self.timeout;

        *running.lock().unwrap() = true;

        gstreamer::glib::timeout_add(Duration::from_secs(1), move || {
            if !*running.lock().unwrap() {
                return gstreamer::glib::ControlFlow::Break;
            }

            let now = Instant::now();
            for entry in streams.iter() {
                let last = *entry.last_activity.lock().unwrap();
                if now.duration_since(last) > timeout {
                    warn!("Stream {} watchdog timeout", entry.name);
                    
                    let mut health = entry.health.lock().unwrap();
                    health.consecutive_errors += 1;
                    if health.state == StreamState::Running {
                        health.state = StreamState::Recovering;
                    }
                }
            }

            gstreamer::glib::ControlFlow::Continue
        });
    }

    fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    fn feed(&self, stream_name: &str) {
        if let Some(info) = self.streams.get(stream_name) {
            *info.last_activity.lock().unwrap() = Instant::now();
        }
    }
}

#[derive(Debug)]
struct StateMachine {
    states: HashMap<String, StreamState>,
    transitions: Vec<StateTransition>,
}

#[derive(Debug, Clone)]
struct StateTransition {
    from: StreamState,
    to: StreamState,
    condition: TransitionCondition,
}

#[derive(Debug, Clone)]
enum TransitionCondition {
    OnSuccess,
    OnError,
    OnTimeout,
    OnRecovery,
}

impl StateMachine {
    fn new() -> Self {
        let transitions = vec![
            StateTransition {
                from: StreamState::Idle,
                to: StreamState::Starting,
                condition: TransitionCondition::OnSuccess,
            },
            StateTransition {
                from: StreamState::Starting,
                to: StreamState::Running,
                condition: TransitionCondition::OnSuccess,
            },
            StateTransition {
                from: StreamState::Starting,
                to: StreamState::Failed,
                condition: TransitionCondition::OnError,
            },
            StateTransition {
                from: StreamState::Running,
                to: StreamState::Recovering,
                condition: TransitionCondition::OnError,
            },
            StateTransition {
                from: StreamState::Recovering,
                to: StreamState::Running,
                condition: TransitionCondition::OnRecovery,
            },
            StateTransition {
                from: StreamState::Recovering,
                to: StreamState::Failed,
                condition: TransitionCondition::OnTimeout,
            },
            StateTransition {
                from: StreamState::Running,
                to: StreamState::Paused,
                condition: TransitionCondition::OnSuccess,
            },
            StateTransition {
                from: StreamState::Paused,
                to: StreamState::Running,
                condition: TransitionCondition::OnSuccess,
            },
        ];

        Self {
            states: HashMap::new(),
            transitions,
        }
    }

    fn transition(&mut self, stream: &str, condition: TransitionCondition) -> Option<StreamState> {
        let current = self.states.get(stream).copied().unwrap_or(StreamState::Idle);
        
        for transition in &self.transitions {
            if transition.from == current && 
               std::mem::discriminant(&transition.condition) == std::mem::discriminant(&condition) {
                self.states.insert(stream.to_string(), transition.to);
                info!("Stream {} transitioned from {:?} to {:?}", stream, current, transition.to);
                return Some(transition.to);
            }
        }
        
        None
    }

    fn get_state(&self, stream: &str) -> StreamState {
        self.states.get(stream).copied().unwrap_or(StreamState::Idle)
    }
}

struct MetricsCollector {
    interval: Duration,
    streams: Arc<DashMap<String, StreamInfo>>,
    running: Arc<Mutex<bool>>,
}

impl MetricsCollector {
    fn new(interval: Duration, streams: Arc<DashMap<String, StreamInfo>>) -> Self {
        Self {
            interval,
            streams,
            running: Arc::new(Mutex::new(false)),
        }
    }

    fn start(&self) {
        let running = Arc::clone(&self.running);
        let streams = Arc::clone(&self.streams);

        *running.lock().unwrap() = true;

        gstreamer::glib::timeout_add(self.interval, move || {
            if !*running.lock().unwrap() {
                return gstreamer::glib::ControlFlow::Break;
            }

            for entry in streams.iter() {
                let health = entry.health.lock().unwrap();
                debug!(
                    "Stream {} metrics - State: {:?}, FPS: {:.2}, Errors: {}",
                    entry.name, health.state, health.metrics.fps, health.metrics.errors
                );
                
                metrics::counter!("stream_frames_processed", 
                    "stream" => entry.name.clone())
                    .increment(health.metrics.frames_processed);
                    
                metrics::gauge!("stream_fps",
                    "stream" => entry.name.clone())
                    .set(health.metrics.fps);
            }

            gstreamer::glib::ControlFlow::Continue
        });
    }

    fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    fn update_metrics(&self, stream_name: &str, metrics: StreamMetrics) {
        if let Some(info) = self.streams.get(stream_name) {
            let mut health = info.health.lock().unwrap();
            health.metrics = metrics;
        }
    }
}

impl RobustPipeline {
    pub fn new(config: PipelineConfig) -> DslResult<Self> {
        let pipeline = gst::Pipeline::builder()
            .name(&config.name)
            .build();

        let bus = pipeline.bus().ok_or_else(|| 
            DslError::Pipeline("Failed to get pipeline bus".to_string()))?;

        let streams = Arc::new(DashMap::new());
        
        let watchdog = if config.enable_watchdog {
            Some(WatchdogTimer::new(config.watchdog_timeout, Arc::clone(&streams)))
        } else {
            None
        };

        let metrics_collector = Arc::new(MetricsCollector::new(
            config.metrics_interval,
            Arc::clone(&streams),
        ));

        Ok(Self {
            pipeline,
            config,
            streams,
            watchdog,
            state_machine: Arc::new(Mutex::new(StateMachine::new())),
            metrics_collector,
            event_bus: bus,
            main_loop: None,
        })
    }

    pub fn add_stream(&self, name: String, bin: gst::Bin) -> DslResult<()> {
        if self.streams.len() >= self.config.max_streams {
            return Err(DslError::ResourceExhaustion(
                format!("Maximum streams ({}) reached", self.config.max_streams)
            ));
        }

        self.pipeline.add(&bin)
            .map_err(|e| DslError::Pipeline(format!("Failed to add stream bin: {}", e)))?;

        let stream_info = StreamInfo {
            name: name.clone(),
            bin,
            health: Arc::new(Mutex::new(StreamHealth::new())),
            last_activity: Arc::new(Mutex::new(Instant::now())),
        };

        self.streams.insert(name.clone(), stream_info);
        
        self.state_machine.lock().unwrap()
            .transition(&name, TransitionCondition::OnSuccess);

        info!("Added stream: {}", name);
        Ok(())
    }

    pub fn remove_stream(&self, name: &str) -> DslResult<()> {
        if let Some((_, info)) = self.streams.remove(name) {
            info.bin.set_state(gst::State::Null)
                .map_err(|_| DslError::Pipeline("Failed to stop stream".to_string()))?;

            self.pipeline.remove(&info.bin)
                .map_err(|e| DslError::Pipeline(format!("Failed to remove stream bin: {}", e)))?;

            info!("Removed stream: {}", name);
            Ok(())
        } else {
            Err(DslError::Stream(format!("Stream {} not found", name)))
        }
    }

    pub fn start(&mut self) -> DslResult<()> {
        self.pipeline.set_state(gst::State::Playing)
            .map_err(|_| DslError::Pipeline("Failed to start pipeline".to_string()))?;

        if let Some(ref watchdog) = self.watchdog {
            watchdog.start();
        }

        if self.config.enable_metrics {
            self.metrics_collector.start();
        }

        self.start_event_handler();

        info!("Pipeline started");
        Ok(())
    }

    pub fn stop(&mut self) -> DslResult<()> {
        if let Some(ref watchdog) = self.watchdog {
            watchdog.stop();
        }

        self.metrics_collector.stop();

        self.pipeline.set_state(gst::State::Null)
            .map_err(|_| DslError::Pipeline("Failed to stop pipeline".to_string()))?;

        if let Some(main_loop) = self.main_loop.take() {
            main_loop.quit();
        }

        info!("Pipeline stopped");
        Ok(())
    }

    pub fn pause(&self) -> DslResult<()> {
        self.pipeline.set_state(gst::State::Paused)
            .map_err(|_| DslError::Pipeline("Failed to pause pipeline".to_string()))?;

        info!("Pipeline paused");
        Ok(())
    }

    pub fn resume(&self) -> DslResult<()> {
        self.pipeline.set_state(gst::State::Playing)
            .map_err(|_| DslError::Pipeline("Failed to resume pipeline".to_string()))?;

        info!("Pipeline resumed");
        Ok(())
    }

    fn start_event_handler(&mut self) {
        let bus = self.event_bus.clone();
        let streams = Arc::clone(&self.streams);
        let state_machine = Arc::clone(&self.state_machine);
        let watchdog = self.watchdog.as_ref().map(|w| w.clone());

        let main_loop = gstreamer::glib::MainLoop::new(None, false);
        self.main_loop = Some(main_loop.clone());

        bus.add_watch(move |_, msg| {
            match msg.view() {
                gst::MessageView::Error(err) => {
                    error!("Pipeline error: {:?}", err);
                    state_machine.lock().unwrap()
                        .transition("pipeline", TransitionCondition::OnError);
                }
                gst::MessageView::Warning(warn) => {
                    warn!("Pipeline warning: {:?}", warn);
                }
                gst::MessageView::Eos(_) => {
                    info!("End of stream");
                }
                gst::MessageView::StateChanged(state) => {
                    if let Some(src) = state.src() {
                        debug!("State changed for {}: {:?} -> {:?}", 
                            src.name(), state.old(), state.current());
                    }
                }
                gst::MessageView::StreamStatus(status) => {
                    if let Some(src) = status.src() {
                        if let Some(watchdog) = watchdog.as_ref() {
                            watchdog.feed(&src.name());
                        }
                    }
                }
                _ => {}
            }
            gstreamer::glib::ControlFlow::Continue
        })
        .expect("Failed to add bus watch");

        std::thread::spawn(move || {
            main_loop.run();
        });
    }

    pub fn get_stream_health(&self, name: &str) -> Option<StreamHealth> {
        self.streams.get(name)
            .map(|info| info.health.lock().unwrap().clone())
    }

    pub fn get_all_stream_names(&self) -> Vec<String> {
        self.streams.iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub fn update_stream_metrics(&self, name: &str, metrics: StreamMetrics) {
        self.metrics_collector.update_metrics(name, metrics);
        if let Some(watchdog) = &self.watchdog {
            watchdog.feed(name);
        }
    }

    pub fn trigger_recovery(&self, stream_name: &str) -> DslResult<()> {
        let mut state_machine = self.state_machine.lock().unwrap();
        
        if let Some(new_state) = state_machine.transition(stream_name, TransitionCondition::OnRecovery) {
            if let Some(info) = self.streams.get(stream_name) {
                let mut health = info.health.lock().unwrap();
                health.state = new_state;
                health.recovery_attempts += 1;
            }
            Ok(())
        } else {
            Err(DslError::StateTransition(
                format!("Cannot recover stream {} from current state", stream_name)
            ))
        }
    }
}

impl Clone for WatchdogTimer {
    fn clone(&self) -> Self {
        Self {
            timeout: self.timeout,
            streams: Arc::clone(&self.streams),
            running: Arc::clone(&self.running),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_machine_transitions() {
        let mut sm = StateMachine::new();
        
        assert_eq!(sm.get_state("test"), StreamState::Idle);
        
        sm.transition("test", TransitionCondition::OnSuccess);
        assert_eq!(sm.get_state("test"), StreamState::Starting);
        
        sm.transition("test", TransitionCondition::OnSuccess);
        assert_eq!(sm.get_state("test"), StreamState::Running);
        
        sm.transition("test", TransitionCondition::OnError);
        assert_eq!(sm.get_state("test"), StreamState::Recovering);
    }

    #[test]
    fn test_pipeline_creation() {
        gst::init().ok();
        
        let config = PipelineConfig::default();
        let pipeline = RobustPipeline::new(config);
        assert!(pipeline.is_ok());
    }
}