import React, { Suspense } from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import './index.css';
import AppLayout from './AppLayout';
import HomePage from './pages/HomePage';

// eslint-disable-next-line react-refresh/only-export-components
const BattlefieldPage = React.lazy(() => import('./pages/battlefield/BattlefieldPage'));
// eslint-disable-next-line react-refresh/only-export-components
const BuilderPage = React.lazy(() => import('./pages/builder/BuilderPage'));
// eslint-disable-next-line react-refresh/only-export-components
const LearnPage = React.lazy(() => import('./pages/LearnPage'));

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
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
        </Route>
      </Routes>
    </BrowserRouter>
  </React.StrictMode>,
);
