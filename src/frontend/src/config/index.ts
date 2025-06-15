export const config = {
  api: {
    baseURL: import.meta.env.VITE_API_BASE_URL || 'http://localhost:3000',
  },
  auth: {
    tokenKey: 'egocentric_vision_token',
  },
} as const;