import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import MatchViewerPage from './MatchViewerPage';
import * as AuthContextModule from '../../api/AuthContext';
import * as replayModule from './useMatchReplay';
import type { MatchStartPayload } from './useMatchReplay';
import { authState } from '../../test/helpers/mockAuth';

vi.mock('../../api/AuthContext');
vi.mock('./useMatchReplay');

const mockUseAuth = vi.mocked(AuthContextModule.useAuth);
const mockUseMatchReplay = vi.mocked(replayModule.useMatchReplay);

const payload: MatchStartPayload = {
  match_id: 'abcdef12-3456-7890-abcd-ef1234567890',
  red_username: 'vale',
  blue_username: 'orion',
  red_warrior_source: 'MOV 0, 1',
  blue_warrior_source: 'MOV 0, 1',
  core_size: 8000,
  max_steps: 80000,
  red_start: 0,
  blue_start: 4000,
  steps_taken: 1234,
  result: 'red_win',
  playback_start_time_ms: 0,
  steps_per_sec: 5000,
};

function replayState(overrides: Partial<ReturnType<typeof replayModule.useMatchReplay>> = {}) {
  return {
    ready: true,
    currentStep: 1234,
    totalSteps: 1234,
    warriors: [
      { name: 'Imp', alive: true, procs: 1 },
      { name: 'Dwarf', alive: false, procs: 0 },
    ],
    finished: false,
    parseError: null,
    gridRef: { current: null },
    ...overrides,
  };
}

// Renders where the viewer navigated to, so tests can assert on redirects.
function LobbyProbe() {
  const location = useLocation();
  const state = location.state as { autoQueue?: boolean } | null;
  return <p>lobby{state?.autoQueue ? ' (auto-queue)' : ''}</p>;
}

function renderViewer(state: { matchStart: MatchStartPayload } | null) {
  return render(
    <MemoryRouter initialEntries={[{ pathname: `/match/${payload.match_id}`, state }]}>
      <Routes>
        <Route path="/match/:matchId" element={<MatchViewerPage />} />
        <Route path="/lobby" element={<LobbyProbe />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe('MatchViewerPage', () => {
  beforeEach(() => {
    mockUseAuth.mockReturnValue(authState({ user: { user_id: 'u1', username: 'vale' } }));
    mockUseMatchReplay.mockReturnValue(replayState());
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it('bounces to the lobby when opened without a match payload', () => {
    renderViewer(null);
    expect(screen.getByText('lobby')).toBeInTheDocument();
  });

  it('shows the match header, step counter and warrior status', () => {
    renderViewer({ matchStart: payload });
    expect(screen.getByRole('heading', { name: 'Match abcdef12' })).toBeInTheDocument();
    expect(screen.getByText('step 1,234 / 1,234')).toBeInTheDocument();
    expect(screen.getByText('Imp — 1 proc')).toBeInTheDocument();
    expect(screen.getByText('Dwarf — dead')).toBeInTheDocument();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('covers the grid with a loading overlay until the engine is ready', () => {
    mockUseMatchReplay.mockReturnValue(replayState({ ready: false }));
    renderViewer({ matchStart: payload });
    expect(screen.getByText('Loading engine…')).toBeInTheDocument();
  });

  it('shows an engine failure instead of the loading overlay', () => {
    mockUseMatchReplay.mockReturnValue(
      replayState({ ready: false, parseError: 'Engine init failed: bad' }),
    );
    renderViewer({ matchStart: payload });
    expect(screen.getByRole('alert')).toHaveTextContent('Engine init failed: bad');
    expect(screen.queryByText('Loading engine…')).not.toBeInTheDocument();
  });

  it('announces the result once the replay finishes', () => {
    mockUseMatchReplay.mockReturnValue(replayState({ finished: true }));
    renderViewer({ matchStart: payload });
    expect(screen.getByRole('status')).toHaveTextContent('Victory!');
    expect(screen.getByRole('status')).toHaveTextContent('1,234 steps');
  });

  it('Play Again returns to the lobby and re-queues', async () => {
    mockUseMatchReplay.mockReturnValue(replayState({ finished: true }));
    renderViewer({ matchStart: payload });
    await userEvent.click(screen.getByRole('button', { name: 'Play Again' }));
    expect(screen.getByText('lobby (auto-queue)')).toBeInTheDocument();
  });

  it('Back to Lobby returns without re-queueing', async () => {
    mockUseMatchReplay.mockReturnValue(replayState({ finished: true }));
    renderViewer({ matchStart: payload });
    await userEvent.click(screen.getByRole('button', { name: 'Back to Lobby' }));
    expect(screen.getByText('lobby')).toBeInTheDocument();
  });
});
