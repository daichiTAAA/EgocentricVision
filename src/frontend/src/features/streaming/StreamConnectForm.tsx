import React from 'react';
import { useForm } from 'react-hook-form';
import {
  Card,
  CardContent,
  CardHeader,
  TextField,
  Button,
  Box,
  Alert,
} from '@mui/material';
import { useStreamConnect, useStreamDisconnect, useStreamStatus } from '@/hooks/useStreaming';
import { useUIStore } from '@/store';

interface StreamConnectFormData {
  url: string;
  username?: string;
  password?: string;
}

export const StreamConnectForm: React.FC = () => {
  const { register, handleSubmit, formState: { errors } } = useForm<StreamConnectFormData>();
  const { data: streamStatus } = useStreamStatus();
  const connectMutation = useStreamConnect();
  const disconnectMutation = useStreamDisconnect();
  const { addNotification } = useUIStore();

  const onSubmit = async (data: StreamConnectFormData) => {
    try {
      await connectMutation.mutateAsync({
        ...data,
        protocol: 'rtsp'
      });
      addNotification('ストリームに接続しました', 'success');
    } catch (error: any) {
      const errorMessage = error.response?.data?.error || 'ストリーム接続に失敗しました';
      addNotification(errorMessage, 'error');
    }
  };

  const handleDisconnect = async () => {
    try {
      await disconnectMutation.mutateAsync();
      addNotification('ストリームを切断しました', 'success');
    } catch (error: any) {
      const errorMessage = error.response?.data?.error || 'ストリーム切断に失敗しました';
      addNotification(errorMessage, 'error');
    }
  };

  return (
    <Card>
      <CardHeader title="ストリーム接続" />
      <CardContent>
        {streamStatus?.is_connected && (
          <Alert severity="success" sx={{ mb: 2 }}>
            ストリーム接続中: {streamStatus.url}
          </Alert>
        )}
        
        {streamStatus?.error && (
          <Alert severity="error" sx={{ mb: 2 }}>
            エラー: {streamStatus.error}
          </Alert>
        )}

        <Box component="form" onSubmit={handleSubmit(onSubmit)} sx={{ mt: 2 }}>
          <TextField
            {...register('url', {
              required: 'URLは必須です',
              pattern: {
                value: /^rtsp:\/\/.+/,
                message: 'RTSP URLを入力してください（例: rtsp://example.com/stream）'
              }
            })}
            label="ストリームURL"
            fullWidth
            margin="normal"
            error={!!errors.url}
            helperText={errors.url?.message}
            placeholder="rtsp://example.com/stream"
            disabled={streamStatus?.is_connected}
          />
          
          <TextField
            {...register('username')}
            label="ユーザー名（任意）"
            fullWidth
            margin="normal"
            disabled={streamStatus?.is_connected}
          />
          
          <TextField
            {...register('password')}
            label="パスワード（任意）"
            type="password"
            fullWidth
            margin="normal"
            disabled={streamStatus?.is_connected}
          />

          <Box sx={{ mt: 2, display: 'flex', gap: 2 }}>
            {!streamStatus?.is_connected ? (
              <Button
                type="submit"
                variant="contained"
                disabled={connectMutation.isPending}
              >
                {connectMutation.isPending ? '接続中...' : '接続'}
              </Button>
            ) : (
              <Button
                variant="outlined"
                color="error"
                onClick={handleDisconnect}
                disabled={disconnectMutation.isPending}
              >
                {disconnectMutation.isPending ? '切断中...' : '切断'}
              </Button>
            )}
          </Box>
        </Box>
      </CardContent>
    </Card>
  );
};