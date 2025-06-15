import { apiClient } from '@/lib/axios';
import type { Recording, RecordingStartRequest } from '@/types/api';

export const recordingsApi = {
  start: (data?: RecordingStartRequest) =>
    apiClient.post('/api/v1/recordings/start', data),
  
  stop: () =>
    apiClient.post('/api/v1/recordings/stop'),
  
  list: (): Promise<{ data: Recording[] }> =>
    apiClient.get('/api/v1/recordings'),
  
  get: (id: string): Promise<{ data: Recording }> =>
    apiClient.get(`/api/v1/recordings/${id}`),
  
  download: (id: string) =>
    apiClient.get(`/api/v1/recordings/${id}/download`, { responseType: 'blob' }),
  
  delete: (id: string) =>
    apiClient.delete(`/api/v1/recordings/${id}`),
};