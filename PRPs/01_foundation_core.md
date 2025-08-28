# PRP: DSL-RS Foundation and Core Module

## Executive Summary

This PRP establishes the foundational architecture for the DeepStream Services Library Rust port (dsl-rs), implementing the core type system, error handling, and base traits that all other components will build upon. This foundation is critical as it defines the patterns and abstractions that will ensure type safety, memory safety, and idiomatic Rust code throughout the entire library.

## Problem Statement

### Current State
- The C++ DSL library exists with a mature architecture but uses C-style error codes, manual memory management, and void* patterns
- No Rust implementation exists yet
- The project needs a solid foundation that leverages Rust's ownership model and type system

### Desired State
- A robust Rust foundation module that provides type-safe abstractions over DeepStream/GStreamer
- Error handling using Result types instead of error codes
- Traits and types that enable safe concurrent access to pipeline components
- Integration with existing Rust GStreamer bindings (gstreamer-rs)

### Business Value
- Provides memory safety guarantees without garbage collection
- Enables concurrent pipeline management without data races
- Reduces runtime errors through compile-time type checking
- Creates a maintainable and extensible architecture for the entire library

## Requirements

### Functional Requirements

1. **Error System**: Comprehensive error types covering all DSL result codes
2. **Type System**: Core types for handles, IDs, coordinates, dimensions
3. **Component Traits**: Base traits for all pipeline components
4. **Thread Safety**: Arc/Mutex patterns for shared component access
5. **FFI Bridge**: Safe wrappers for C API compatibility (future requirement)
6. **Logging**: Structured logging using tracing or log crate
7. **Configuration**: Types for component configuration and properties

### Non-Functional Requirements

1. **Performance**: Zero-cost abstractions where possible
2. **Safety**: No unsafe code in public APIs
3. **Compatibility**: Must work with gstreamer-rs 0.21+ 
4. **Testing**: Minimum 80% test coverage for core modules
5. **Documentation**: All public APIs must have rustdoc comments

### Context and Research

The DeepStream Services Library provides a component-based architecture for building video analytics pipelines. The C++ implementation uses a singleton Services class with a C API wrapper. For Rust, we'll use a more idiomatic approach with proper ownership and borrowing.

### Documentation & References
```yaml
# MUST READ - Include these in your context window
- url: https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer/
  why: GStreamer Rust bindings documentation - essential for integration
  
- file: ..\prominenceai--deepstream-services-library\src\DslApi.h
  why: Complete list of error codes and return values to implement
  
- file: ..\prominenceai--deepstream-services-library\src\DslBase.h  
  why: Base component patterns and interfaces to port

- url: https://developer.nvidia.com/deepstream-sdk
  why: DeepStream SDK documentation for understanding the underlying platform
  
- url: https://docs.rs/thiserror/latest/thiserror/
  why: For implementing custom error types with derive macros

- url: https://docs.rs/tracing/latest/tracing/
  why: Structured logging framework for diagnostics
```

### List of tasks to be completed to fulfill the PRP in the order they should be completed

