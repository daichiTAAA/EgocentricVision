# Record Service

This is the Record service for EgocentricVision project, a Rust-based application for managing RTSP/WebRTC video stream recording.

## Features

- REST API for recording management
- Multiple RTSP/WebRTC stream connection and recording
- PostgreSQL database integration
- Docker containerization support

## Prerequisites

- Docker and Docker Compose
- Git

## Quick Start with Docker Compose

1. Clone the repository:
```bash
git clone https://github.com/daichiTAAA/EgocentricVision.git
cd EgocentricVision
```

2. Start the services:
```bash
docker compose up -d
```

This will start:
- PostgreSQL database on port 5432
- Record service on port 3000

3. Check service status:
```bash
docker compose ps
```

4. View logs:
```bash
# All services
docker compose logs

# Record service only
docker compose logs record-service

# PostgreSQL only
docker compose logs postgres
```

## API Testing

### Health Check
```bash
curl http://localhost:3000/health
```

### Stream Management
```bash
# Connect to Stream
curl -X POST http://localhost:3000/api/v1/streams/connect \
  -H "Content-Type: application/json" \
  -d '{"protocol": "rtsp", "url": "rtsp://192.168.0.18:8554/cam1"}'

# Response
{
  "stream_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "CONNECTING",
  "message": "Stream connection initiated for protocol: rtsp"
}

# List All Streams
curl http://localhost:3000/api/v1/streams/status

# Response
{
  "streams": {
    "550e8400-e29b-41d4-a716-446655440000": {
      "is_connected": true,
      "protocol": "rtsp",
      "url": "rtsp://192.168.0.18:8554/cam1",
      "is_recording": false,
      "connected_at": "2024-03-14T05:30:00Z"
    }
  }
}

# Get Stream Status
curl http://localhost:3000/api/v1/streams/{stream_id}/status

# Response
{
  "is_connected": true,
  "protocol": "rtsp",
  "url": "rtsp://192.168.0.18:8554/cam1",
  "is_recording": false,
  "connected_at": "2024-03-14T05:30:00Z"
}

# Debug Stream Details
curl http://localhost:3000/api/v1/streams/{stream_id}/debug

# Response
{
  "is_connected": true,
  "protocol": "rtsp",
  "url": "rtsp://192.168.0.18:8554/cam1",
  "is_recording": false,
  "connected_at": "2024-03-14T05:30:00Z",
  "pipeline_state": "PLAYING",
  "pipeline_info": {
    "elements": ["rtspsrc", "rtph264depay", "h264parse", "mp4mux", "filesink"],
    "state_details": "All elements in PLAYING state"
  }
}

# Disconnect Stream
curl -X POST http://localhost:3000/api/v1/streams/{stream_id}/disconnect

# Response
{
  "stream_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "DISCONNECTING",
  "message": "Stream disconnection initiated."
}
```

### Recording Management
```bash
# Start Recording
curl -X POST http://localhost:3000/api/v1/recordings/{stream_id}/start

# Response
{
  "recording_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "stream_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "RECORDING_STARTED",
  "message": "Recording has been initiated."
}

# Stop Recording
curl -X POST http://localhost:3000/api/v1/recordings/{stream_id}/stop

# Response
{
  "recording_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "stream_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "RECORDING_STOPPED",
  "message": "Recording has been stopped."
}

# List Recordings
curl http://localhost:3000/api/v1/recordings

# Response
[
  {
    "id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
    "stream_id": "550e8400-e29b-41d4-a716-446655440000",
    "file_name": "rec_20240314_143000.mp4",
    "start_time": "2024-03-14T05:30:00Z",
    "end_time": "2024-03-14T05:45:10Z",
    "duration_seconds": 910,
    "file_size_bytes": 546000000
  }
]

# Get Recording Details
curl http://localhost:3000/api/v1/recordings/{recording_id}

# Response
{
  "id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "stream_id": "550e8400-e29b-41d4-a716-446655440000",
  "file_name": "rec_20240314_143000.mp4",
  "file_path": "/var/data/recordings/rec_20240314_143000.mp4",
  "start_time": "2024-03-14T05:30:00Z",
  "end_time": "2024-03-14T05:45:10Z",
  "duration_seconds": 910,
  "file_size_bytes": 546000000,
  "status": "COMPLETED"
}

# Download Recording
curl http://localhost:3000/api/v1/recordings/{recording_id}/download

# Delete Recording
curl -X DELETE http://localhost:3000/api/v1/recordings/{recording_id}
```

## API Usage Notes

1. **Stream Connection**
   - Connect to a stream before starting recording
   - Supported protocols: RTSP, WebRTC
   - Stream ID is required for all stream-specific operations

2. **Recording Management**
   - Start recording for a specific stream using the stream ID
   - Stop recording using the stream ID
   - List all recordings or get details of a specific recording
   - Download or delete recordings using the recording ID

3. **Troubleshooting**
   - Check stream status using the debug endpoint
   - Verify recording file size and content
   - Check logs for detailed error information
   - Log command: `docker compose logs record-service --tail=100`

