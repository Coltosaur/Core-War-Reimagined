import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, cleanup, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import AppLayout from './AppLayout';
import * as useAuthModule from './api/useAuth';
import { useServerWarriorSync } from './warriors/useServerWarriorSync';
import { authState } from './test/helpers/mockAuth';

vi.mock('./api/useAuth');
vi.mock('./warriors/useServerWarriorSync');
// The modal has its own tests; here we only care that the shell opens it.
vi.mock('./api/AuthModal', () => ({
  default: () => <div role="dialog" aria-label="auth" />,
}));

const mockUseAuth = vi.mocked(useAuthModule.useAuth);
const logoutSpy = vi.fn();

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route element={<AppLayout />}>
          <Route path="*" element={<p>page body</p>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mockUseAuth.mockReturnValue(authState({ logout: logoutSpy }));
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('AppLayout', () => {
  // The shell is where saved warriors get synced for every route (#58). A
  // full rewrite of this file once dropped the call and silently emptied
  // "My Warriors" app-wide; this pins it.
  it('mounts the app-level server warrior sync', () => {
    renderAt('/');
    expect(useServerWarriorSync).toHaveBeenCalled();
  });

  it('renders every section in the main nav, plus the page outlet', () => {
    renderAt('/');
    const nav = screen.getByRole('navigation', { name: 'Main' });
    const labels = within(nav)
      .getAllByRole('link')
      .map((a) => a.textContent);
    expect(labels).toEqual([
      expect.stringContaining('Home'),
      expect.stringContaining('Battle'),
      expect.stringContaining('Builder'),
      expect.stringContaining('Learn'),
      expect.stringContaining('Play'),
      expect.stringContaining('Ranks'),
    ]);
    expect(screen.getByRole('main')).toHaveTextContent('page body');
  });

  it('marks only the current section as active', () => {
    renderAt('/learn');
    const nav = screen.getByRole('navigation', { name: 'Main' });
    const current = within(nav)
      .getAllByRole('link')
      .filter((a) => a.getAttribute('aria-current') === 'page');
    expect(current).toHaveLength(1);
    expect(current[0]).toHaveTextContent('Learn');
  });

  it('does not mark Home active on other routes', () => {
    renderAt('/leaderboard');
    expect(screen.getByRole('link', { name: /Home/ })).not.toHaveAttribute('aria-current');
  });

  it('forces text presentation on nav icons so phones do not render emoji', () => {
    renderAt('/');
    const nav = screen.getByRole('navigation', { name: 'Main' });
    for (const link of within(nav).getAllByRole('link')) {
      const icon = link.querySelector('[aria-hidden]');
      expect(icon?.textContent).toMatch(/︎$/);
    }
  });

  it('opens the auth modal from the log-in button when logged out', async () => {
    const user = userEvent.setup();
    renderAt('/');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /Log In/ }));
    expect(screen.getByRole('dialog', { name: 'auth' })).toBeInTheDocument();
  });

  it('shows the username and a working logout when logged in', async () => {
    mockUseAuth.mockReturnValue(
      authState({ logout: logoutSpy, user: { user_id: 'u1', username: 'neo' } }),
    );
    const user = userEvent.setup();
    renderAt('/');
    expect(screen.getByRole('link', { name: 'neo' })).toHaveAttribute('href', '/profile');
    expect(screen.queryByRole('button', { name: /Log In/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /Logout/ }));
    expect(logoutSpy).toHaveBeenCalledOnce();
  });

  it('renders no auth controls while the session is loading', () => {
    mockUseAuth.mockReturnValue(authState({ logout: logoutSpy, loading: true }));
    renderAt('/');
    expect(screen.queryByRole('button', { name: /Log In|Logout/ })).not.toBeInTheDocument();
  });
});
