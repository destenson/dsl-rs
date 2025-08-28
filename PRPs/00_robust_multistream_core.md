# PRP: DSL-RS Robust Multi-Stream Processing Core

## Executive Summary

This PRP defines the first iteration of dsl-rs, focusing on building an extremely robust multi-stream processing core that handles file and RTSP sources/sinks with automatic error recovery, dynamic stream management, and zero-downtime source modifications. This foundation prioritizes production reliability over feature completeness, ensuring the system can run 24/7 in mission-critical deployments.

## Problem Statement

### Current State
- No Rust implementation exists
- C++ DSL requires pipeline restart for source changes
- Network disconnections can crash or freeze pipelines
- Error recovery often requires manual intervention
- Adding/removing streams impacts other streams' performance

### Desired State
- Rock-solid pipeline that auto-recovers from all failures
- Hot-swap sources without affecting other streams
- Automatic RTSP reconnection with exponential backoff
- File sources with seamless loop restart
- Performance isolation between streams
- Comprehensive error handling without panics

### Business Value
- Enable 24/7 production deployments without operator intervention
- Reduce downtime from network issues to zero
- Support dynamic surveillance scenarios (cameras coming online/offline)
- Provide foundation for cloud-native video analytics
- Reduce operational costs through automation

## Requirements

### Functional Requirements

1. **File Source**: MP4/MKV with automatic loop restart on EOF
2. **RTSP Source**: Client with automatic reconnection (configurable retry strategy)
3. **File Sink**: MP4/MKV recording with rotation
4. **RTSP Sink**: Server with client management
5. **Dynamic Sources**: Add/remove sources without pipeline restart
6. **Stream Isolation**: One stream's failure doesn't affect others
7. **Error Recovery**: Automatic recovery from all transient failures
8. **Health Monitoring**: Per-stream health status and metrics

### Non-Functional Requirements

1. **Availability**: 99.9% uptime for healthy streams
2. **Performance**: <5% CPU overhead for error handling
3. **Latency**: <100ms to detect and begin recovery
4. **Scalability**: Handle 32+ concurrent streams
5. **Memory**: No memory leaks during stream churn

### Context and Research

Building a robust multi-stream system requires careful consideration of GStreamer's threading model, proper resource isolation, and comprehensive error handling at every level. The implementation must handle partial failures gracefully while maintaining overall system stability.

### Documentation & References
```yaml
# MUST READ - Include these in your context window

# REFERENCE IMPLEMENTATIONS - Study these for patterns and best practices
- dir: ..\prominenceai--deepstream-services-library\src\
  why: Complete C++ DSL implementation - study error handling, state management, and pipeline patterns
  key_files:
    - DslSourceBintr.cpp - Source state management and error recovery
    - DslSinkBintr.cpp - Sink implementations and buffering
    - DslPipelineBintr.cpp - Pipeline orchestration and dynamic management
    - DslPipelineSourcesBintr.cpp - Multi-source management patterns

- dir: ..\prominenceai--deepstream-services-library\examples\
  why: Working examples of dynamic pipelines and error handling
  key_examples:
    - cpp\dynamically_add_remove_sources_with_tiler_window_sink.cpp
    - python\dynamically_add_remove_sources_with_tiler_window_sink.py
    - cpp\smart_record_sink_start_session_on_ode_occurrence.cpp

- dir: ..\NVIDIA-AI-IOT--deepstream_reference_apps\
  why: NVIDIA reference implementations showing DeepStream best practices
  key_apps:
    - runtime_source_add_delete\ - Dynamic source management
    - deepstream-3d-sensor-fusion\ - Multi-stream processing
    - anomaly\ - Robust pipeline patterns

# TECHNICAL DOCUMENTATION
- url: https://gstreamer.freedesktop.org/documentation/additional/design/states.html
  why: Understanding state management for dynamic pipelines

- url: https://gstreamer.freedesktop.org/documentation/application-development/advanced/threads.html
  why: Threading model for stream isolation

- url: https://docs.rs/glib/latest/glib/
  why: GLib async patterns and MainLoop integration

- url: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html
  why: Async stream handling for events
```

### List of tasks to be completed to fulfill the PRP in the order they should be completed

