# Record Service

This is the Record service for EgocentricVision project, a Rust-based application for managing RTSP/WebRTC video stream recording.

## Features

- REST API for recording management
- RTSP stream connection and recording
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

### List Recordings
```bash
curl http://localhost:3000/api/v1/recordings
```

### Start Recording
```bash
curl -X POST http://localhost:3000/api/v1/recordings \
  -H "Content-Type: application/json" \
  -d '{"rtsp_url": "rtsp://example.com/stream"}'
```

### Stop Recording
```bash
curl -X POST http://localhost:3000/api/v1/recordings/{id}/stop
```

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

## Troubleshooting

### Database Connection Issues
- Ensure PostgreSQL is running: `docker compose ps postgres`
- Check database logs: `docker compose logs postgres`
- Verify connection string in configuration

### Service Won't Start
- Check service logs: `docker compose logs record-service`
- Ensure all required environment variables are set
- Verify port 3000 is not already in use

### Recording Directory Issues
- Ensure the recording directory exists and is writable
- Check volume mounts in docker-compose.yml