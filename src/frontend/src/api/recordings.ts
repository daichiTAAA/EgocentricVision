import apiClient from '@/lib/axios';
import type { Recording, RecordingStartRequest } from '@/types/api';

export const recordingsApi = {
  start: (stream_id: string, data?: RecordingStartRequest) =>
    apiClient.post(`/api/v1/recordings/${stream_id}/start`, data),
  
  stop: (stream_id: string) =>
    apiClient.post(`/api/v1/recordings/${stream_id}/stop`),
  
  list: (stream_id?: string): Promise<{ data: Recording[] }> =>
    stream_id
      ? apiClient.get(`/api/v1/recordings?stream_id=${stream_id}`)
      : apiClient.get('/api/v1/recordings'),
  
  get: (recording_id: string): Promise<{ data: Recording }> =>
    apiClient.get(`/api/v1/recordings/${recording_id}`),
  
  download: (recording_id: string) =>
    apiClient.get(`/api/v1/recordings/${recording_id}/download`, {
      responseType: 'blob',
      headers: {
        Accept: 'video/mp4'
      }
    }),
  
  delete: (recording_id: string) =>
    apiClient.delete(`/api/v1/recordings/${recording_id}`),
};