import React from 'react';
import {
  Card,
  CardContent,
  CardHeader,
  Box,
  Typography,
} from '@mui/material';

export const StreamPlayer: React.FC = () => {
  return (
    <Card>
      <CardHeader title="ライブストリーム" />
      <CardContent>
        <Box
          sx={{
            width: '100%',
            height: 400,
            backgroundColor: '#000',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            borderRadius: 1,
          }}
        >
          <Typography color="white" variant="h6">
            ストリーム表示エリア
          </Typography>
        </Box>
        <Typography variant="caption" sx={{ mt: 1, display: 'block' }}>
          ※ 動画ストリーミング機能は今後実装予定
        </Typography>
      </CardContent>
    </Card>
  );
};