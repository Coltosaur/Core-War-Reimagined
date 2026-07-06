import { Link } from 'react-router-dom';
import type { ProfileStats } from '../../api/profile';
import { SECTION_HEADER, actionLink, formatDate, type ProfileWarrior } from './profileStyles';

/**
 * Everything both the own-dashboard and the public-view render in common:
 * header (username + join date), stats cards, and warriors list. Kept as one
 * component so the two entry points can't drift — before this refactor the
 * own dashboard had lost the warriors list entirely (see #88).
 *
 * The parent (own or public) decides what to render *around* the shared body
 * (quick actions, account settings, etc.) via the `aboveWarriors` and
 * `belowWarriors` slots.
 */

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

function WarriorsList({ warriors, own }: { warriors: ProfileWarrior[]; own: boolean }) {
  return (
    <>
      <div style={SECTION_HEADER}>Warriors ({warriors.length})</div>
      {warriors.length === 0 ? (
        <p style={EMPTY_STYLE}>
          {own ? 'No warriors yet — head to the Builder to write your first.' : 'No warriors yet.'}
        </p>
      ) : (
        warriors.map((w) => (
          <div key={w.id} style={WARRIOR_ROW} data-testid="warrior-row">
            <span style={WARRIOR_NAME}>{w.name}</span>
            <span style={WARRIOR_DATE}>{formatDate(w.updated_at)}</span>
          </div>
        ))
      )}
    </>
  );
}

export type ProfileContentProps = {
  username: string;
  createdAt: string;
  stats: ProfileStats;
  warriors: ProfileWarrior[];
  /** Rendered above the warriors list, own-view only (quick actions, etc.). */
  aboveWarriors?: React.ReactNode;
  /** Rendered below the warriors list, own-view only (account settings). */
  belowWarriors?: React.ReactNode;
  /** Extends the outermost wrapper for callers that need extra bottom padding. */
  extraStyle?: React.CSSProperties;
  /** Used by tests + empty-state copy to distinguish own vs. public paths. */
  own?: boolean;
};

export default function ProfileContent({
  username,
  createdAt,
  stats,
  warriors,
  aboveWarriors,
  belowWarriors,
  extraStyle,
  own = false,
}: ProfileContentProps) {
  return (
    <div style={{ ...PAGE_STYLE, ...extraStyle }}>
      <div style={HEADER_STYLE}>
        <h1 style={USERNAME_STYLE}>{username}</h1>
        <span style={JOINED_STYLE}>Joined {formatDate(createdAt)}</span>
      </div>
      <StatsCards stats={stats} />
      {aboveWarriors}
      <WarriorsList warriors={warriors} own={own} />
      {belowWarriors}
    </div>
  );
}

export function QuickActions() {
  return (
    <>
      <div style={SECTION_HEADER}>Quick Actions</div>
      <div style={{ display: 'flex', gap: '0.75rem', padding: '0.75rem 0' }}>
        <Link to="/builder" style={actionLink('#4fc3f7')}>
          New Warrior
        </Link>
        <Link to="/lobby" style={actionLink('#e94560')}>
          Start Battle
        </Link>
      </div>
    </>
  );
}
