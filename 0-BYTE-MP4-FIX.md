# 0-byte MP4 Issue - Quick Fix Guide

## Problem
MP4 files are created but have 0-byte size after following the standard recording workflow.

## Quick Solution

### 1. Update the Code
The fix has been implemented in the following files:
- `src/record/src/stream.rs` - Enhanced RTSP connection and recording pipeline
- `src/record/src/api/handlers/streams.rs` - Added debug endpoint
- `src/record/src/api/mod.rs` - Added debug route

### 2. Key Improvements Made
- **Extended timeout**: Increased wait time for RTSP negotiation from 2s to 10s
- **Flexible H264 detection**: Case-insensitive matching and improved caps parsing
- **Enhanced logging**: Detailed GStreamer pipeline diagnostics
- **Optimized MP4 settings**: Added faststart and streamable properties

### 3. Testing Your Fix

#### Step 1: Test GStreamer directly
```bash
# Test the exact pipeline we use
./debug-gstreamer.sh rtsp://192.168.0.18:8554/cam
```

#### Step 2: Test the API with debugging
```bash
# Connect to stream
curl -X POST http://localhost:3000/api/v1/streams/connect \
  -H "Content-Type: application/json" \
  -d '{"protocol": "rtsp", "url": "rtsp://192.168.0.18:8554/cam"}'

# Check detailed status (NEW)
curl http://localhost:3000/api/v1/streams/debug

# Start recording
curl -X POST http://localhost:3000/api/v1/recordings/start

# Check recording was created with non-zero size
curl http://localhost:3000/api/v1/recordings
```

### 4. Diagnostic Information

#### Look for these log messages:
✅ **Success indicators:**
- `Tee is now ready for recording`
- `Successfully linked src_pad to depay sink`
- `Successfully created recording bin`

❌ **Failure indicators:**
- `Tee is not ready after X seconds`
- `Failed to link src_pad to depay sink`
- `GStreamer Error from element`

#### Use the debug endpoint:
```bash
curl http://localhost:3000/api/v1/streams/debug
```

This will show:
- Pipeline state (should be PLAYING)
- Tee readiness status
- Active recording pads
- Connection details

### 5. Common Issues and Solutions

| Issue | Solution |
|-------|----------|
| Still getting 0-byte files | Check `curl http://localhost:3000/api/v1/streams/debug` - Tee should be ready |
| Connection timeout | Verify RTSP URL with `./debug-gstreamer.sh` |
| No H264 stream | Ensure your RTSP source provides H264 video |
| Docker build fails | Use the improved Dockerfile with SSL certificate handling |

### 6. Environment Variables
For more detailed logging, set:
```bash
RUST_LOG=debug
```

The fix addresses the most common causes of 0-byte MP4 files while providing comprehensive debugging tools to identify any remaining issues.