```yaml
Task 1:
MODIFY Cargo.toml:
  - ADD dependencies: gstreamer = "0.21", gstreamer-app = "0.21", gstreamer-video = "0.21"
  - ADD dependencies: thiserror = "1.0", anyhow = "1.0"
  - ADD dependencies: tracing = "0.1", tracing-subscriber = "0.3"
  - ADD dependencies: once_cell = "1.19", arc-swap = "1.7"
  
Task 2:
CREATE src/lib.rs:
  - DEFINE module structure (error, types, component, pipeline)
  - EXPORT public API surface
  - CONFIGURE feature flags for optional components

Task 3:  
CREATE src/error.rs:
  - IMPLEMENT DslError enum with all result codes from DslApi.h
  - DERIVE thiserror traits for automatic From implementations
  - CREATE conversion from gstreamer::Error

Task 4:
CREATE src/types.rs:
  - DEFINE core types: ComponentHandle, PipelineHandle, SourceId
  - IMPLEMENT Coordinate, Dimensions, BoundingBox types
  - CREATE DisplayColor with RGB/RGBA support
  
Task 5:
CREATE src/component.rs:
  - DEFINE Component trait with common operations
  - IMPLEMENT ComponentBase with Arc<Mutex<>> for thread safety
  - CREATE ComponentType enum for runtime type checking

Task 6:
CREATE src/result.rs:
  - TYPE ALIAS DslResult<T> = Result<T, DslError>
  - IMPLEMENT conversion utilities for C FFI (future)
  
Task 7:
CREATE src/logger.rs:
  - SETUP tracing subscriber with env filter
  - CREATE macros for component-specific logging
  - IMPLEMENT log level configuration

Task 8:
CREATE tests/foundation.rs:
  - TEST error conversions and Display implementations
  - TEST component trait implementations
  - TEST thread safety with multiple accessors
```

### Out of Scope
- Actual pipeline implementation (separate PRP)
- Specific component types like sources/sinks (separate PRPs)
- C API compatibility layer (future PRP)
- Python bindings (future PRP)

## Success Criteria

- [x] All error types from DSL C++ are represented in Rust
- [x] Core types compile without warnings
- [x] Component trait can be implemented by a test struct
- [x] Thread safety demonstrated with concurrent access test
- [x] All public APIs have documentation
- [x] cargo test passes with no failures
- [x] cargo clippy shows no warnings

## Dependencies

### Technical Dependencies
- gstreamer-rs crate ecosystem
- Rust 1.70+ (for stable async traits)
- NVIDIA DeepStream SDK 6.0+ (runtime)

### Knowledge Dependencies
- Understanding of GStreamer pipeline architecture
- Rust ownership and borrowing model
- Arc/Mutex patterns for shared state

## Risks and Mitigation

| Risk | Probability | Impact | Mitigation Strategy |
|------|------------|--------|-------------------|
| gstreamer-rs API changes | Low | Medium | Pin to specific version, monitor changelog |
| Performance overhead from safety | Medium | Medium | Profile and optimize hot paths, use unsafe where justified |
| Missing DeepStream bindings | High | High | May need to create custom sys crate for DeepStream |

## Architecture Decisions

### Decision: Use gstreamer-rs instead of raw FFI
**Options Considered:**
1. Direct FFI bindings to GStreamer C API
2. Use existing gstreamer-rs crate

**Decision:** Use gstreamer-rs crate

**Rationale:** Provides safe abstractions, active maintenance, and idiomatic Rust patterns. Reduces unsafe code significantly.

### Decision: Error handling with thiserror
**Options Considered:**
1. Manual Error implementation
2. thiserror derive macros
3. anyhow for all errors

**Decision:** thiserror for library errors, anyhow for application errors

**Rationale:** thiserror provides zero-cost custom errors with good ergonomics. anyhow is better for applications than libraries.

## Validation Strategy

- **Unit Testing**: Test each error variant, type conversion, and trait implementation
- **Integration Testing**: Create mock pipeline with base components
- **Documentation Testing**: Ensure all doctests pass
- **Thread Safety Testing**: Use loom or similar for concurrency testing

## Future Considerations

- C API compatibility layer for existing DSL users
- Python bindings using PyO3
- Async/await support for pipeline operations
- Custom derive macros for component creation

## References

- [GStreamer Application Development Manual](https://gstreamer.freedesktop.org/documentation/application-development/index.html)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [DeepStream SDK Documentation](https://docs.nvidia.com/metropolis/deepstream/dev-guide/)

---

## PRP Metadata

- **Author**: Claude (AI Assistant)
- **Created**: 2025-08-28
- **Last Modified**: 2025-08-28
- **Status**: Draft
- **Confidence Level**: 8/10 - High confidence in architectural approach, some uncertainty around DeepStream-specific bindings that may require iteration