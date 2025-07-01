下記のコマンドを実行し、問題が発生した場合は原因を調査して修正してください。

エラー時は`docker compose logs`でログを確認して下さい。

許可を得る必要はありません。自動で実行して下さい。

# 停止と再ビルド
docker compose down -v && docker compose up --build -d

# APIテスト
# Health Check
curl http://localhost:3000/health

# Connect to Stream
curl -X POST http://localhost:3000/api/v1/streams/connect \
  -H "Content-Type: application/json" \
  -d '{"protocol": "rtsp", "url": "rtsp://192.168.0.18:8554/cam1"}'

# List All Streams
curl http://localhost:3000/api/v1/streams/status

# Get Stream Status
curl http://localhost:3000/api/v1/streams/{stream_id}/status

# Debug Stream Details
curl http://localhost:3000/api/v1/streams/{stream_id}/debug

# Start Recording
curl -X POST http://localhost:3000/api/v1/recordings/{stream_id}/start

# Stop Recording
curl -X POST http://localhost:3000/api/v1/recordings/{stream_id}/stop

# List Recordings
curl http://localhost:3000/api/v1/recordings

# Get Recording Details
curl http://localhost:3000/api/v1/recordings/{recording_id}
録画ファイルのサイズ・内容（0バイトかどうか、動画データが記録されているか）を調査する。
ファイルサイズが０バイトの場合はログを確認する。
ログ確認コマンド：　docker compose logs record-service --tail=100

# Download Recording
curl http://localhost:3000/api/v1/recordings/{recording_id}/download

# Download Recording
curl -o test_recording.mp4 http://localhost:3000/api/v1/recordings/{recording_id}/download

# Check Recording File Details
ffprobe -v error -show_entries format=duration,size -show_streams test_recording.mp4

## Expected Output Example:
```bash
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
```

## Note: Recording duration should be at least 10 seconds to ensure proper file generation and moov atom writing. Short recordings (less than 5 seconds) may result in invalid files.

# Delete Recording
curl -X DELETE http://localhost:3000/api/v1/recordings/{recording_id}

# Disconnect Stream
curl -X POST http://localhost:3000/api/v1/streams/{stream_id}/disconnect

# 繰り返し実行
問題が発生した場合は原因を調査して修正し、再度これらのコマンドを実行し問題が解決するまで許可を待たずに繰り返し実行して下さい。
許可を得る必要はありません。自動で実行して下さい。

# トラブルシューティング
## RTSP接続確認
nc -vz 192.168.0.18 8554
docker compose exec record-service nc -vz 192.168.0.18 8554

## GSTプラグイン確認
docker compose exec record-service gst-inspect-1.0 プラグイン名