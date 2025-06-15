import React from 'react';
import { BrowserRouter as Router, Routes, Route } from 'react-router-dom';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import { CssBaseline } from '@mui/material';
import { QueryProvider } from '@/providers/QueryProvider';
import { NotificationContainer } from '@/components/common/NotificationContainer';
import { DashboardPage, RecordingsPage } from '@/pages';
import { useUIStore } from '@/store';

export const App: React.FC = () => {
  const { theme } = useUIStore();

  const muiTheme = createTheme({
    palette: {
      mode: theme,
      primary: {
        main: '#1976d2',
      },
    },
  });

  return (
    <ThemeProvider theme={muiTheme}>
      <CssBaseline />
      <QueryProvider>
        <Router>
          <Routes>
            <Route path="/" element={<DashboardPage />} />
            <Route path="/recordings" element={<RecordingsPage />} />
            <Route path="*" element={<DashboardPage />} />
          </Routes>
        </Router>
        <NotificationContainer />
      </QueryProvider>
    </ThemeProvider>
  );
};