import React, { Suspense } from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import './index.css';
import AppLayout from './AppLayout';
import { AuthProvider } from './api/AuthContext';
import HomePage from './pages/HomePage';

// eslint-disable-next-line react-refresh/only-export-components
const BattlefieldPage = React.lazy(() => import('./pages/battlefield/BattlefieldPage'));
// eslint-disable-next-line react-refresh/only-export-components
const BuilderPage = React.lazy(() => import('./pages/builder/BuilderPage'));
// eslint-disable-next-line react-refresh/only-export-components
const LearnPage = React.lazy(() => import('./pages/LearnPage'));
// eslint-disable-next-line react-refresh/only-export-components
const LeaderboardPage = React.lazy(() => import('./pages/LeaderboardPage'));
// eslint-disable-next-line react-refresh/only-export-components
const ProfilePage = React.lazy(() => import('./pages/profile/ProfilePage'));
// eslint-disable-next-line react-refresh/only-export-components
const LobbyPage = React.lazy(() => import('./pages/LobbyPage'));
// eslint-disable-next-line react-refresh/only-export-components
const MatchViewerPage = React.lazy(() => import('./pages/match/MatchViewerPage'));

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AuthProvider>
      <BrowserRouter>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/" element={<HomePage />} />
            <Route
              path="/battle"
              element={
                <Suspense>
                  <BattlefieldPage />
                </Suspense>
              }
            />
            <Route
              path="/builder"
              element={
                <Suspense>
                  <BuilderPage />
                </Suspense>
              }
            />
            <Route
              path="/learn"
              element={
                <Suspense>
                  <LearnPage />
                </Suspense>
              }
            />
            <Route
              path="/leaderboard"
              element={
                <Suspense>
                  <LeaderboardPage />
                </Suspense>
              }
            />
            <Route
              path="/profile"
              element={
                <Suspense>
                  <ProfilePage />
                </Suspense>
              }
            />
            <Route
              path="/users/:username"
              element={
                <Suspense>
                  <ProfilePage />
                </Suspense>
              }
            />
            <Route
              path="/lobby"
              element={
                <Suspense>
                  <LobbyPage />
                </Suspense>
              }
            />
            <Route
              path="/match/:matchId"
              element={
                <Suspense>
                  <MatchViewerPage />
                </Suspense>
              }
            />
          </Route>
        </Routes>
      </BrowserRouter>
    </AuthProvider>
  </React.StrictMode>,
);
