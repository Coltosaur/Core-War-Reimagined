import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { useAuth } from '../../api/AuthContext';
import {
  getMyProfile,
  getPublicProfile,
  type ProfileStats,
  type PublicProfile,
} from '../../api/profile';

const PAGE_STYLE: React.CSSProperties = {
  minHeight: '100vh',
  padding: '2rem',
  maxWidth: '720px',
  margin: '0 auto',
};

const HEADER_STYLE: React.CSSProperties = {
  display: 'flex',
  alignItems: 'baseline',
  gap: '1rem',
  marginBottom: '1.5rem',
};

const USERNAME_STYLE: React.CSSProperties = {
  margin: 0,
  fontSize: '1.8rem',
  color: '#e94560',
  letterSpacing: '0.08em',
};

const JOINED_STYLE: React.CSSProperties = {
  fontSize: '0.75rem',
  color: '#666',
};

const STATS_ROW: React.CSSProperties = {
  display: 'flex',
  gap: '1.5rem',
  marginBottom: '2rem',
  flexWrap: 'wrap',
};

const STAT_BOX: React.CSSProperties = {
  flex: '1 1 100px',
  padding: '0.75rem 1rem',
  backgroundColor: '#111',
  border: '1px solid #222',
  borderRadius: '6px',
  textAlign: 'center',
};

const STAT_VALUE: React.CSSProperties = {
  fontSize: '1.5rem',
  fontWeight: 700,
};

const STAT_LABEL: React.CSSProperties = {
  fontSize: '0.65rem',
  color: '#888',
  textTransform: 'uppercase',
  letterSpacing: '0.1em',
  marginTop: '0.25rem',
};

const SECTION_HEADER: React.CSSProperties = {
  fontSize: '0.7rem',
  letterSpacing: '0.1em',
  color: '#666',
  textTransform: 'uppercase',
  padding: '0.5rem 0',
  borderBottom: '1px solid #222',
  marginBottom: '0.5rem',
};

const WARRIOR_ROW: React.CSSProperties = {
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
  padding: '0.5rem 0',
  borderBottom: '1px solid #1a1a1a',
};

const WARRIOR_NAME: React.CSSProperties = {
  color: '#4fc3f7',
  fontWeight: 600,
};

const WARRIOR_DATE: React.CSSProperties = {
  fontSize: '0.7rem',
  color: '#555',
};

const EMPTY_STYLE: React.CSSProperties = {
  color: '#555',
  fontStyle: 'italic',
  padding: '1rem 0',
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

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

function StatsCards({ stats }: { stats: ProfileStats }) {
  return (
    <div style={STATS_ROW}>
      <div style={STAT_BOX}>
        <div style={{ ...STAT_VALUE, color: '#4fc3f7' }}>{stats.warrior_count}</div>
        <div style={STAT_LABEL}>Warriors</div>
      </div>
      <div style={STAT_BOX}>
        <div style={{ ...STAT_VALUE, color: '#4caf50' }}>{stats.wins}</div>
        <div style={STAT_LABEL}>Wins</div>
      </div>
      <div style={STAT_BOX}>
        <div style={{ ...STAT_VALUE, color: '#e94560' }}>{stats.losses}</div>
        <div style={STAT_LABEL}>Losses</div>
      </div>
      <div style={STAT_BOX}>
        <div style={{ ...STAT_VALUE, color: '#f0c040' }}>{stats.ties}</div>
        <div style={STAT_LABEL}>Ties</div>
      </div>
      <div style={STAT_BOX}>
        <div style={{ ...STAT_VALUE, color: '#e0e0e0' }}>{stats.match_count}</div>
        <div style={STAT_LABEL}>Matches</div>
      </div>
    </div>
  );
}

function MyDashboard() {
  const [profile, setProfile] = useState<ProfileStats | null>(null);
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

  if (error)
    return (
      <div style={PAGE_STYLE}>
        <p style={{ color: '#e94560' }}>{error}</p>
      </div>
    );
  if (!profile)
    return (
      <div style={PAGE_STYLE}>
        <p style={{ color: '#888' }}>Loading...</p>
      </div>
    );

  return (
    <div style={PAGE_STYLE}>
      <div style={HEADER_STYLE}>
        <h1 style={USERNAME_STYLE}>{profile.username}</h1>
        <span style={JOINED_STYLE}>Joined {formatDate(profile.created_at)}</span>
      </div>
      <StatsCards stats={profile} />
      <div style={SECTION_HEADER}>Quick Actions</div>
      <div style={{ display: 'flex', gap: '0.75rem', padding: '0.75rem 0' }}>
        <Link to="/builder" style={actionLink('#4fc3f7')}>
          New Warrior
        </Link>
        <Link to="/battle" style={actionLink('#e94560')}>
          Start Battle
        </Link>
      </div>
    </div>
  );
}

function PublicProfileView({ username }: { username: string }) {
  const [profile, setProfile] = useState<PublicProfile | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getPublicProfile(username)
      .then(setProfile)
      .catch((e) => setError(e instanceof Error ? e.message : 'Failed to load profile'));
  }, [username]);

  if (error)
    return (
      <div style={PAGE_STYLE}>
        <p style={{ color: '#e94560' }}>{error}</p>
      </div>
    );
  if (!profile)
    return (
      <div style={PAGE_STYLE}>
        <p style={{ color: '#888' }}>Loading...</p>
      </div>
    );

  return (
    <div style={PAGE_STYLE}>
      <div style={HEADER_STYLE}>
        <h1 style={USERNAME_STYLE}>{profile.username}</h1>
        <span style={JOINED_STYLE}>Joined {formatDate(profile.created_at)}</span>
      </div>
      <StatsCards stats={profile} />
      <div style={SECTION_HEADER}>Warriors ({profile.warriors.length})</div>
      {profile.warriors.length === 0 ? (
        <p style={EMPTY_STYLE}>No warriors yet.</p>
      ) : (
        profile.warriors.map((w) => (
          <div key={w.id} style={WARRIOR_ROW}>
            <span style={WARRIOR_NAME}>{w.name}</span>
            <span style={WARRIOR_DATE}>{formatDate(w.updated_at)}</span>
          </div>
        ))
      )}
    </div>
  );
}

const actionLink = (color: string): React.CSSProperties => ({
  padding: '0.5rem 1rem',
  fontSize: '0.8rem',
  color,
  textDecoration: 'none',
  border: `1px solid ${color}66`,
  borderRadius: '4px',
  backgroundColor: `${color}11`,
});

export default function ProfilePage() {
  const { username } = useParams<{ username: string }>();
  const { user, loading } = useAuth();

  if (loading)
    return (
      <div style={PAGE_STYLE}>
        <p style={{ color: '#888' }}>Loading...</p>
      </div>
    );

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
