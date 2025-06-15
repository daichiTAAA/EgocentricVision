import React from 'react';
import {
  Card,
  CardContent,
  CardHeader,
  Button,
  Box,
  Typography,
  Chip,
} from '@mui/material';
import { PlayArrow, Stop } from '@mui/icons-material';
import { useStartRecording, useStopRecording } from '@/hooks/useRecording';
import { useStreamStatus } from '@/hooks/useStreaming';
import { useUIStore } from '@/store';

export const RecordingControls: React.FC = () => {
  const { data: streamStatus } = useStreamStatus();
  const startRecordingMutation = useStartRecording();
  const stopRecordingMutation = useStopRecording();
  const { addNotification } = useUIStore();

  const isRecording = false; // TODO: Get from stream status
  const canRecord = streamStatus?.connected;

  const handleStartRecording = async () => {
    try {
      await startRecordingMutation.mutateAsync({});
      addNotification('録画を開始しました', 'success');
    } catch (error) {
      addNotification('録画開始に失敗しました', 'error');
    }
  };

  const handleStopRecording = async () => {
    try {
      await stopRecordingMutation.mutateAsync();
      addNotification('録画を停止しました', 'success');
    } catch (error) {
      addNotification('録画停止に失敗しました', 'error');
    }
  };

  return (
    <Card>
      <CardHeader title="録画制御" />
      <CardContent>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 2, mb: 2 }}>
          <Typography variant="body2">
            ステータス:
          </Typography>
          {isRecording ? (
            <Chip label="録画中" color="error" variant="filled" />
          ) : (
            <Chip label="停止中" color="default" variant="outlined" />
          )}
        </Box>

        {!canRecord && (
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            録画するにはストリームに接続してください
          </Typography>
        )}

        <Box sx={{ display: 'flex', gap: 2 }}>
          {!isRecording ? (
            <Button
              variant="contained"
              startIcon={<PlayArrow />}
              onClick={handleStartRecording}
              disabled={!canRecord || startRecordingMutation.isPending}
            >
              {startRecordingMutation.isPending ? '開始中...' : '録画開始'}
            </Button>
          ) : (
            <Button
              variant="outlined"
              color="error"
              startIcon={<Stop />}
              onClick={handleStopRecording}
              disabled={stopRecordingMutation.isPending}
            >
              {stopRecordingMutation.isPending ? '停止中...' : '録画停止'}
            </Button>
          )}
        </Box>
      </CardContent>
    </Card>
  );
};