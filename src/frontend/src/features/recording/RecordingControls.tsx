import React, { useState } from "react";
import {
  Card,
  CardContent,
  CardHeader,
  Button,
  Box,
  Typography,
  Chip,
  TextField,
  Grid,
} from "@mui/material";
import { PlayArrow, Stop } from "@mui/icons-material";
import { useStartRecording, useStopRecording } from "@/hooks/useRecording";
import { useStreamStatus } from "@/hooks/useStreaming";
import { useUIStore } from "@/store";

interface RecordingControlsProps {
  stream_id: string;
}

export const RecordingControls: React.FC<RecordingControlsProps> = ({
  stream_id,
}) => {
  const { data: streamStatus } = useStreamStatus(stream_id);
  const startRecordingMutation = useStartRecording(stream_id);
  const stopRecordingMutation = useStopRecording(stream_id);
  const { addNotification } = useUIStore();
  const [filename, setFilename] = useState("");

  const isRecording = streamStatus?.is_recording || false;
  const canRecord = streamStatus?.is_connected && !isRecording;

  const handleStartRecording = async () => {
    try {
      await startRecordingMutation.mutateAsync({
        filename: filename || undefined,
      });
      setFilename("");
      addNotification("録画を開始しました", "success");
    } catch (error) {
      addNotification("録画開始に失敗しました", "error");
    }
  };

  const handleStopRecording = async () => {
    try {
      await stopRecordingMutation.mutateAsync();
      addNotification("録画を停止しました", "success");
    } catch (error) {
      addNotification("録画停止に失敗しました", "error");
    }
  };

  return (
    <Card>
      <CardHeader title={`録画制御 (${stream_id})`} />
      <CardContent>
        <Grid container spacing={2}>
          <Grid item xs={12}>
            <Box sx={{ display: "flex", alignItems: "center", gap: 2, mb: 2 }}>
              <Typography variant="body2">ステータス:</Typography>
              {isRecording ? (
                <Chip label="録画中" color="error" variant="filled" />
              ) : (
                <Chip label="停止中" color="default" variant="outlined" />
              )}
            </Box>
          </Grid>

          <Grid item xs={12}>
            {!canRecord && (
              <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
                録画するにはストリームに接続してください
              </Typography>
            )}
          </Grid>

          <Grid item xs={12}>
            <TextField
              fullWidth
              label="ファイル名（オプション）"
              value={filename}
              onChange={(e) => setFilename(e.target.value)}
              disabled={!canRecord || isRecording}
              placeholder="例: recording_20240315"
              helperText="指定しない場合は自動生成されます"
              size="small"
            />
          </Grid>

          <Grid item xs={12}>
            <Box sx={{ display: "flex", gap: 2 }}>
              {!isRecording ? (
                <Button
                  variant="contained"
                  startIcon={<PlayArrow />}
                  onClick={handleStartRecording}
                  disabled={!canRecord || startRecordingMutation.isPending}
                >
                  {startRecordingMutation.isPending ? "開始中..." : "録画開始"}
                </Button>
              ) : (
                <Button
                  variant="outlined"
                  color="error"
                  startIcon={<Stop />}
                  onClick={handleStopRecording}
                  disabled={stopRecordingMutation.isPending}
                >
                  {stopRecordingMutation.isPending ? "停止中..." : "録画停止"}
                </Button>
              )}
            </Box>
          </Grid>
        </Grid>
      </CardContent>
    </Card>
  );
};
