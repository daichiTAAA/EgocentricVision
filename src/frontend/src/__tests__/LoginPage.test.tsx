import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import { LoginPage } from '@/pages/LoginPage';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
    mutations: { retry: false },
  },
});

const theme = createTheme();

const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <BrowserRouter>
    <ThemeProvider theme={theme}>
      <QueryClientProvider client={queryClient}>
        {children}
      </QueryClientProvider>
    </ThemeProvider>
  </BrowserRouter>
);

describe('LoginPage', () => {
  it('renders login form', () => {
    render(
      <TestWrapper>
        <LoginPage />
      </TestWrapper>
    );

    expect(screen.getByText('ログイン')).toBeInTheDocument();
    expect(screen.getByLabelText('APIトークン')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'ログイン' })).toBeInTheDocument();
  });
});