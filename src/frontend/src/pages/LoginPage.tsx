import React from 'react';
import { useForm } from 'react-hook-form';
import {
  Container,
  Card,
  CardContent,
  CardHeader,
  TextField,
  Button,
  Box,
  Typography,
  Alert,
} from '@mui/material';
import { useAuthStore } from '@/store';

interface LoginFormData {
  token: string;
}

export const LoginPage: React.FC = () => {
  const { register, handleSubmit, formState: { errors } } = useForm<LoginFormData>();
  const { login } = useAuthStore();
  const [error, setError] = React.useState<string>('');

  const onSubmit = async (data: LoginFormData) => {
    setError('');
    
    if (!data.token) {
      setError('APIトークンを入力してください');
      return;
    }

    try {
      // For now, just simulate a login with the token
      // In a real implementation, you would validate the token with the backend
      const mockUser = {
        id: '1',
        username: 'user',
        role: 'admin',
      };
      
      login(data.token, mockUser);
    } catch (err) {
      setError('ログインに失敗しました');
    }
  };

  return (
    <Container maxWidth="sm" sx={{ mt: 8 }}>
      <Card>
        <CardHeader 
          title="ログイン" 
          subheader="EgocentricVision Frontend"
          sx={{ textAlign: 'center' }}
        />
        <CardContent>
          {error && (
            <Alert severity="error" sx={{ mb: 2 }}>
              {error}
            </Alert>
          )}
          
          <Box component="form" onSubmit={handleSubmit(onSubmit)}>
            <TextField
              {...register('token', { required: 'APIトークンは必須です' })}
              label="APIトークン"
              type="password"
              fullWidth
              margin="normal"
              error={!!errors.token}
              helperText={errors.token?.message}
              placeholder="APIトークンを入力してください"
            />
            
            <Button
              type="submit"
              fullWidth
              variant="contained"
              sx={{ mt: 3, mb: 2 }}
            >
              ログイン
            </Button>
            
            <Typography variant="body2" color="text.secondary" sx={{ mt: 2 }}>
              ※ このフロントエンドは録画バックエンドサービスと連携します。
              有効なAPIトークンを入力してください。
            </Typography>
          </Box>
        </CardContent>
      </Card>
    </Container>
  );
};