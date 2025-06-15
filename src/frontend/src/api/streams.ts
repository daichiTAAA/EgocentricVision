import { apiClient } from '@/lib/axios';
import type { StreamStatus, StreamConnectRequest } from '@/types/api';

export const streamsApi = {
  connect: (data: StreamConnectRequest) =>
    apiClient.post('/api/v1/streams/connect', data),
  
  disconnect: () =>
    apiClient.post('/api/v1/streams/disconnect'),
  
  getStatus: (): Promise<{ data: StreamStatus }> =>
    apiClient.get('/api/v1/streams/status'),
  
  getDebugStatus: () =>
    apiClient.get('/api/v1/streams/debug'),
};