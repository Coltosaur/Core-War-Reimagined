import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { useAuth } from '../../api/AuthContext';
import { getMyProfile, getPublicProfile, type PublicProfile } from '../../api/profile';
import { listWarriors } from '../../api/warriors';
import ProfileContent, { QuickActions } from './ProfileContent';
import type { ProfileWarrior } from './profileStyles';
import AccountSettings from './AccountSettings';

/**
 * Two entry paths, one shared render (see ProfileContent):
 * - `/profile` (own dashboard): stats + warriors + Quick Actions + Account Settings.
 * - `/users/:username` (public view): stats + warriors only.
 *
 * The own dashboard fetches warriors from the existing `/api/warriors`
 * endpoint rather than extending `/api/profile` — the public endpoint
 * already returns warrior rows on its own, so the two paths converge on
 * the same visual list without a backend contract change.
 */

const PAGE_STYLE: React.CSSProperties = {
  minHeight: '100vh',
  padding: '2rem',
  maxWidth: '720px',
  margin: '0 auto',
};

const LOGIN_PROMPT: React.CSSProperties = {
  minHeight: '100vh',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  gap: '1rem',
  color: '#888',
};

function LoadingPage() {
  return (
    <div style={PAGE_STYLE}>
      <p style={{ color: '#888' }}>Loading...</p>
    </div>
  );
}

function ErrorPage({ message }: { message: string }) {
  return (
    <div style={PAGE_STYLE}>
      <p style={{ color: '#e94560' }}>{message}</p>
    </div>
  );
}

function MyDashboard() {
  // Two independent fetches — profile stats and warriors — because we
  // deliberately don't extend /api/profile with a warriors array. If either
  // fails, we render an error, but the two are otherwise decoupled.
  const [profile, setProfile] = useState<Awaited<ReturnType<typeof getMyProfile>> | null>(null);
  const [warriors, setWarriors] = useState<ProfileWarrior[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getMyProfile()
      .then((p) => {
        if (!cancelled) setProfile(p);
      })
      .catch((e) => {
        if (!cancelled) setError(e instanceof Error ? e.message : 'Failed to load profile');
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    // Grab up to 100 warriors — matches the Builder's page size cap. If a
    // user ever exceeds that we'll add pagination to the profile too; today
    // it's a non-issue.
    listWarriors(1, 100)
      .then((resp) => {
        if (!cancelled) {
          setWarriors(
            resp.warriors.map((w) => ({
              id: w.id,
              name: w.name,
              updated_at: w.updated_at,
            })),
          );
        }
      })
      .catch((e) => {
        if (!cancelled) setError(e instanceof Error ? e.message : 'Failed to load warriors');
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (error) return <ErrorPage message={error} />;
  if (!profile || warriors === null) return <LoadingPage />;

  return (
    <ProfileContent
      username={profile.username}
      createdAt={profile.created_at}
      stats={profile}
      warriors={warriors}
      own
      aboveWarriors={<QuickActions />}
      belowWarriors={<AccountSettings />}
    />
  );
}

function PublicProfileView({ username }: { username: string }) {
  const [profile, setProfile] = useState<PublicProfile | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getPublicProfile(username)
      .then((p) => {
        if (!cancelled) setProfile(p);
      })
      .catch((e) => {
        if (!cancelled) setError(e instanceof Error ? e.message : 'Failed to load profile');
      });
    return () => {
      cancelled = true;
    };
  }, [username]);

  if (error) return <ErrorPage message={error} />;
  if (!profile) return <LoadingPage />;

  return (
    <ProfileContent
      username={profile.username}
      createdAt={profile.created_at}
      stats={profile}
      warriors={profile.warriors.map((w) => ({
        id: w.id,
        name: w.name,
        updated_at: w.updated_at,
      }))}
    />
  );
}

export default function ProfilePage() {
  const { username } = useParams<{ username: string }>();
  const { user, loading } = useAuth();

  if (loading) return <LoadingPage />;

  if (username) {
    return <PublicProfileView username={username} />;
  }

  if (!user) {
    return (
      <div style={LOGIN_PROMPT}>
        <p>Log in to view your dashboard.</p>
      </div>
    );
  }

  return <MyDashboard />;
}
