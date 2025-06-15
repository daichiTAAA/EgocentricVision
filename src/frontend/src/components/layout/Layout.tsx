import React from 'react';
import { Link, useLocation } from 'react-router-dom';
import {
  AppBar,
  Toolbar,
  Typography,
  Container,
  Box,
  IconButton,
  Button,
  Tabs,
  Tab,
} from '@mui/material';
import { Menu as MenuIcon, Logout } from '@mui/icons-material';
import { useAuthStore, useUIStore } from '@/store';

interface LayoutProps {
  children: React.ReactNode;
}

export const Layout: React.FC<LayoutProps> = ({ children }) => {
  const { logout, user } = useAuthStore();
  const { toggleSidebar } = useUIStore();
  const location = useLocation();

  const handleLogout = () => {
    logout();
  };

  const getCurrentTab = () => {
    if (location.pathname === '/') return 0;
    if (location.pathname === '/recordings') return 1;
    return 0;
  };

  return (
    <Box sx={{ flexGrow: 1 }}>
      <AppBar position="static">
        <Toolbar>
          <IconButton
            size="large"
            edge="start"
            color="inherit"
            aria-label="menu"
            sx={{ mr: 2 }}
            onClick={toggleSidebar}
          >
            <MenuIcon />
          </IconButton>
          <Typography variant="h6" component="div" sx={{ flexGrow: 1 }}>
            EgocentricVision
          </Typography>
          {user && (
            <Typography variant="body2" sx={{ mr: 2 }}>
              {user.username}
            </Typography>
          )}
          <Button color="inherit" onClick={handleLogout} startIcon={<Logout />}>
            ログアウト
          </Button>
        </Toolbar>
        <Box sx={{ borderBottom: 1, borderColor: 'divider' }}>
          <Tabs value={getCurrentTab()} textColor="inherit" indicatorColor="secondary">
            <Tab label="ダッシュボード" component={Link} to="/" />
            <Tab label="録画一覧" component={Link} to="/recordings" />
          </Tabs>
        </Box>
      </AppBar>
      <Container maxWidth="xl" sx={{ mt: 2, mb: 2 }}>
        {children}
      </Container>
    </Box>
  );
};