# Note: Recording duration should be at least 10 seconds to ensure proper file generation
# and moov atom writing. Short recordings (less than 5 seconds) may result in invalid files.

## API利用時の注意

### ストリーム接続と録画開始の手順

録画を開始するには、必ず事前にストリーム接続APIを呼び出してください。

1. **ストリーム接続**
```bash
curl -X POST http://localhost:3000/api/v1/streams/connect \
  -H "Content-Type: application/json" \
  -d '{"protocol": "rtsp", "url": "rtsp://192.168.0.18:8554/cam1"}'
```
2. **録画開始**
```bash
curl -X POST http://localhost:3000/api/v1/streams/{stream_id}/recordings/start
```

> 事前にストリーム接続せずに録画開始APIを呼ぶと、
> `{ "error_code": "NOT_CONNECTED", "message": "Not connected to stream" }`
> というエラーが返ります。

### 0バイトMP4ファイル問題のトラブルシューティング

録画ファイルが0バイトで作成される場合は、以下を確認してください：

1. **RTSPストリームの確認**
```bash
# コマンドラインでGStreamerを直接テスト
./debug-gstreamer.sh rtsp://192.168.0.18:8554/cam1

# または手動で実行
gst-launch-1.0 -e rtspsrc location=rtsp://192.168.0.18:8554/cam1 latency=0 timeout=20 ! rtph264depay ! h264parse ! mp4mux ! filesink location=/tmp/test.mp4
```

2. **ストリーム状態の詳細確認**
```bash
# 基本ステータス確認
curl http://localhost:3000/api/v1/streams/{stream_id}/status

# 詳細デバッグ情報（GStreamerパイプライン状態含む）
curl http://localhost:3000/api/v1/streams/{stream_id}/debug
```

3. **よくある原因と対処法**
- **RTSP URLが無効**: ネットワーク疎通とRTSPサーバーの動作を確認
- **H264以外のコーデック**: H264エンコードされていないストリームは非対応
- **ネットワークタイムアウト**: ファイアウォールやDocker設定を確認
- **GStreamerエラー**: ログで`GStreamer Error`や`GStreamer Warning`を確認

4. **ログレベルの変更**
Docker環境変数に以下を追加してより詳細なログを出力：
```bash
RUST_LOG=debug
```

### Dockerコンテナからのネットワーク疎通

- ホストOSで `nc -vz 192.168.0.18 8554` が成功しても、Dockerコンテナ内から同じIPにアクセスできるとは限りません。
- 必要に応じて、コンテナ内で下記のように疎通確認してください。

```bash
docker compose exec record-service nc -vz 192.168.0.18 8554
```

### RTSPストリームの確認
- RTSPストリームを受信しmp4ファイルが保存されるか確認するためには、以下のコマンドを実行してみてください。
```bash
gst-launch-1.0 -e rtspsrc location=rtsp://192.168.0.18:8554/cam1 latency=0 ! rtph264depay ! h264parse ! mp4mux ! filesink location=test.mp4
```

- ファイアウォールやDockerネットワーク設定によっては外部アクセスが制限されている場合があります。

## Configuration

The Record service can be configured via:

1. **Configuration file**: `config/record.yaml`
2. **Environment variables**: Prefixed with `RECORD_` (e.g., `RECORD_DATABASE__URL`)

### Key Configuration Options

- `RECORD_DATABASE__URL`: PostgreSQL connection string
- `RECORD_RECORDING_DIRECTORY`: Directory for storing recordings
- `RECORD_SERVER__HOST`: Server host (default: 0.0.0.0)
- `RECORD_SERVER__PORT`: Server port (default: 3000)

## Development

### Local Development (without Docker)

1. Install Rust (https://rustup.rs/)
2. Install PostgreSQL
3. Set up environment variables or modify `config/record.yaml`
4. Run migrations:
```bash
cd src/record
cargo run --bin migration
```
5. Start the service:
```bash
cargo run
```

### Building the Docker Image

```bash
cd src/record
docker build -t record-service .
```

## Stopping the Services

```bash
# Stop all services
docker compose down

# Stop and remove volumes (WARNING: This will delete all data)
docker compose down -v
```

## Known Issues

### Docker Build SSL Certificate Issues
Currently, there may be SSL certificate issues when building the Docker image in certain environments. If you encounter SSL/TLS errors during the Docker build process:

1. **Alternative 1**: Build locally and run with Docker
```bash
# Build the Rust application locally
cd src/record
cargo build --release

# Then start with Docker Compose (PostgreSQL only)
cd ../..
docker compose up -d postgres

# Run the service locally with Docker database
export RECORD_DATABASE__URL="postgres://user:password@localhost:5432/egocentric_vision"
cd src/record
./target/release/record-service
```

2. **Alternative 2**: Use host networking for Docker build
```bash
docker compose build --build-arg BUILDKIT_PROGRESS=plain record-service
```

3. **Alternative 3**: Wait for network connectivity improvements
The Docker build should work in environments with proper certificate chains.

### Testing the Setup
A test script is provided to verify the basic setup:
```bash
./test-docker-setup.sh
```

This validates:
- PostgreSQL container startup
- Database connectivity
- Environment variable configuration
- Volume mounting