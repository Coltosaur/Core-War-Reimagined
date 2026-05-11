import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import {
  getLeaderboard,
  type LeaderboardEntry,
  type LeaderboardResponse,
} from '../api/leaderboard';

const PAGE_STYLE: React.CSSProperties = {
  minHeight: '100vh',
  padding: '2rem',
  maxWidth: '720px',
  margin: '0 auto',
};

const TITLE_STYLE: React.CSSProperties = {
  margin: '0 0 1.5rem',
  fontSize: '1.8rem',
  color: '#e94560',
  letterSpacing: '0.08em',
};

const TABLE_STYLE: React.CSSProperties = {
  width: '100%',
  borderCollapse: 'collapse',
};

const TH_STYLE: React.CSSProperties = {
  textAlign: 'left',
  padding: '0.6rem 0.75rem',
  fontSize: '0.65rem',
  letterSpacing: '0.1em',
  color: '#666',
  textTransform: 'uppercase',
  borderBottom: '1px solid #333',
};

const TD_STYLE: React.CSSProperties = {
  padding: '0.6rem 0.75rem',
  borderBottom: '1px solid #1a1a1a',
};

const RANK_STYLE: React.CSSProperties = {
  color: '#888',
  fontWeight: 600,
  width: '50px',
};

const USERNAME_LINK: React.CSSProperties = {
  color: '#4fc3f7',
  textDecoration: 'none',
  fontWeight: 600,
};

const RATING_STYLE: React.CSSProperties = {
  color: '#f0c040',
  fontWeight: 700,
  textAlign: 'right',
};

const PAGINATION_STYLE: React.CSSProperties = {
  display: 'flex',
  justifyContent: 'center',
  gap: '0.75rem',
  marginTop: '1.5rem',
};

const PAGE_BTN: React.CSSProperties = {
  padding: '0.4rem 1rem',
  fontSize: '0.8rem',
  fontFamily: 'inherit',
  color: '#888',
  backgroundColor: '#111',
  border: '1px solid #333',
  borderRadius: '4px',
  cursor: 'pointer',
};

const PAGE_BTN_DISABLED: React.CSSProperties = {
  ...PAGE_BTN,
  opacity: 0.3,
  cursor: 'default',
};

function rankMedal(rank: number): string {
  if (rank === 1) return '\u{1F947}';
  if (rank === 2) return '\u{1F948}';
  if (rank === 3) return '\u{1F949}';
  return `#${rank}`;
}

export default function LeaderboardPage() {
  const [data, setData] = useState<LeaderboardResponse | null>(null);
  const [page, setPage] = useState(1);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getLeaderboard(page)
      .then((d) => {
        if (!cancelled) setData(d);
      })
      .catch((e) => {
        if (!cancelled) setError(e instanceof Error ? e.message : 'Failed to load');
      });
    return () => {
      cancelled = true;
    };
  }, [page]);

  if (error) {
    return (
      <div style={PAGE_STYLE}>
        <p style={{ color: '#e94560' }}>{error}</p>
      </div>
    );
  }

  if (!data) {
    return (
      <div style={PAGE_STYLE}>
        <p style={{ color: '#888' }}>Loading...</p>
      </div>
    );
  }

  const totalPages = Math.ceil(data.total / data.per_page);

  return (
    <div style={PAGE_STYLE}>
      <h1 style={TITLE_STYLE}>Leaderboard</h1>

      {data.entries.length === 0 ? (
        <p style={{ color: '#555', fontStyle: 'italic' }}>No players yet.</p>
      ) : (
        <table style={TABLE_STYLE}>
          <thead>
            <tr>
              <th style={TH_STYLE}>Rank</th>
              <th style={TH_STYLE}>Player</th>
              <th style={{ ...TH_STYLE, textAlign: 'right' }}>Rating</th>
            </tr>
          </thead>
          <tbody>
            {data.entries.map((entry: LeaderboardEntry) => (
              <tr key={entry.user_id}>
                <td style={{ ...TD_STYLE, ...RANK_STYLE }}>{rankMedal(entry.rank)}</td>
                <td style={TD_STYLE}>
                  <Link to={`/users/${entry.username}`} style={USERNAME_LINK}>
                    {entry.username}
                  </Link>
                </td>
                <td style={{ ...TD_STYLE, ...RATING_STYLE }}>{entry.rating}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {totalPages > 1 && (
        <div style={PAGINATION_STYLE}>
          <button
            style={page <= 1 ? PAGE_BTN_DISABLED : PAGE_BTN}
            disabled={page <= 1}
            onClick={() => setPage((p) => p - 1)}
          >
            Prev
          </button>
          <span style={{ color: '#666', fontSize: '0.8rem', alignSelf: 'center' }}>
            {page} / {totalPages}
          </span>
          <button
            style={page >= totalPages ? PAGE_BTN_DISABLED : PAGE_BTN}
            disabled={page >= totalPages}
            onClick={() => setPage((p) => p + 1)}
          >
            Next
          </button>
        </div>
      )}
    </div>
  );
}
