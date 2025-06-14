-- Create custom enum type for recording status
CREATE TYPE recording_status AS ENUM ('RECORDING', 'COMPLETED', 'FAILED');

-- Create recordings table
CREATE TABLE recordings (
    id UUID PRIMARY KEY,
    file_name TEXT NOT NULL,
    file_path TEXT NOT NULL UNIQUE,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,
    duration_seconds BIGINT,
    file_size_bytes BIGINT,
    status recording_status NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create index for better performance on start_time ordering
CREATE INDEX idx_recordings_start_time ON recordings (start_time DESC);