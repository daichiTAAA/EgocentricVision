export interface StreamStatus {
  connected: boolean;
  url?: string;
  error?: string;
}

export interface Recording {
  id: string;
  filename: string;
  start_time: string;
  end_time?: string;
  duration?: number;
  size?: number;
  status: 'recording' | 'completed' | 'failed';
}

export interface StreamConnectRequest {
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