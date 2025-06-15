export interface User {
  id: string;
  username: string;
  role: string;
}

export interface AuthState {
  isAuthenticated: boolean;
  user: User | null;
  token: string | null;
}