export interface StreamStatus {
  is_connected: boolean;
  is_recording: boolean;
  protocol?: string;
  url?: string;
  connected_at?: string | null;
  error?: string;
}

export interface Recording {
  id: string;
  filename: string;
  start_time: string;
  end_time?: string;
  duration_seconds?: number;
  file_size_bytes?: number;
  status: 'RECORDING' | 'COMPLETED' | 'FAILED';
}

export interface StreamConnectRequest {
  protocol: 'rtsp';
  url: string;
  username?: string;
  password?: string;
}

export interface RecordingStartRequest {
  filename?: string;
}

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}