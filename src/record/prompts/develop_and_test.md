下記のコマンドを実行し、問題が発生場合は原因を調査して修正し、再度これらのコマンドを実行し問題が解決するまで許可を待たずに繰り返し実行して下さい。

許可を得る必要はありません。自動で実行して下さい。

1. 停止と再ビルド
        docker compose down -v && docker compose up --build -d

## API Testing

### Health Check
```bash
curl http://localhost:3000/health
```

### Stream Management
```bash
# Connect to RTSP Stream
curl -X POST http://localhost:3000/api/v1/streams/connect \
  -H "Content-Type: application/json" \
  -d '{"protocol": "rtsp", "url": "rtsp://192.168.0.18:8554/cam"}'

# Check Stream Status
curl http://localhost:3000/api/v1/streams/status

# Debug Stream Details (includes GStreamer pipeline state)
curl http://localhost:3000/api/v1/streams/debug

# Disconnect from Stream
curl -X POST http://localhost:3000/api/v1/streams/disconnect
```

### Recording Management
```bash
# Start Recording
curl -X POST http://localhost:3000/api/v1/recordings/start


# List Recordings
curl http://localhost:3000/api/v1/recordings

# Get Recording Details
curl http://localhost:3000/api/v1/recordings/{id}
録画ファイルのサイズ・内容（0バイトかどうか、動画データが記録されているか）を調査する。
ファイルサイズが０バイトの場合はログを確認する。
ログ確認コマンド：　docker compose logs record-service --tail=100

# Stop Recording
curl -X POST http://localhost:3000/api/v1/recordings/stop

# Download Recording
curl http://localhost:3000/api/v1/recordings/{id}/download

# Delete Recording
curl -X DELETE http://localhost:3000/api/v1/recordings/{id}
```

### Recording File Validation
```bash
# Download Recording
curl -o test_recording.mp4 http://localhost:3000/api/v1/recordings/{id}/download

# Check Recording File Details
ffprobe -v error -show_entries format=duration,size -show_streams test_recording.mp4

# Expected Output Example:
# [STREAM]
# index=0
# codec_name=h264
# codec_type=video
# width=1280
# height=720
# r_frame_rate=15/1
# duration=12.722000
# [FORMAT]
# duration=12.722000
# size=8019990
# [/FORMAT]

# Note: Recording duration should be at least 10 seconds to ensure proper file generation
# and moov atom writing. Short recordings (less than 5 seconds) may result in invalid files.
```

