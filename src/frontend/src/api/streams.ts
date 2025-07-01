import apiClient from '@/lib/axios';
import type { StreamStatus, StreamConnectRequest } from '@/types/api';

export const streamsApi = {
  connect: (data: StreamConnectRequest) =>
    apiClient.post('/api/v1/streams/connect', data),
  
  disconnect: (stream_id: string) =>
    apiClient.post(`/api/v1/streams/${stream_id}/disconnect`),
  
  getStatus: (stream_id?: string): Promise<{ data: StreamStatus | Record<string, StreamStatus> }> =>
    stream_id
      ? apiClient.get(`/api/v1/streams/${stream_id}/status`)
      : apiClient.get('/api/v1/streams/status'),
  
  getDebugStatus: (stream_id: string) =>
    apiClient.get(`/api/v1/streams/${stream_id}/debug`),
};