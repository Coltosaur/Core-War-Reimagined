import { Link } from 'react-router-dom';
import type { ProfileStats } from '../../api/profile';
import { formatDate, type ProfileWarrior } from './profileUtils';
import styles from './Profile.module.css';

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

type StatKey = 'warrior_count' | 'wins' | 'losses' | 'ties' | 'match_count';

const STATS: { key: StatKey; label: string; className?: string }[] = [
  { key: 'warrior_count', label: 'Warriors' },
  { key: 'wins', label: 'Wins', className: styles.wins },
  { key: 'losses', label: 'Losses', className: styles.losses },
  { key: 'ties', label: 'Ties', className: styles.ties },
  { key: 'match_count', label: 'Matches' },
];

function StatsCards({ stats }: { stats: ProfileStats }) {
  return (
    <ul className={styles.stats}>
      {STATS.map(({ key, label, className }) => (
        <li key={key} className={styles.stat}>
          <div className={`${styles.statValue} ${className ?? ''}`}>{stats[key]}</div>
          <div className={styles.statLabel}>{label}</div>
        </li>
      ))}
    </ul>
  );
}

function WarriorsList({ warriors, own }: { warriors: ProfileWarrior[]; own: boolean }) {
  return (
    <>
      <h2 className={styles.sectionHeader}>Warriors ({warriors.length})</h2>
      {warriors.length === 0 ? (
        <p className={styles.empty}>
          {own ? 'No warriors yet — head to the Builder to write your first.' : 'No warriors yet.'}
        </p>
      ) : (
        <ul className={styles.warriors}>
          {warriors.map((w) => (
            <li key={w.id} className={styles.warrior} data-testid="warrior-row">
              <span className={styles.warriorName}>{w.name}</span>
              <span className={styles.warriorDate}>{formatDate(w.updated_at)}</span>
            </li>
          ))}
        </ul>
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
  own = false,
}: ProfileContentProps) {
  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <h1 className={styles.username}>{username}</h1>
        <span className={styles.joined}>Joined {formatDate(createdAt)}</span>
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
      <h2 className={styles.sectionHeader}>Quick Actions</h2>
      <div className={styles.actions}>
        <Link to="/builder" className={styles.action}>
          New Warrior
        </Link>
        <Link to="/lobby" className={`${styles.action} ${styles.primary}`}>
          Start Battle
        </Link>
      </div>
    </>
  );
}
