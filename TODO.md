# TODO

## Critical Priority - Testing & Validation

### Systematic Testing Framework (PRPs/10_systematic_testing_framework.md)
- [ ] Create common test utilities (`tests/common/mod.rs`)
- [ ] Enhance unit test coverage to >80% for all modules
- [ ] Add integration tests for multi-stream scenarios (`tests/integration/`)
- [ ] Implement chaos testing for network failures (`tests/chaos/network_failures.rs`)
- [ ] Add resource exhaustion tests (`tests/chaos/resource_exhaustion.rs`)
- [ ] Create performance benchmarks (`tests/performance/benchmarks.rs`)
- [ ] Add configuration matrix tests (`tests/configurations/matrix_tests.rs`)
- [ ] Implement endurance test for 24-hour stability

### Test Runner Script (PRPs/11_test_runner_script.md)
- [ ] Create Python test runner script (`scripts/test_runner.py`)
- [ ] Implement configuration matrix generator (`scripts/lib/config_generator.py`)
- [ ] Add test executor with parallel support (`scripts/lib/test_executor.py`)
- [ ] Create HTML/JSON report generator (`scripts/lib/report_generator.py`)
- [ ] Add test scenarios YAML configs (`scripts/configs/scenarios/`)
- [ ] Create platform-specific wrappers (`.sh` and `.ps1`)
- [ ] Add CI/CD integration (`.github/workflows/test.yml`)

## High Priority

### Production Readiness
- [ ] Implement proper authentication for RTSP server (`src/sink/rtsp_sink_robust.rs:161`)
- [ ] Add actual memory usage calculation in health monitor (`src/health/health_monitor.rs:255-256`)
- [ ] Implement platform-specific memory checking (`src/health/health_monitor.rs:305-306`)
- [ ] Add platform-specific disk space checking (`src/sink/file_sink_robust.rs:204-205`)

### Core Pipeline Features
- [ ] Connect sources to sinks through stream manager in example (`examples/robust_multistream.rs:70-71`)
- [ ] Implement proper stream unlinking and removal (`src/stream/stream_manager.rs:228`)
- [ ] Link decoded pads to downstream elements in sources:
  - File source (`src/source/file_source_robust.rs:95`)
  - RTSP source (`src/source/rtsp_source_robust.rs:131`)

## Medium Priority

### Resource Management
- [ ] Implement actual memory limiting in stream isolator (`src/isolation/stream_isolator.rs:206-207`)
- [ ] Add CPU throttling using platform APIs (`src/isolation/stream_isolator.rs:230`)
- [ ] Use actual system APIs for resource monitoring (`src/isolation/stream_isolator.rs:273`)
- [ ] Trigger recovery for isolated stream panics (`src/isolation/stream_isolator.rs:101`)

### RTSP Improvements
- [ ] Replace test source with actual upstream pipeline (`src/sink/rtsp_sink_robust.rs:144`)
- [ ] Implement encoder bitrate adjustment for bandwidth adaptation (`src/sink/rtsp_sink_robust.rs:217`)
- [ ] Add force-key-unit event sending (`src/sink/rtsp_sink_robust.rs:235`)
- [ ] Use real cameras instead of simulated sources in example (`examples/robust_multistream.rs:41`)

### Recovery System
- [ ] Implement proper cloneable trait for RecoveryStrategy (`src/recovery/recovery_manager.rs:429-430`)

## Low Priority

### Code Quality
- [ ] Fix all compilation warnings (unused variables, imports, etc.)
- [ ] Remove underscore prefixes from actually used parameters
- [ ] Add proper error handling for all edge cases
- [ ] Complete documentation for all public APIs

### Future Enhancements
- [ ] Add Kubernetes operator for orchestration
- [ ] Support distributed stream processing
- [ ] Add cloud storage backends
- [ ] Implement WebRTC for low-latency streaming
- [ ] Add ML-based failure prediction
- [ ] Support for DeepStream integration
- [ ] Add ODE (Object Detection Event) services
- [ ] Implement display types and OSD

## Completed (but needs testing)

- [x] Core foundation with error types and traits
- [x] Robust pipeline with watchdog and state machine
- [x] Stream manager for dynamic stream handling
- [x] File and RTSP sources with retry logic
- [x] File and RTSP sinks
- [x] Recovery manager with circuit breakers
- [x] Health monitoring system
- [x] Stream isolation framework
- [x] Basic multistream example
- [x] Project documentation (README.md)
