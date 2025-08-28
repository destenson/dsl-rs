# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Important Tool Usage Guidelines

**ALWAYS use the available tools instead of bash commands:**
- Use `Grep` for searching file contents (not `grep` or `rg`)
- Use `LS` for listing directory contents (not `ls`)
- Use `Glob` for finding files by pattern (not `find`)
- Use `Read` for reading files (not `cat`, `head`, `tail`)
- Use `Edit` or `MultiEdit` for file modifications
- Use MCP Cargo tools for all Rust/Cargo operations (not `cargo` commands)
- Only use `Bash` as a last resort when no appropriate tool exists

## Project Overview

DSL-RS is a Rust implementation of a robust multi-stream video processing framework built on GStreamer. It focuses on production reliability with automatic error recovery, dynamic stream management, and zero-downtime source modifications for 24/7 mission-critical deployments.

## Build and Development Commands

**IMPORTANT: Use the MCP Cargo tools instead of bash commands. The project has MCP tools available for all cargo operations.**

```bash
# Use MCP tools (preferred):
mcp__Cargo__cargo_build         # Build the project
mcp__Cargo__cargo_test          # Run tests  
mcp__Cargo__cargo_check         # Check code without building
mcp__Cargo__cargo_fmt           # Format code
mcp__Cargo__cargo_clippy        # Run clippy lints
mcp__Cargo__cargo_clean         # Clean build artifacts
mcp__Cargo__cargo_doc           # Generate documentation
```

## Architecture Overview

### Core Components

The codebase is organized into domain-specific modules:

- **`core/`** - Foundation types and traits (`DslError`, `Source`, `Sink`, `StreamState`, `RetryConfig`)
- **`pipeline/`** - `RobustPipeline` with watchdog timer, state machine, and metrics collection
- **`stream/`** - `StreamManager` for dynamic stream lifecycle and isolation
- **`source/`** - File and RTSP sources with automatic reconnection
- **`recovery/`** - Circuit breakers, retry strategies, and recovery policies
- **`health/`** - Health monitoring and metrics collection

### Key Design Patterns

1. **Error Recovery**: All components implement `handle_error()` with configurable retry strategies
2. **Stream Isolation**: Each stream runs in its own GStreamer bin with queues for decoupling
3. **State Management**: Centralized state machine tracks stream transitions
4. **Metrics**: Real-time metrics via the `metrics` crate with Prometheus export
5. **Async Operations**: Source/sink operations are async for non-blocking recovery

### Critical Traits

- `Source` - Must implement connection management and error handling
- `Sink` - Must implement preparation/cleanup and error handling
- `RecoveryStrategy` - Defines custom recovery policies

## Development Guidelines

### GStreamer Integration

- Always check GStreamer operations for errors
- Use `gst::prelude::*` for trait imports
- Elements must be added to bins before linking
- State changes should be verified with timeout

### Error Handling

- Use `DslError` enum for all errors
- Implement proper error context with `thiserror`
- Never panic in production code paths
- Log errors at appropriate levels (error, warn, info)

### Testing Approach

- Unit tests for state machines and recovery logic
- Integration tests require GStreamer initialization
- Use `gst::init().ok()` in test setup
- Mock GStreamer elements for complex scenarios

## Current Implementation Status

The project structure is defined with comprehensive module implementations for:
- Core error types and traits
- Robust pipeline with watchdog and state management
- Stream manager for dynamic stream handling
- Recovery manager with circuit breakers
- Source implementations (file and RTSP) with retry logic

The actual implementation needs to be completed based on the PRP specifications in the `PRPs/` directory.
