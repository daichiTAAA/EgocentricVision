export interface StreamStatus {
  stream_id: string; // 追加
  is_connected: boolean;
  is_recording: boolean;
  protocol?: string;
  url?: string;
  connected_at?: string | null;
  error?: string;
}

// 全体取得時は Record<string, StreamStatus> 型で返す
export type StreamStatusMap = Record<string, StreamStatus>;

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

export interface RecordingStopRequest {
  recording_id: string;
}

export interface RecordingDeleteRequest {
  recording_id: string;
}

export interface RecordingDownloadRequest {
  recording_id: string;
}

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}