import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { recordingsApi } from '@/api';
import type { RecordingStartRequest } from '@/types/api';

export const useRecordings = (stream_id?: string) => {
  return useQuery({
    queryKey: ['recordings', stream_id],
    queryFn: () => recordingsApi.list(stream_id),
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

export const useStartRecording = (stream_id: string) => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: (data?: RecordingStartRequest) => recordingsApi.start(stream_id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recordings', stream_id] });
      queryClient.invalidateQueries({ queryKey: ['stream', 'status', stream_id] });
    },
  });
};

export const useStopRecording = (stream_id: string) => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: () => recordingsApi.stop(stream_id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recordings', stream_id] });
      queryClient.invalidateQueries({ queryKey: ['stream', 'status', stream_id] });
    },
  });
};

export const useDeleteRecording = () => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: (id: string) => recordingsApi.delete(id),
    onSuccess: (_data, _id) => {
      // invalidate all recordings queries
      queryClient.invalidateQueries({ queryKey: ['recordings'] });
    },
  });
};