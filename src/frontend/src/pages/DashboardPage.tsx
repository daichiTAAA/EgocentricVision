import React from 'react';
import { Grid, Box } from '@mui/material';
import { Layout } from '@/components/layout/Layout';
import { StreamPlayer, StreamConnectForm } from '@/features/streaming';
import { RecordingControls } from '@/features/recording';

export const DashboardPage: React.FC = () => {
  return (
    <Layout>
      <Box sx={{ flexGrow: 1 }}>
        <Grid container spacing={3}>
          {/* Stream Player - Top Row */}
          <Grid item xs={12}>
            <StreamPlayer />
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