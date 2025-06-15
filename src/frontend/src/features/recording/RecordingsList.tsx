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
import { recordingsApi } from '@/api';

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

  const handlePlay = async (id: string) => {
    try {
      const response = await recordingsApi.download(id);
      const blob = response.data;
      const url = window.URL.createObjectURL(blob);
      
      const dialog = document.createElement('dialog');
      dialog.style.width = '80%';
      dialog.style.maxWidth = '800px';
      dialog.style.padding = '20px';
      dialog.style.position = 'relative';
      dialog.style.border = 'none';
      dialog.style.borderRadius = '12px';
      dialog.style.background = '#fff';
      dialog.style.boxShadow = '0 4px 24px rgba(0,0,0,0.2)';

      const video = document.createElement('video');
      video.src = url;
      video.controls = true;
      video.style.width = '100%';
      video.style.maxHeight = '80vh';

      const closeButton = document.createElement('button');
      closeButton.innerHTML = '×';
      closeButton.setAttribute('aria-label', '閉じる');
      closeButton.style.position = 'absolute';
      closeButton.style.right = '16px';
      closeButton.style.top = '16px';
      closeButton.style.width = '40px';
      closeButton.style.height = '40px';
      closeButton.style.background = '#fff';
      closeButton.style.border = '2px solid #888';
      closeButton.style.borderRadius = '50%';
      closeButton.style.fontSize = '28px';
      closeButton.style.fontWeight = 'bold';
      closeButton.style.cursor = 'pointer';
      closeButton.style.color = '#333';
      closeButton.style.display = 'flex';
      closeButton.style.alignItems = 'center';
      closeButton.style.justifyContent = 'center';
      closeButton.style.boxShadow = '0 2px 8px rgba(0,0,0,0.08)';
      closeButton.style.zIndex = '1000';

      const closeDialog = () => {
        dialog.close();
        window.URL.revokeObjectURL(url);
        document.body.removeChild(dialog);
      };

      closeButton.onclick = (e) => {
        e.preventDefault();
        e.stopPropagation();
        closeDialog();
      };

      closeButton.onmouseenter = () => {
        closeButton.style.background = '#f44336';
        closeButton.style.color = '#fff';
        closeButton.style.borderColor = '#f44336';
      };

      closeButton.onmouseleave = () => {
        closeButton.style.background = '#fff';
        closeButton.style.color = '#333';
        closeButton.style.borderColor = '#888';
      };

      dialog.appendChild(closeButton);
      dialog.appendChild(video);
      document.body.appendChild(dialog);
      dialog.showModal();

      dialog.addEventListener('click', (e) => {
        if (e.target === dialog) {
          closeDialog();
        }
      });
    } catch (error) {
      addNotification('録画の再生に失敗しました', 'error');
    }
  };

  const handleDownload = async (id: string, filename: string) => {
    try {
      const response = await recordingsApi.download(id);
      const blob = response.data;
      const url = window.URL.createObjectURL(blob);
      
      // filenameがundefinedの場合はデフォルトのファイル名を使用
      const safeFilename = filename && filename.endsWith('.mp4') 
        ? filename 
        : `${filename || 'recording'}.mp4`;
      
      const link = document.createElement('a');
      link.href = url;
      link.download = safeFilename;
      link.style.display = 'none';
      document.body.appendChild(link);
      
      // クリックイベントを発火
      link.click();
      
      // クリーンアップ
      setTimeout(() => {
        document.body.removeChild(link);
        window.URL.revokeObjectURL(url);
      }, 100);

      addNotification('録画のダウンロードを開始しました', 'success');
    } catch (error) {
      console.error('Download error:', error);
      addNotification('録画のダウンロードに失敗しました', 'error');
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
                        時間: {formatDuration(recording.duration_seconds)}
                      </Typography>
                      <Chip
                        label={recording.status}
                        size="small"
                        color={recording.status === 'COMPLETED' ? 'success' : 'default'}
                        sx={{ mt: 0.5 }}
                      />
                    </Box>
                  }
                />
                <ListItemSecondaryAction>
                  <IconButton 
                    edge="end" 
                    aria-label="play" 
                    sx={{ mr: 1 }}
                    onClick={() => handlePlay(recording.id)}
                  >
                    <PlayArrow />
                  </IconButton>
                  <IconButton 
                    edge="end" 
                    aria-label="download" 
                    sx={{ mr: 1 }}
                    onClick={() => handleDownload(recording.id, recording.filename)}
                  >
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