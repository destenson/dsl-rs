# DSL-RS Examples

## robust_multistream

A comprehensive example demonstrating the DSL-RS multi-stream video processing capabilities with automatic error recovery and dynamic stream management.

### Features

- **Directory Processing**: Automatically discovers and processes all video files in a directory
- **Single File Processing**: Can process individual video files
- **Multi-Stream Support**: Processes multiple video files concurrently
- **Error Recovery**: Automatic recovery from source failures
- **Health Monitoring**: Real-time stream health and metrics reporting
- **File Output**: Records/transcodes videos to output files with rotation support

### Usage

```bash
# Process all videos in a directory
cargo run --example robust_multistream -- /path/to/video/directory

# Process a single video file
cargo run --example robust_multistream -- /path/to/video.mp4

# Use default test directory (./test_videos)
cargo run --example robust_multistream
```

### Supported Video Formats

The example automatically detects and processes the following video formats:
- MP4 (.mp4)
- Matroska (.mkv)
- AVI (.avi)
- QuickTime (.mov)
- WebM (.webm)
- Flash Video (.flv)
- MPEG-TS (.ts)
- M4V (.m4v)

### Output

Processed videos are saved to the `./recordings` directory with:
- Automatic file rotation when reaching 100MB
- Maximum of 10 files per stream
- Output filenames: `output_{original_filename}_*.mp4`

### Monitoring

The example provides real-time monitoring output every 2 seconds showing:
- Stream state (Running, Paused, Error, etc.)
- Error count and recovery attempts
- FPS and bitrate metrics when available

### Configuration

The pipeline is configured with:
- Maximum 8 concurrent streams
- Watchdog timer (10 second timeout)
- Automatic error recovery
- Stream isolation for fault tolerance

### Notes

- If no video files are found or the path doesn't exist, the example will create a test pattern source
- The example runs for approximately 60 seconds in demo mode
- In production, you would implement proper signal handling for graceful shutdown