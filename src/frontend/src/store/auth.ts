import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';
import type { AuthState, User } from '@/types/auth';

interface AuthStore extends AuthState {
  login: (token: string, user: User) => void;
  logout: () => void;
}

export const useAuthStore = create<AuthStore>()(
  persist(
    (set) => ({
      isAuthenticated: false,
      user: null,
      token: null,
      login: (token: string, user: User) => {
        localStorage.setItem('egocentric_vision_token', token);
        set({ isAuthenticated: true, user, token });
      },
      logout: () => {
        localStorage.removeItem('egocentric_vision_token');
        set({ isAuthenticated: false, user: null, token: null });
      },
    }),
    {
      name: 'auth-storage',
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({ 
        isAuthenticated: state.isAuthenticated,
        user: state.user,
        token: state.token 
      }),
    }
  )
);