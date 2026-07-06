import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { MockedFunction } from 'vitest';
import { render, screen, waitFor, cleanup } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import ProfilePage from './ProfilePage';
import * as profileApi from '../../api/profile';
import * as warriorsApi from '../../api/warriors';
import * as accountApi from '../../api/account';
import * as AuthContextModule from '../../api/AuthContext';

vi.mock('../../api/profile');
vi.mock('../../api/warriors');
vi.mock('../../api/account');
vi.mock('../../api/AuthContext');

type UseAuthReturn = ReturnType<typeof AuthContextModule.useAuth>;

const mockUseAuth = AuthContextModule.useAuth as MockedFunction<() => UseAuthReturn>;
const mockGetMyProfile = profileApi.getMyProfile as MockedFunction<typeof profileApi.getMyProfile>;
const mockGetPublicProfile = profileApi.getPublicProfile as MockedFunction<
  typeof profileApi.getPublicProfile
>;
const mockListWarriors = warriorsApi.listWarriors as MockedFunction<
  typeof warriorsApi.listWarriors
>;
const mockGetAccount = accountApi.getAccount as MockedFunction<typeof accountApi.getAccount>;

const myStats: profileApi.ProfileStats = {
  user_id: 'u1',
  username: 'vale',
  created_at: '2025-01-01T00:00:00Z',
  warrior_count: 2,
  match_count: 10,
  wins: 5,
  losses: 4,
  ties: 1,
};

const myServerWarriors: warriorsApi.ServerWarrior[] = [
  {
    id: 'w1',
    user_id: 'u1',
    name: 'Imp Redux',
    source: 'MOV.I $0, $1',
    created_at: '2025-01-02T00:00:00Z',
    updated_at: '2025-01-03T00:00:00Z',
  },
  {
    id: 'w2',
    user_id: 'u1',
    name: 'Bomber',
    source: 'DAT #0, #0',
    created_at: '2025-01-04T00:00:00Z',
    updated_at: '2025-01-05T00:00:00Z',
  },
];

const authedUser: UseAuthReturn = {
  user: { user_id: 'u1', username: 'vale' },
  loading: false,
  login: vi.fn(),
  register: vi.fn(),
  logout: vi.fn(),
};

const anonymous: UseAuthReturn = {
  user: null,
  loading: false,
  login: vi.fn(),
  register: vi.fn(),
  logout: vi.fn(),
};

beforeEach(() => {
  mockUseAuth.mockReturnValue(authedUser);
  mockGetMyProfile.mockResolvedValue(myStats);
  mockListWarriors.mockResolvedValue({
    warriors: myServerWarriors,
    total: myServerWarriors.length,
    page: 1,
    per_page: 100,
  });
  mockGetAccount.mockResolvedValue({ email: 'vale@example.com' });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/profile" element={<ProfilePage />} />
        <Route path="/users/:username" element={<ProfilePage />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe('/profile — own dashboard', () => {
  it('renders the warriors list (the exact bug in #88)', async () => {
    renderAt('/profile');

    // Wait for warriors to appear.
    await waitFor(() => {
      expect(screen.getByText('Imp Redux')).toBeInTheDocument();
    });
    expect(screen.getByText('Bomber')).toBeInTheDocument();
    // Warriors list section header includes the count.
    expect(screen.getByText(/Warriors \(2\)/)).toBeInTheDocument();
  });

  it('renders the Account Settings panel', async () => {
    renderAt('/profile');

    await waitFor(() => {
      expect(screen.getByRole('region', { name: /account settings/i })).toBeInTheDocument();
    });
    expect(screen.getByRole('form', { name: /change password/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /log out/i })).toBeInTheDocument();
  });

  it('Start Battle Quick Action points to /lobby (not /battle)', async () => {
    renderAt('/profile');

    const startBattle = await screen.findByRole('link', { name: /start battle/i });
    expect(startBattle).toHaveAttribute('href', '/lobby');
  });

  it('displays the email in the Account Settings panel', async () => {
    renderAt('/profile');

    await waitFor(() => {
      expect(screen.getByTestId('account-email')).toHaveTextContent('vale@example.com');
    });
  });
});

describe('/users/:username — public profile', () => {
  const otherWarriors: profileApi.PublicWarrior[] = [
    {
      id: 'ow1',
      name: 'Public Warrior',
      created_at: '2025-02-01T00:00:00Z',
      updated_at: '2025-02-02T00:00:00Z',
    },
  ];

  const publicProfile: profileApi.PublicProfile = {
    user_id: 'u2',
    username: 'orion',
    created_at: '2024-06-01T00:00:00Z',
    warrior_count: 1,
    match_count: 3,
    wins: 2,
    losses: 1,
    ties: 0,
    warriors: otherWarriors,
  };

  beforeEach(() => {
    mockGetPublicProfile.mockResolvedValue(publicProfile);
  });

  it('renders warriors list from the public endpoint', async () => {
    renderAt('/users/orion');

    await waitFor(() => {
      expect(screen.getByText('Public Warrior')).toBeInTheDocument();
    });
    // Public endpoint drives the fetch; the /api/warriors endpoint must not
    // be called (it would 401 for a stranger looking at your profile).
    expect(mockListWarriors).not.toHaveBeenCalled();
  });

  it('does NOT render the Account Settings panel', async () => {
    renderAt('/users/orion');

    await waitFor(() => {
      expect(screen.getByText('Public Warrior')).toBeInTheDocument();
    });
    expect(screen.queryByRole('region', { name: /account settings/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('form', { name: /change password/i })).not.toBeInTheDocument();
  });

  it('does NOT render Quick Actions (no Start Battle link)', async () => {
    renderAt('/users/orion');

    await waitFor(() => {
      expect(screen.getByText('Public Warrior')).toBeInTheDocument();
    });
    expect(screen.queryByRole('link', { name: /start battle/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('link', { name: /new warrior/i })).not.toBeInTheDocument();
  });
});

describe('/profile — not logged in', () => {
  it('renders a login prompt', () => {
    mockUseAuth.mockReturnValue(anonymous);
    renderAt('/profile');

    expect(screen.getByText(/log in to view your dashboard/i)).toBeInTheDocument();
    expect(mockGetMyProfile).not.toHaveBeenCalled();
  });
});
