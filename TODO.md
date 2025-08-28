# TODO

## Critical Priority - Build & CI

### Windows Build Support
- [ ] Fix Windows CI builds - GStreamer installation issues
- [ ] Document Windows local development setup with GStreamer
- [ ] Consider vcpkg or other package managers for Windows dependencies

### Code Quality
- [ ] Fix all compilation warnings (42+ warnings in lib)
  - [ ] Remove unused imports (`std::sync::Arc`, various tracing imports)
  - [ ] Fix unused variables (prefix with `_` where appropriate)
  - [ ] Fix unused `mut` warnings
  - [ ] Fix unreachable code after `todo!()` macro
  - [ ] Fix private interface warnings in `RecoveryManager`

## High Priority - Testing & Validation

### Endurance Testing
- [ ] Implement 24-hour stability test (`scripts/endurance_test.sh`)
- [ ] Add memory leak detection during long runs
- [ ] Monitor resource usage over time

### Production Readiness
- [ ] Implement proper authentication for RTSP server (`src/sink/rtsp_sink_robust.rs:166`)
- [ ] Add actual memory usage calculation in health monitor (`src/health/health_monitor.rs:265-266`)
- [ ] Implement platform-specific memory checking (`src/health/health_monitor.rs:315`)
- [ ] Add platform-specific disk space checking (`src/sink/file_sink_robust.rs:208`)

### Core Pipeline Features
- [x] Connect sources to sinks through stream manager in example (COMPLETED)
- [ ] Implement proper stream unlinking and removal (`src/stream/stream_manager.rs:241`)
- [ ] Link decoded pads to downstream elements in sources:
  - File source (`src/source/file_source_robust.rs:93`)
  - RTSP source (`src/source/rtsp_source_robust.rs:128`)

## Medium Priority

### Resource Management
- [ ] Implement actual memory limiting in stream isolator (`src/isolation/stream_isolator.rs:212-213`)
- [ ] Add CPU throttling using platform APIs (`src/isolation/stream_isolator.rs:239`)
- [ ] Update memory & CPU usage metrics (`src/isolation/stream_isolator.rs:287` - todo!() macro)
- [ ] Use actual system APIs for resource monitoring
- [ ] Trigger recovery for isolated stream panics

### RTSP Improvements
- [ ] Replace test source with actual upstream pipeline (`src/sink/rtsp_sink_robust.rs:147`)
- [ ] Implement encoder bitrate adjustment for bandwidth adaptation
- [ ] Add force-key-unit event sending  
- [ ] Use real cameras instead of simulated sources in example

### Recovery System
- [ ] Implement proper cloneable trait for RecoveryStrategy (`src/recovery/recovery_manager.rs:435-440`)

## Low Priority

### Documentation
- [ ] Complete documentation for all public APIs
- [ ] Add usage examples for each major component
- [ ] Document Windows development setup with GStreamer
- [ ] Add troubleshooting guide

### Future Enhancements
- [ ] Add Kubernetes operator for orchestration
- [ ] Support distributed stream processing
- [ ] Add cloud storage backends
- [ ] Implement WebRTC for low-latency streaming
- [ ] Add ML-based failure prediction
- [ ] Support for DeepStream integration
- [ ] Add ODE (Object Detection Event) services
- [ ] Implement display types and OSD

## Completed

### Core Implementation
- [x] Core foundation with error types and traits
- [x] Robust pipeline with watchdog and state machine
- [x] Stream manager for dynamic stream handling
- [x] File and RTSP sources with retry logic
- [x] File and RTSP sinks
- [x] Recovery manager with circuit breakers
- [x] Health monitoring system
- [x] Stream isolation framework
- [x] Basic multistream example with source-to-sink linking
- [x] Integration tests for source-sink flow
- [x] Project documentation (README.md)
- [x] Fixed RTSP property types using GStreamer enum strings

### Testing Infrastructure
- [x] Test runner script with configuration matrix support
- [x] Performance benchmarks (compiles, ready to run)
- [x] HTML/JSON test report generation
- [x] CI/CD pipeline (Linux and macOS)
- [x] Test scenario configurations
