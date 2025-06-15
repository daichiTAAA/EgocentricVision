import React, { useEffect, useRef } from 'react';
import { Card, CardContent, CardHeader, Typography } from '@mui/material';

interface StreamPlayerProps {
  rtspUrl?: string;
}

export const StreamPlayer: React.FC<StreamPlayerProps> = ({ rtspUrl }) => {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    if (!videoRef.current || !rtspUrl) return;

    // RTSP URLをWHEP URLに変換
    const url = new URL(rtspUrl);
    const webrtcUrl = `http://${url.hostname}:8889${url.pathname}/whep`;

    console.log('Original RTSP URL:', rtspUrl);
    console.log('Converted WHEP URL:', webrtcUrl);

    const pc = new RTCPeerConnection();

    // メディアストリームを設定
    pc.addTransceiver('video', { direction: 'recvonly' });
    pc.addTransceiver('audio', { direction: 'recvonly' });

    // オファーを作成
    pc.createOffer()
      .then(offer => pc.setLocalDescription(offer))
      .then(() => {
        // WHEPエンドポイントにオファーを送信
        return fetch(webrtcUrl, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/sdp'
          },
          body: pc.localDescription?.sdp
        });
      })
      .then(response => {
        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }
        return response.text();
      })
      .then(answer => {
        // アンサーを設定
        return pc.setRemoteDescription(new RTCSessionDescription({
          type: 'answer',
          sdp: answer
        }));
      })
      .catch(error => {
        console.error('WebRTC error:', error);
      });

    // ストリームを受信したらビデオ要素に設定
    pc.ontrack = (event) => {
      if (videoRef.current) {
        videoRef.current.srcObject = event.streams[0];
      }
    };

    return () => {
      pc.close();
    };
  }, [rtspUrl]);

  return (
    <Card>
      <CardHeader title="ストリームプレーヤー" />
      <CardContent>
        {rtspUrl ? (
          <div style={{ position: 'relative', paddingTop: '56.25%' }}>
            <video
              ref={videoRef}
              autoPlay
              playsInline
              controls
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                height: '100%'
              }}
            />
          </div>
        ) : (
          <Typography variant="body1" color="text.secondary">
            ストリームを接続するとプレーヤーが表示されます
          </Typography>
        )}
      </CardContent>
    </Card>
  );
};