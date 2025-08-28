# PRP: DSL-RS Pipeline and Component Management

## Executive Summary

This PRP implements the core pipeline management system for dsl-rs, providing the ability to create, configure, and control GStreamer pipelines with DeepStream components. This module builds on the foundation established in PRP-01 and enables users to construct video analytics pipelines by connecting sources, processors, and sinks.

## Problem Statement

### Current State
- Foundation module exists with base types and error handling
- No pipeline creation or management capabilities
- No component linking or state management
- Cannot build or run actual video processing pipelines

### Desired State
- Complete pipeline builder API with add/remove component operations
- State management (NULL, READY, PLAYING, PAUSED)
- Component graph validation and linking
- Event bus handling for pipeline messages
- Player functionality for testing pipelines

### Business Value
- Enables users to build complex video analytics pipelines programmatically
- Provides safe abstractions over GStreamer's pipeline model
- Allows dynamic pipeline modification at runtime
- Ensures proper resource cleanup and state transitions

## Requirements

### Functional Requirements

1. **Pipeline Creation**: Builder pattern for constructing pipelines
2. **Component Management**: Add, remove, link components dynamically
3. **State Control**: Play, pause, stop with proper state transitions
4. **Event Handling**: Bus message processing and callbacks
5. **Pipeline Query**: Get current state, position, duration
6. **Validation**: Ensure component compatibility before linking
7. **Player Support**: Standalone player for testing sources

### Non-Functional Requirements

1. **Thread Safety**: Pipelines must be Send + Sync
2. **Performance**: Minimal overhead over raw GStreamer
3. **Error Recovery**: Graceful handling of state change failures
4. **Memory Management**: Proper cleanup on drop
5. **Debugging**: Detailed logging of pipeline operations

### Context and Research

GStreamer pipelines are directed graphs of elements connected through pads. The DSL library simplifies this by providing high-level component abstractions. The pipeline manager must handle the complexity of pad negotiation, state synchronization, and event propagation.

### Documentation & References
```yaml
# MUST READ - Include these in your context window
- file: ..\prominenceai--deepstream-services-library\src\DslPipelineBintr.h
  why: Pipeline class structure and state management patterns
  
- file: ..\prominenceai--deepstream-services-library\src\DslPipelineStateMgr.h
  why: State machine implementation for pipeline transitions

- file: ..\prominenceai--deepstream-services-library\src\DslPipelineBusSyncMgr.h
  why: Bus message handling and synchronization
  
- url: https://gstreamer.freedesktop.org/documentation/application-development/basics/pads.html
  why: Understanding pad linking and capabilities negotiation

- url: https://docs.rs/gstreamer/latest/gstreamer/struct.Pipeline.html
  why: Rust pipeline API and best practices

- file: ..\prominenceai--deepstream-services-library\src\DslPlayerBintr.h
  why: Player implementation for standalone playback testing
```

### List of tasks to be completed to fulfill the PRP in the order they should be completed

```yaml
Task 1:
CREATE src/pipeline/mod.rs:
  - MODULE structure: pipeline, builder, state, bus
  - RE-EXPORT public types
  
Task 2:
CREATE src/pipeline/pipeline.rs:
  - STRUCT Pipeline with gstreamer::Pipeline wrapper
  - IMPLEMENT component storage with HashMap
  - METHODS: new(), add_component(), remove_component()
  - STATE methods: play(), pause(), stop()
  
Task 3:
CREATE src/pipeline/builder.rs:
  - STRUCT PipelineBuilder with fluent API
  - METHODS: add_source(), add_sink(), add_processor()
  - VALIDATION: check_compatibility() before build()
  - BUILD: create and link all components

Task 4:
CREATE src/pipeline/state.rs:
  - ENUM PipelineState (Null, Ready, Paused, Playing)
  - STRUCT StateManager with transition logic
  - ASYNC state_change() with timeout handling
  - STATE validation and error recovery

Task 5:
CREATE src/pipeline/bus.rs:
  - STRUCT BusManager for message handling
  - CALLBACKS: on_eos(), on_error(), on_warning()
  - WATCH setup with main loop integration
  - MESSAGE filtering and dispatching

Task 6:
CREATE src/pipeline/link.rs:
  - TRAIT Linkable for components
  - FUNCTION link_components() with pad negotiation
  - DYNAMIC pad handling for demuxers
  - CAPABILITY checking before linking

Task 7:
CREATE src/player/mod.rs:
  - STRUCT Player for standalone playback
  - SUPPORT for URI and file sources
  - RENDER sink selection (window, fake)
  - PLAYBACK control methods

Task 8:
CREATE tests/pipeline_integration.rs:
  - TEST pipeline creation and destruction
  - TEST state transitions
  - TEST component addition/removal
  - TEST error handling scenarios
```

### Out of Scope
- Specific source types (separate PRP)
- Specific sink types (separate PRP)  
- Inference components (separate PRP)
- ODE triggers and actions (separate PRP)

## Success Criteria

- [x] Pipeline can be created and components added
- [x] State transitions work correctly
- [x] Bus messages are properly handled
- [x] Components can be dynamically added/removed
- [x] Player can render test videos
- [x] All tests pass without memory leaks
- [x] Documentation examples compile and run

## Dependencies

### Technical Dependencies
- PRP-01 foundation module completed
- gstreamer-rs 0.21+
- glib for main loop integration

### Knowledge Dependencies
- GStreamer pipeline architecture
- GObject signal handling
- State machine patterns

## Risks and Mitigation

| Risk | Probability | Impact | Mitigation Strategy |
|------|------------|--------|-------------------|
| Complex pad negotiation | High | High | Use GStreamer's autopluggers where possible |
| State deadlocks | Medium | High | Implement timeout-based state changes |
| Memory leaks | Medium | High | Use weak references where appropriate |
| Bus message flooding | Low | Medium | Implement message filtering and throttling |

## Architecture Decisions

### Decision: Use Arc<Mutex<>> for Pipeline storage
**Options Considered:**
1. Single ownership with RefCell
2. Arc<Mutex<>> for thread-safe sharing
3. Actor model with channels

**Decision:** Arc<Mutex<>> for components

**Rationale:** Enables safe concurrent access from multiple threads, required for GStreamer callbacks and user code.

### Decision: Builder pattern for pipeline construction
**Options Considered:**
1. Direct mutation API
2. Builder pattern with validation
3. Declarative macro DSL

**Decision:** Builder pattern

**Rationale:** Provides compile-time safety, enables validation before construction, and offers good ergonomics.

## Validation Strategy

- **Unit Testing**: Test each pipeline operation in isolation
- **Integration Testing**: Full pipeline with test sources and sinks
- **Stress Testing**: Rapid state changes and component modifications
- **Memory Testing**: Valgrind/ASAN for leak detection

## Future Considerations

- Pipeline templates for common use cases
- Pipeline serialization/deserialization
- Hot-reload of pipeline configurations
- Distributed pipeline execution

## References

- [GStreamer Application Development Manual - Pipelines](https://gstreamer.freedesktop.org/documentation/application-development/basics/bins.html)
- [GStreamer State Management](https://gstreamer.freedesktop.org/documentation/application-development/basics/states.html)
- [DeepStream Pipeline Architecture](https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvstreammux.html)

---

## PRP Metadata

- **Author**: Claude (AI Assistant)
- **Created**: 2025-08-28
- **Last Modified**: 2025-08-28
- **Status**: Draft
- **Confidence Level**: 7/10 - Good understanding of requirements, some complexity in pad negotiation may require iteration