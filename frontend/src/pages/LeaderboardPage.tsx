import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import {
  getLeaderboard,
  type LeaderboardEntry,
  type LeaderboardResponse,
} from '../api/leaderboard';
import alertStyles from '../components/alert.module.css';
import controls from '../components/controls.module.css';
import styles from './LeaderboardPage.module.css';

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
      <div className={styles.page}>
        <h1 className={styles.title}>Leaderboard</h1>
        <p role="alert" className={alertStyles.error}>
          {error}
        </p>
      </div>
    );
  }

  if (!data) {
    return (
      <div className={styles.page}>
        <h1 className={styles.title}>Leaderboard</h1>
        <p className={styles.hint}>Loading...</p>
      </div>
    );
  }

  const totalPages = Math.ceil(data.total / data.per_page);

  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Leaderboard</h1>

      {data.entries.length === 0 ? (
        <p className={styles.empty}>No players yet.</p>
      ) : (
        <table className={styles.table}>
          <thead>
            <tr>
              <th>Rank</th>
              <th>Player</th>
              <th className={styles.rating}>Rating</th>
            </tr>
          </thead>
          <tbody>
            {data.entries.map((entry: LeaderboardEntry) => (
              <tr key={entry.user_id}>
                <td className={styles.rank}>{rankMedal(entry.rank)}</td>
                <td className={styles.player}>
                  <Link to={`/users/${entry.username}`}>{entry.username}</Link>
                </td>
                <td className={styles.rating}>{entry.rating}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {totalPages > 1 && (
        <div className={`${controls.row} ${styles.pager}`}>
          <button
            className={controls.button}
            disabled={page <= 1}
            onClick={() => setPage((p) => p - 1)}
          >
            Prev
          </button>
          <span className={styles.pageCount}>
            {page} / {totalPages}
          </span>
          <button
            className={controls.button}
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
