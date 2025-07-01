import React from "react";
import { Grid, Box, Typography, Alert, CircularProgress } from "@mui/material";
import { Layout } from "@/components/layout/Layout";
import { StreamPlayer, StreamConnectForm } from "@/features/streaming";
import { RecordingControls } from "@/features/recording";
import { useStreamStatus } from "@/hooks/useStreaming";
import type { StreamStatus } from "@/types/api";

export const DashboardPage: React.FC = () => {
  // 全ストリームの状態を取得
  const { data: streamStatusMap, error, isLoading } = useStreamStatus();

  // デバッグ用: 取得したストリーム状態を出力
  console.log("streamStatusMap:", streamStatusMap);

  // streamStatusMapがRecord<string, StreamStatus>型かどうかを判定
  const streamList =
    streamStatusMap &&
    typeof streamStatusMap === "object" &&
    !Array.isArray(streamStatusMap)
      ? Object.entries(streamStatusMap as Record<string, StreamStatus>)
      : [];

  return (
    <Layout>
      <Box sx={{ flexGrow: 1 }}>
        {/* ローディング表示 */}
        {isLoading && (
          <Box sx={{ my: 4, textAlign: "center" }}>
            <CircularProgress />
            <Typography>ストリーム状態を取得中...</Typography>
          </Box>
        )}
        {/* エラー表示 */}
        {error && (
          <Alert severity="error" sx={{ my: 2 }}>
            ストリーム状態の取得に失敗しました: {String(error)}
          </Alert>
        )}
        {/* streamStatusMapの値を明示的に表示 */}
        {/* <Box sx={{ my: 2 }}>
          <Typography variant="caption" color="text.secondary">
            streamStatusMap: {JSON.stringify(streamStatusMap)}
          </Typography>
        </Box> */}
        <Grid container spacing={3}>
          {streamList.length === 0 && !isLoading && !error && (
            <Grid item xs={12}>
              <Typography color="text.secondary">
                接続中のストリームはありません
              </Typography>
              {/* 新規ストリーム接続フォーム: stream_id未指定で表示 */}
              <Box sx={{ mt: 2 }}>
                <StreamConnectForm />
              </Box>
            </Grid>
          )}
          {streamList.map(([stream_id, status]) => (
            <React.Fragment key={stream_id}>
              <Grid item xs={12} md={6}>
                {status.is_connected && (
                  <>
                    {console.log(
                      "stream_id:",
                      stream_id,
                      "is_connected:",
                      status.is_connected,
                      "url:",
                      status.url
                    )}
                    <StreamPlayer stream_id={stream_id} rtspUrl={status.url} />
                  </>
                )}
              </Grid>
              <Grid item xs={12} md={6}>
                <RecordingControls stream_id={stream_id} />
                {/* ストリーム未接続時は接続フォームを表示 */}
                {!status.is_connected && (
                  <Box sx={{ mt: 2 }}>
                    <StreamConnectForm stream_id={stream_id} />
                  </Box>
                )}
              </Grid>
            </React.Fragment>
          ))}
        </Grid>
      </Box>
    </Layout>
  );
};
