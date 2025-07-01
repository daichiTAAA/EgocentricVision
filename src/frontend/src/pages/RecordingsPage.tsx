import React from "react";
import { Layout } from "@/components/layout/Layout";
import { RecordingsList } from "@/features/recording";
import { useStreamStatus } from "@/hooks/useStreaming";

export const RecordingsPage: React.FC = () => {
  // 全ストリームの状態を取得
  const { data: streamStatusMap } = useStreamStatus();
  const streamList =
    streamStatusMap && typeof streamStatusMap === "object"
      ? Object.entries(streamStatusMap)
      : [];

  return (
    <Layout>
      {streamList.length === 0 ? (
        <div>録画ストリームがありません</div>
      ) : (
        streamList.map(([stream_id]) => (
          <RecordingsList key={stream_id} stream_id={stream_id} />
        ))
      )}
    </Layout>
  );
};
