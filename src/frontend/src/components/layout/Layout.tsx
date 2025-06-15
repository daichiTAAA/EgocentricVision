import React from 'react';
import { Link, useLocation } from 'react-router-dom';
import {
  AppBar,
  Toolbar,
  Typography,
  Container,
  Box,
  IconButton,
  Tabs,
  Tab,
} from '@mui/material';
import { Menu as MenuIcon } from '@mui/icons-material';
import { useUIStore } from '@/store';

interface LayoutProps {
  children: React.ReactNode;
}

export const Layout: React.FC<LayoutProps> = ({ children }) => {
  const location = useLocation();
  const { toggleSidebar } = useUIStore();

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', minHeight: '100vh' }}>
      <AppBar position="static">
        <Toolbar>
          <IconButton
            edge="start"
            color="inherit"
            onClick={toggleSidebar}
            sx={{ mr: 2 }}
          >
            <MenuIcon />
          </IconButton>
          <Typography variant="h6" component="div" sx={{ flexGrow: 1 }}>
            EgocentricVision
          </Typography>
        </Toolbar>
        <Tabs
          value={location.pathname}
          sx={{ borderBottom: 1, borderColor: 'divider' }}
        >
          <Tab
            label="ダッシュボード"
            value="/"
            component={Link}
            to="/"
          />
          <Tab
            label="録画一覧"
            value="/recordings"
            component={Link}
            to="/recordings"
          />
        </Tabs>
      </AppBar>
      <Container component="main" sx={{ flexGrow: 1, py: 3 }}>
        {children}
      </Container>
    </Box>
  );
};