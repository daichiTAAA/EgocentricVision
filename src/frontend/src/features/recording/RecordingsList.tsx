import React from 'react';
import {
  Card,
  CardContent,
  CardHeader,
  List,
  ListItem,
  ListItemText,
  ListItemSecondaryAction,
  IconButton,
  Typography,
  Chip,
  Box,
} from '@mui/material';
import { Download, Delete, PlayArrow } from '@mui/icons-material';
import { useRecordings, useDeleteRecording } from '@/hooks/useRecording';
import { useUIStore } from '@/store';

export const RecordingsList: React.FC = () => {
  const { data: recordings, isLoading } = useRecordings();
  const deleteRecordingMutation = useDeleteRecording();
  const { addNotification } = useUIStore();

  const handleDelete = async (id: string) => {
    if (window.confirm('この録画を削除しますか？')) {
      try {
        await deleteRecordingMutation.mutateAsync(id);
        addNotification('録画を削除しました', 'success');
      } catch (error) {
        addNotification('録画削除に失敗しました', 'error');
      }
    }
  };

  const formatDuration = (duration?: number) => {
    if (!duration) return 'N/A';
    const minutes = Math.floor(duration / 60);
    const seconds = Math.floor(duration % 60);
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  };

  if (isLoading) {
    return (
      <Card>
        <CardHeader title="録画一覧" />
        <CardContent>
          <Typography>読み込み中...</Typography>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader title="録画一覧" />
      <CardContent>
        {!recordings || recordings.length === 0 ? (
          <Typography color="text.secondary">
            録画がありません
          </Typography>
        ) : (
          <List>
            {recordings.map((recording) => (
              <ListItem key={recording.id} divider>
                <ListItemText
                  primary={recording.filename}
                  secondary={
                    <Box>
                      <Typography variant="caption" component="div">
                        開始: {new Date(recording.start_time).toLocaleString()}
                      </Typography>
                      <Typography variant="caption" component="div">
                        時間: {formatDuration(recording.duration)}
                      </Typography>
                      <Chip
                        label={recording.status}
                        size="small"
                        color={recording.status === 'completed' ? 'success' : 'default'}
                        sx={{ mt: 0.5 }}
                      />
                    </Box>
                  }
                />
                <ListItemSecondaryAction>
                  <IconButton edge="end" aria-label="play" sx={{ mr: 1 }}>
                    <PlayArrow />
                  </IconButton>
                  <IconButton edge="end" aria-label="download" sx={{ mr: 1 }}>
                    <Download />
                  </IconButton>
                  <IconButton
                    edge="end"
                    aria-label="delete"
                    onClick={() => handleDelete(recording.id)}
                    disabled={deleteRecordingMutation.isPending}
                  >
                    <Delete />
                  </IconButton>
                </ListItemSecondaryAction>
              </ListItem>
            ))}
          </List>
        )}
      </CardContent>
    </Card>
  );
};