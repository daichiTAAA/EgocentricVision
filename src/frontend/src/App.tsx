import React from 'react';
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import { CssBaseline } from '@mui/material';
import { QueryProvider } from '@/providers/QueryProvider';
import { NotificationContainer } from '@/components/common/NotificationContainer';
import { LoginPage, DashboardPage, RecordingsPage } from '@/pages';
import { useAuthStore, useUIStore } from '@/store';

const ProtectedRoute: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { isAuthenticated } = useAuthStore();
  return isAuthenticated ? <>{children}</> : <Navigate to="/login" replace />;
};

export const App: React.FC = () => {
  const { isAuthenticated } = useAuthStore();
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
            <Route
              path="/login"
              element={isAuthenticated ? <Navigate to="/" replace /> : <LoginPage />}
            />
            <Route
              path="/"
              element={
                <ProtectedRoute>
                  <DashboardPage />
                </ProtectedRoute>
              }
            />
            <Route
              path="/recordings"
              element={
                <ProtectedRoute>
                  <RecordingsPage />
                </ProtectedRoute>
              }
            />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </Router>
        <NotificationContainer />
      </QueryProvider>
    </ThemeProvider>
  );
};