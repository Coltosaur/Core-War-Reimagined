import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import LeaderboardPage from './LeaderboardPage';
import * as leaderboardApi from '../api/leaderboard';

vi.mock('../api/leaderboard');
const mockGetLeaderboard = vi.mocked(leaderboardApi.getLeaderboard);

function entry(rank: number, username: string, rating: number): leaderboardApi.LeaderboardEntry {
  return { rank, user_id: `u${rank}`, username, rating, created_at: '2026-10-01T00:00:00Z' };
}

function renderPage() {
  render(<LeaderboardPage />, { wrapper: MemoryRouter });
}

afterEach(() => {
  vi.clearAllMocks();
});

describe('LeaderboardPage', () => {
  it('lists players with medals for the top three and links to their profiles', async () => {
    mockGetLeaderboard.mockResolvedValue({
      entries: [entry(1, 'vale', 1320), entry(2, 'nova', 1250), entry(4, 'orion', 1100)],
      total: 3,
      page: 1,
      per_page: 50,
    });
    renderPage();

    expect(await screen.findByRole('link', { name: 'vale' })).toHaveAttribute(
      'href',
      '/users/vale',
    );
    expect(screen.getByText('\u{1F947}')).toBeInTheDocument();
    expect(screen.getByText('#4')).toBeInTheDocument();
    expect(screen.getByText('1320')).toBeInTheDocument();
    // One page of results: no pager.
    expect(screen.queryByRole('button', { name: 'Next' })).not.toBeInTheDocument();
  });

  it('pages forward and disables Prev on the first page', async () => {
    mockGetLeaderboard.mockImplementation(async (page = 1) => ({
      entries: [entry(page, `player${page}`, 1000)],
      total: 2,
      page,
      per_page: 1,
    }));
    const user = userEvent.setup();
    renderPage();

    expect(await screen.findByText('1 / 2')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Prev' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Next' }));
    expect(await screen.findByText('2 / 2')).toBeInTheDocument();
    expect(mockGetLeaderboard).toHaveBeenLastCalledWith(2);
    expect(screen.getByRole('button', { name: 'Next' })).toBeDisabled();
  });

  it('shows the empty state', async () => {
    mockGetLeaderboard.mockResolvedValue({ entries: [], total: 0, page: 1, per_page: 50 });
    renderPage();
    expect(await screen.findByText('No players yet.')).toBeInTheDocument();
  });

  it('shows a load failure as an alert', async () => {
    mockGetLeaderboard.mockRejectedValue(new Error('Leaderboard unavailable'));
    renderPage();
    expect(await screen.findByRole('alert')).toHaveTextContent('Leaderboard unavailable');
  });
});
