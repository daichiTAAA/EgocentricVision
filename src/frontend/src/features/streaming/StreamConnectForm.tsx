import React, { useCallback } from "react";
import { useForm } from "react-hook-form";
import {
  Card,
  CardContent,
  CardHeader,
  TextField,
  Button,
  Box,
  Alert,
} from "@mui/material";
import {
  useStreamConnect,
  useStreamDisconnect,
  useStreamStatus,
} from "@/hooks/useStreaming";
import { useUIStore } from "@/store";
import { useQueryClient } from "@tanstack/react-query";

interface StreamConnectFormData {
  url: string;
  username?: string;
  password?: string;
}

interface StreamConnectFormProps {
  stream_id?: string; // optional
}

export const StreamConnectForm: React.FC<StreamConnectFormProps> = ({
  stream_id,
}) => {
  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<StreamConnectFormData>({});
  const { data: streamStatusRaw } = useStreamStatus(stream_id);
  const connectMutation = useStreamConnect();
  const disconnectMutation = useStreamDisconnect(stream_id || "");
  const { addNotification } = useUIStore();
  const queryClient = useQueryClient();

  // streamStatusの型をStreamStatus | undefinedに限定
  const streamStatus =
    streamStatusRaw &&
    typeof streamStatusRaw === "object" &&
    !Array.isArray(streamStatusRaw) &&
    "is_connected" in streamStatusRaw
      ? (streamStatusRaw as import("@/types/api").StreamStatus)
      : undefined;

  // onSubmitでstream_id生成・リクエストボディへの追加を削除
  const onSubmit = useCallback(
    async (data: StreamConnectFormData) => {
      try {
        await connectMutation.mutateAsync({
          ...data,
          protocol: "rtsp",
        });
        addNotification("ストリームに接続しました", "success");
        // 状態を即時反映
        queryClient.invalidateQueries({ queryKey: ["stream", "status"] });
        // --- WebRTC配信開始API呼び出し ---
        // stream_idの取得方法を修正
        let newStreamId: string | undefined = undefined;
        if (streamStatusRaw && typeof streamStatusRaw === "object") {
          if ("stream_id" in streamStatusRaw) {
            newStreamId = (streamStatusRaw as any).stream_id;
          } else {
            const keys = Object.keys(streamStatusRaw);
            if (keys.length > 0) newStreamId = keys[0];
          }
        }
        if (newStreamId) {
          const signaling_url = window.location.origin + "/webrtc-signal";
          await fetch(
            `/api/v1/streams/${newStreamId}/webrtc/start?signaling_url=${encodeURIComponent(
              signaling_url
            )}`,
            {
              method: "POST",
            }
          );
        }
        // ---
      } catch (error: any) {
        const errorMessage =
          error.response?.data?.error || "ストリーム接続に失敗しました";
        addNotification(errorMessage, "error");
      }
    },
    [connectMutation, addNotification, queryClient]
  );

  const handleDisconnect = async () => {
    try {
      await disconnectMutation.mutateAsync();
      addNotification("ストリームを切断しました", "success");
      // 状態を即時反映
      queryClient.invalidateQueries({ queryKey: ["stream", "status"] });
    } catch (error: any) {
      const errorMessage =
        error.response?.data?.error || "ストリーム切断に失敗しました";
      addNotification(errorMessage, "error");
    }
  };

  return (
    <Card>
      <CardHeader title={`ストリーム接続`} />
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
          {/* stream_idフィールドは表示しない */}
          <TextField
            {...register("url", {
              required: "URLは必須です",
              pattern: {
                value: /^rtsp:\/\/.+/, // RTSP URLのみ
                message:
                  "RTSP URLを入力してください（例: rtsp://example.com/stream）",
              },
            })}
            label="ストリームURL"
            fullWidth
            margin="normal"
            error={!!errors.url}
            helperText={errors.url?.message}
            placeholder="rtsp://example.com/stream"
            disabled={!!streamStatus?.is_connected}
          />
          <TextField
            {...register("username")}
            label="ユーザー名（任意）"
            fullWidth
            margin="normal"
            disabled={!!streamStatus?.is_connected}
          />
          <TextField
            {...register("password")}
            label="パスワード（任意）"
            type="password"
            fullWidth
            margin="normal"
            disabled={!!streamStatus?.is_connected}
          />
          <Box sx={{ mt: 2, display: "flex", gap: 2 }}>
            {!streamStatus?.is_connected ? (
              <Button
                type="submit"
                variant="contained"
                disabled={!!connectMutation.isPending}
              >
                {connectMutation.isPending ? "接続中..." : "接続"}
              </Button>
            ) : (
              <Button
                variant="outlined"
                color="error"
                onClick={handleDisconnect}
                disabled={!!disconnectMutation.isPending}
              >
                {disconnectMutation.isPending ? "切断中..." : "切断"}
              </Button>
            )}
          </Box>
        </Box>
      </CardContent>
    </Card>
  );
};
