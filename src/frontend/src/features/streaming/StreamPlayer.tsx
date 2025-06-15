import React from 'react';
import { Card, CardContent, CardHeader, Typography } from '@mui/material';
import ReactPlayer from 'react-player';

interface StreamPlayerProps {
  rtspUrl?: string;
}

export const StreamPlayer: React.FC<StreamPlayerProps> = ({ rtspUrl }) => {
  const [webrtcUrl, setWebrtcUrl] = React.useState<string | undefined>(undefined);

  React.useEffect(() => {
    if (rtspUrl) {
      // RTSP URLをWebRTC URLに変換
      const webrtcUrl = rtspUrl.replace('rtsp://', 'http://').replace('8554', '8889');
      setWebrtcUrl(webrtcUrl);
    } else {
      setWebrtcUrl(undefined);
    }
  }, [rtspUrl]);

  return (
    <Card>
      <CardHeader title="ストリームプレーヤー" />
      <CardContent>
        {webrtcUrl ? (
          <div style={{ position: 'relative', paddingTop: '56.25%' }}>
            <ReactPlayer
              url={webrtcUrl}
              width="100%"
              height="100%"
              style={{ position: 'absolute', top: 0, left: 0 }}
              playing
              controls
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