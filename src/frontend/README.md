# EgocentricVision Frontend

React-based frontend application for the EgocentricVision project that provides a web interface for video stream management, recording control, and video playback.

## Features

- **Stream Connection Management**: Connect/disconnect from RTSP/WebRTC streams
- **Real-time Video Display**: Live stream viewing (placeholder implementation)
- **Recording Controls**: Start/stop recording functionality
- **Recording Management**: List, play, download, and delete saved recordings
- **Authentication**: Token-based authentication
- **Responsive Design**: Works on desktop, tablet, and mobile devices

## Technology Stack

- **Framework**: React 18 with TypeScript
- **Build Tool**: Vite
- **UI Components**: Material-UI (MUI)
- **State Management**: 
  - TanStack Query (server state)
  - Zustand (client state)
- **Routing**: React Router
- **Forms**: React Hook Form
- **HTTP Client**: Axios

## Development

### Prerequisites

- Node.js 18+
- npm

### Installation

```bash
npm install
```

### Development Server

```bash
npm run dev
```

The app will be available at `http://localhost:5173`

### Build

```bash
npm run build
```

### Docker

Build and run with Docker:

```bash
docker build -t egocentric-vision-frontend .
docker run -p 8080:80 egocentric-vision-frontend
```

Or use docker-compose from the project root:

```bash
docker-compose up frontend
```

## Configuration

Environment variables:

- `VITE_API_BASE_URL`: Backend API base URL (default: `http://localhost:3000`)

## Project Structure

```
src/
├── api/              # API client functions
├── components/       # Reusable UI components
├── config/           # Configuration files
├── features/         # Feature-specific components
├── hooks/            # Custom React hooks
├── lib/              # External library configurations
├── pages/            # Page components
├── providers/        # React context providers
├── store/            # State management
├── styles/           # Global styles
└── types/            # TypeScript type definitions
```

## Architecture

The frontend follows a modern React architecture with:

- **Component-based design**: Reusable UI components
- **Feature-driven structure**: Organized by functionality
- **Separation of concerns**: Clear separation between UI, logic, and data
- **Type safety**: Full TypeScript coverage
- **State management**: Server state with TanStack Query, client state with Zustand

## API Integration

The frontend communicates with the record backend service through REST APIs:

- Stream management: `/api/v1/streams/*`
- Recording management: `/api/v1/recordings/*`
- Health checks: `/health`

## Future Enhancements

- WebRTC/HLS video streaming implementation
- Real-time notifications via WebSocket
- Advanced authentication (OAuth, JWT)
- User management interface
- Video playback controls and seeking
- Stream quality settings
- Multi-camera support