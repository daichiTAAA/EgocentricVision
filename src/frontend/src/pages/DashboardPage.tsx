import React from 'react';
import { Grid, Box } from '@mui/material';
import { Layout } from '@/components/layout/Layout';
import { StreamPlayer, StreamConnectForm } from '@/features/streaming';
import { RecordingControls } from '@/features/recording';
import { useStreamStatus } from '@/hooks/useStreaming';

export const DashboardPage: React.FC = () => {
  const { data: streamStatus } = useStreamStatus();
  return (
    <Layout>
      <Box sx={{ flexGrow: 1 }}>
        <Grid container spacing={3}>
          {/* Stream Player - Top Row */}
          <Grid item xs={12}>
            <StreamPlayer rtspUrl={streamStatus?.is_connected ? streamStatus.url : undefined} />
          </Grid>
          
          {/* Controls Row */}
          <Grid item xs={12} md={6}>
            <StreamConnectForm />
          </Grid>
          
          <Grid item xs={12} md={6}>
            <RecordingControls />
          </Grid>
        </Grid>
      </Box>
    </Layout>
  );
};