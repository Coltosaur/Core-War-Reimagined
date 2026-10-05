import { useState } from 'react';
import { NavLink, Outlet, Link } from 'react-router-dom';
import { useAuth } from './api/AuthContext';
import AuthModal from './api/AuthModal';
import { useServerWarriorSync } from './warriors/useServerWarriorSync';
import styles from './AppLayout.module.css';

type Item = { to: string; label: string; icon: string };

// U+FE0E (text presentation selector) keeps iOS and Android from swapping
// these symbols for color emoji, which would clash with the monochrome nav.
const TEXT = '︎';

const ITEMS: Item[] = [
  { to: '/', label: 'Home', icon: `⌂${TEXT}` },
  { to: '/battle', label: 'Battle', icon: `⚔${TEXT}` },
  { to: '/builder', label: 'Builder', icon: `✎${TEXT}` },
  { to: '/learn', label: 'Learn', icon: `ℹ${TEXT}` },
  { to: '/lobby', label: 'Play', icon: `▶${TEXT}` },
  { to: '/leaderboard', label: 'Ranks', icon: `♛${TEXT}` },
];

const navItemClass = ({ isActive }: { isActive: boolean }) =>
  isActive ? `${styles.navItem} ${styles.active}` : styles.navItem;

export default function AppLayout() {
  const { user, loading, logout } = useAuth();
  // App-level owner of the server warrior cache: every route, not just the
  // Builder, needs the logged-in user's saved warriors (#58).
  useServerWarriorSync();
  const [showAuth, setShowAuth] = useState(false);

  return (
    <div className={styles.shell}>
      <div className={styles.brandCell}>
        <Link to="/" className={styles.brand}>
          CORE WAR
        </Link>
      </div>

      <nav className={styles.nav} aria-label="Main">
        {ITEMS.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === '/'}
            title={item.label}
            className={navItemClass}
          >
            <span className={styles.icon} aria-hidden>
              {item.icon}
            </span>
            <span className={styles.label}>{item.label}</span>
          </NavLink>
        ))}
      </nav>

      <div className={styles.auth}>
        {loading ? null : user ? (
          <>
            <Link to="/profile" className={styles.username} title="Dashboard">
              {user.username}
            </Link>
            <button type="button" className={styles.authButton} onClick={logout} title="Log out">
              <span className={styles.icon} aria-hidden>
                ←
              </span>
              <span>Logout</span>
            </button>
          </>
        ) : (
          <button
            type="button"
            className={styles.authButton}
            onClick={() => setShowAuth(true)}
            title="Log in or register"
          >
            <span className={styles.icon} aria-hidden>
              →
            </span>
            <span>Log In</span>
          </button>
        )}
      </div>

      <main className={styles.main}>
        <Outlet />
      </main>

      {showAuth && <AuthModal onClose={() => setShowAuth(false)} />}
    </div>
  );
}
