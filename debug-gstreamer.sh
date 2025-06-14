#!/bin/bash

# Debug script to test GStreamer RTSP pipeline
# Usage: ./debug-gstreamer.sh [RTSP_URL]

set -e

RTSP_URL=${1:-"rtsp://192.168.0.18:8554/cam"}
OUTPUT_FILE="/tmp/test_recording.mp4"

echo "🔍 Testing GStreamer RTSP Pipeline"
echo "========================================"
echo "RTSP URL: $RTSP_URL"
echo "Output file: $OUTPUT_FILE"
echo ""

# Check if gst-launch-1.0 is available
if ! command -v gst-launch-1.0 &> /dev/null; then
    echo "❌ gst-launch-1.0 not found. Installing GStreamer tools..."
    sudo apt-get update && sudo apt-get install -y gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav
fi

# Clean up previous test file
rm -f "$OUTPUT_FILE"

echo "🚀 Testing RTSP connection and recording for 10 seconds..."
echo "Command: gst-launch-1.0 -e rtspsrc location=$RTSP_URL latency=0 timeout=20 retry=3 ! rtph264depay ! h264parse config-interval=-1 ! mp4mux ! filesink location=$OUTPUT_FILE"
echo ""

# Run the pipeline for 10 seconds
timeout 10s gst-launch-1.0 -e rtspsrc location="$RTSP_URL" latency=0 timeout=20 retry=3 ! rtph264depay ! h264parse config-interval=-1 ! mp4mux ! filesink location="$OUTPUT_FILE" || {
    echo "⚠️  Pipeline terminated (expected after 10s timeout)"
}

echo ""
echo "📊 Results:"
echo "========================================"

if [ -f "$OUTPUT_FILE" ]; then
    FILE_SIZE=$(stat -c%s "$OUTPUT_FILE" 2>/dev/null || echo "0")
    echo "✅ Output file created: $OUTPUT_FILE"
    echo "📐 File size: $FILE_SIZE bytes"
    
    if [ "$FILE_SIZE" -gt 0 ]; then
        echo "🎉 SUCCESS: Non-zero file created!"
        echo ""
        echo "📹 File details:"
        if command -v ffprobe &> /dev/null; then
            ffprobe -v quiet -print_format json -show_format -show_streams "$OUTPUT_FILE" 2>/dev/null || echo "Could not analyze file with ffprobe"
        else
            ls -lh "$OUTPUT_FILE"
        fi
    else
        echo "❌ FAILURE: File is 0 bytes"
    fi
else
    echo "❌ FAILURE: No output file created"
fi

echo ""
echo "🔧 Debugging suggestions:"
echo "========================================"
echo "1. Check network connectivity:"
echo "   nc -vz 192.168.0.18 8554"
echo ""
echo "2. Test RTSP stream with VLC or similar player"
echo ""
echo "3. Check Docker container network if running in Docker:"
echo "   docker compose exec record-service nc -vz 192.168.0.18 8554"
echo ""
echo "4. Try different GStreamer debug levels:"
echo "   GST_DEBUG=3 gst-launch-1.0 ..."
echo ""