import React from 'react';
import { Layout } from '@/components/layout/Layout';
import { RecordingsList } from '@/features/recording';

export const RecordingsPage: React.FC = () => {
  return (
    <Layout>
      <RecordingsList />
    </Layout>
  );
};