```yaml
Task 1:
CREATE src/core/mod.rs:
  - MINIMAL foundation (just error types and result)
  - BASIC logging setup
  - CORE traits for Source and Sink

Task 2:
CREATE src/pipeline/robust_pipeline.rs:
  - STRUCT RobustPipeline with error isolation
  - WATCHDOG timer for health monitoring
  - STATE recovery state machine
  - METRIC collection per stream

Task 3:
CREATE src/stream/stream_manager.rs:
  - STRUCT StreamManager for dynamic sources
  - ISOLATION via separate bins
  - ADD/REMOVE without pipeline state change
  - QUEUE elements for decoupling

Task 4:
CREATE src/source/file_source_robust.rs:
  - LOOP restart on EOF
  - ERROR recovery on decode failure
  - POSITION tracking for resume
  - FILE validation before play

Task 5:
CREATE src/source/rtsp_source_robust.rs:
  - CONNECTION state machine
  - CONFIGURABLE retry strategy (immediate, fixed delay, custom)
  - TIMEOUT handling (connect, stream)
  - CREDENTIAL retry on 401
  - NETWORK error classification

Task 6:
CREATE src/recovery/recovery_manager.rs:
  - STRATEGY pattern for recovery (configurable per source)
  - CIRCUIT breaker to prevent thrashing
  - RETRY policy configuration (attempts, delays)
  - TELEMETRY for failure patterns

Task 7:
CREATE src/health/health_monitor.rs:
  - PER-STREAM health status
  - METRICS: fps, bitrate, errors, uptime
  - DEADLOCK detection
  - MEMORY monitoring
  - EVENT log with ring buffer

Task 8:
CREATE src/sink/file_sink_robust.rs:
  - ROTATION on size/time
  - DISK space monitoring
  - WRITE failure recovery
  - ATOMIC file operations

Task 9:
CREATE src/sink/rtsp_sink_robust.rs:
  - CLIENT connection management
  - GRACEFUL client disconnect
  - BANDWIDTH adaptation
  - KEY frame generation

Task 10:
CREATE src/isolation/stream_isolator.rs:
  - THREAD pool per stream
  - MEMORY quota enforcement
  - CPU throttling
  - PANIC isolation with catch_unwind

Task 11:
CREATE examples/robust_multistream.rs:
  - DEMO with 4 RTSP sources
  - SIMULATE network failures
  - SHOW recovery in action
  - METRICS dashboard

Task 12:
CREATE tests/chaos_testing.rs:
  - RANDOM failure injection
  - NETWORK partition simulation
  - MEMORY pressure testing
  - LONG-RUNNING stability test
```

### Out of Scope
- Inference components
- ODE services  
- Complex processing (just pass-through initially)
- Display types and OSD
- WebRTC or other advanced sinks
- Transcoding or format conversion

## Success Criteria

- [ ] Pipeline runs for 7 days without intervention
- [ ] RTSP disconnections recover within 5 seconds
- [ ] Adding source doesn't interrupt other streams
- [ ] Removing source doesn't cause memory leaks
- [ ] 32 concurrent streams with <5% CPU per stream
- [ ] Zero panics during chaos testing
- [ ] Memory usage stable over 24 hours

## Dependencies

### Technical Dependencies
- gstreamer-rs 0.21+
- glib for async runtime (GMainLoop)
- tracing for structured logging
- metrics-rs for telemetry
- dashmap for concurrent collections

### Knowledge Dependencies
- GStreamer dynamic pipeline patterns
- Async Rust patterns
- Error recovery strategies
- Network programming

## Risks and Mitigation

| Risk | Probability | Impact | Mitigation Strategy |
|------|------------|--------|-------------------|
| GStreamer thread deadlocks | Medium | High | Careful lock ordering, timeout-based recovery |
| Memory leaks in native code | Medium | High | Valgrind testing, periodic restart capability |
| Cascade failures | Low | High | Circuit breakers, resource isolation |
| Network storm from reconnects | Medium | Medium | Configurable retry delays, jitter, global rate limiting |

## Architecture Decisions

### Decision: Bin-per-stream isolation
**Options Considered:**
1. Single pipeline with all sources
2. Separate bin per source
3. Separate pipeline per source

**Decision:** Separate bin per source

**Rationale:** Provides isolation while allowing shared resources, enables dynamic management, matches DeepStream model.

### Decision: GLib async runtime
**Options Considered:**
1. GLib MainLoop only
2. Tokio for control plane (incompatible)
3. Custom thread pool

**Decision:** GLib MainLoop with async extensions

**Rationale:** Native integration with GStreamer, no compatibility issues, proven in production DSL.

### Decision: Recovery through state machines
**Options Considered:**
1. Simple retry loops
2. State machine per source
3. Global recovery coordinator

**Decision:** State machine per source with coordinator

**Rationale:** Explicit states make debugging easier, prevents invalid transitions, enables sophisticated recovery.

## Validation Strategy

- **Unit Testing**: Test each recovery mechanism
- **Integration Testing**: Multi-stream scenarios
- **Chaos Testing**: Failure injection framework
- **Load Testing**: Maximum stream count
- **Endurance Testing**: 7-day continuous run
- **Network Testing**: Various failure modes

## Future Considerations

- Kubernetes operator for orchestration
- Distributed stream processing
- Cloud storage backends
- WebRTC for low-latency streaming
- ML-based failure prediction

## References

- [GStreamer Dynamic Pipelines](https://gstreamer.freedesktop.org/documentation/tutorials/basic/dynamic-pipelines.html)
- [GLib Main Loop](https://docs.gtk.org/glib/main-loop.html)
- [Circuit Breaker Pattern](https://martinfowler.com/bliki/CircuitBreaker.html)
- [Error Handling in Rust](https://nick.groenen.me/posts/rust-error-handling/)

---

## PRP Metadata

- **Author**: Claude (AI Assistant)
- **Created**: 2025-08-27
- **Last Modified**: 2025-08-27
- **Status**: Draft
- **Confidence Level**: 9/10 - Focused scope with clear requirements, builds on proven patterns, emphasizes robustness over features
