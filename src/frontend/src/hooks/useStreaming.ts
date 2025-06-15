import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { streamsApi } from '@/api';
import type { StreamConnectRequest } from '@/types/api';

export const useStreamStatus = () => {
  return useQuery({
    queryKey: ['stream', 'status'],
    queryFn: () => streamsApi.getStatus(),
    refetchInterval: 5000, // Poll every 5 seconds
    select: (data) => data.data,
  });
};

export const useStreamConnect = () => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: (data: StreamConnectRequest) => streamsApi.connect(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['stream', 'status'] });
    },
  });
};

export const useStreamDisconnect = () => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: () => streamsApi.disconnect(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['stream', 'status'] });
    },
  });
};