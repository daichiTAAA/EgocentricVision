import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { recordingsApi } from '@/api';
import type { RecordingStartRequest } from '@/types/api';

export const useRecordings = () => {
  return useQuery({
    queryKey: ['recordings'],
    queryFn: () => recordingsApi.list(),
    select: (data) => data.data,
  });
};

export const useRecording = (id: string) => {
  return useQuery({
    queryKey: ['recording', id],
    queryFn: () => recordingsApi.get(id),
    select: (data) => data.data,
    enabled: !!id,
  });
};

export const useStartRecording = () => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: (data?: RecordingStartRequest) => recordingsApi.start(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recordings'] });
      queryClient.invalidateQueries({ queryKey: ['stream', 'status'] });
    },
  });
};

export const useStopRecording = () => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: () => recordingsApi.stop(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recordings'] });
      queryClient.invalidateQueries({ queryKey: ['stream', 'status'] });
    },
  });
};

export const useDeleteRecording = () => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: (id: string) => recordingsApi.delete(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recordings'] });
    },
  });
};