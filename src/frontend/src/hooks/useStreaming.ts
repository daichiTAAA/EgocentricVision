import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { streamsApi } from '@/api';
import type { StreamConnectRequest, StreamStatus, StreamStatusMap } from '@/types/api';

// stream_id未指定時は全ストリームの状態（StreamStatusMap）を返す
export const useStreamStatus = (stream_id?: string) => {
  const validStreamId = stream_id && stream_id.trim() !== '' ? stream_id : undefined;
  return useQuery<StreamStatus | StreamStatusMap>({
    queryKey: validStreamId ? ['stream', 'status', validStreamId] : ['stream', 'status'],
    queryFn: async () => {
      const res = await streamsApi.getStatus(validStreamId);
      return res.data;
    },
    refetchInterval: 5000, // Poll every 5 seconds
    enabled: validStreamId !== undefined || stream_id === undefined, // stream_idが空文字やnullなら全体取得、明示的にnullなら無効化
  });
};

export const useStreamConnect = () => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: (data: StreamConnectRequest) => streamsApi.connect(data),
    onSuccess: (_data) => {
      // invalidate all stream status queries
      queryClient.invalidateQueries({ queryKey: ['stream', 'status'] });
    },
  });
};

export const useStreamDisconnect = (stream_id: string) => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: () => streamsApi.disconnect(stream_id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['stream', 'status', stream_id] });
    },
  });
};