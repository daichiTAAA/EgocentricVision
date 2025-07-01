import React, { useEffect, useRef } from "react";
import { Card, CardContent, Typography } from "@mui/material";

interface StreamPlayerProps {
  stream_id: string;
  rtspUrl?: string;
}

export const StreamPlayer: React.FC<StreamPlayerProps> = ({
  stream_id,
  rtspUrl,
}) => {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    if (!videoRef.current || !stream_id) return;

    // recordサービスのWebRTCストリームURLを組み立て
    // 例: http://<recordサービスのホスト>:3000/api/v1/streams/<stream_id>/webrtc/start
    const recordHost = window.location.hostname; // 必要に応じて環境変数等で変更
    const webrtcUrl = `http://${recordHost}:3000/api/v1/streams/${stream_id}/webrtc`;

    console.log("WebRTC URL:", webrtcUrl);

    const pc = new RTCPeerConnection();
    pc.addTransceiver("video", { direction: "recvonly" });
    pc.addTransceiver("audio", { direction: "recvonly" });
    pc.createOffer()
      .then((offer) => pc.setLocalDescription(offer))
      .then(() => {
        return fetch(webrtcUrl, {
          method: "POST",
          headers: {
            "Content-Type": "application/sdp",
          },
          body: pc.localDescription?.sdp,
        });
      })
      .then((response) => {
        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }
        return response.text();
      })
      .then((answer) => {
        return pc.setRemoteDescription(
          new RTCSessionDescription({
            type: "answer",
            sdp: answer,
          })
        );
      })
      .catch((error) => {
        console.error("WebRTC error:", error);
      });
    pc.ontrack = (event) => {
      if (videoRef.current) {
        videoRef.current.srcObject = event.streams[0];
      }
    };
    return () => {
      pc.close();
    };
  }, [stream_id]);

  return (
    <Card sx={{ border: "2px solid red", mb: 2 }}>
      <CardContent>
        <video
          ref={videoRef}
          autoPlay
          controls
          style={{ width: "100%", background: "black" }}
        />
        <Typography
          variant="caption"
          color="text.secondary"
          sx={{ mt: 1, display: "block" }}
        >
          StreamPlayer: {stream_id} / {rtspUrl}
        </Typography>
      </CardContent>
    </Card>
  );
};
