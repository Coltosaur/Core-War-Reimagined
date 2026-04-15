import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import './index.css';
import AppLayout from './AppLayout';
import HomePage from './pages/HomePage';
import BattlefieldPage from './pages/BattlefieldPage';
import BuilderPage from './pages/BuilderPage';
import LearnPage from './pages/LearnPage';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <BrowserRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route path="/" element={<HomePage />} />
          <Route path="/battle" element={<BattlefieldPage />} />
          <Route path="/builder" element={<BuilderPage />} />
          <Route path="/learn" element={<LearnPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  </React.StrictMode>,
);
