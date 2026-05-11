import { useState } from 'react';
import { NavLink, Outlet } from 'react-router-dom';
import { useAuth } from './api/AuthContext';
import AuthModal from './api/AuthModal';

const SHELL_STYLE: React.CSSProperties = {
  display: 'flex',
  height: '100vh',
  fontFamily: '"JetBrains Mono", "Fira Code", monospace',
  backgroundColor: '#0a0a0a',
  color: '#e0e0e0',
};

const SIDEBAR_STYLE: React.CSSProperties = {
  width: '80px',
  flexShrink: 0,
  backgroundColor: '#111',
  borderRight: '1px solid #222',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  padding: '1rem 0',
  gap: '0.25rem',
};

const MAIN_STYLE: React.CSSProperties = {
  flex: 1,
  minWidth: 0,
  overflow: 'auto',
};

const navLinkStyle = (active: boolean): React.CSSProperties => ({
  width: '64px',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  gap: '0.2rem',
  padding: '0.5rem 0',
  borderRadius: '6px',
  color: active ? '#e94560' : '#888',
  backgroundColor: active ? '#1a1a1a' : 'transparent',
  textDecoration: 'none',
  transition: 'background-color 0.15s, color 0.15s',
});

const ICON_STYLE: React.CSSProperties = {
  fontSize: '1.3rem',
  lineHeight: 1,
};

const LABEL_STYLE: React.CSSProperties = {
  fontSize: '0.65rem',
  letterSpacing: '0.05em',
  textTransform: 'uppercase',
};

type Item = { to: string; label: string; icon: string };

const ITEMS: Item[] = [
  { to: '/', label: 'Home', icon: '⌂' },
  { to: '/battle', label: 'Battle', icon: '⚔' },
  { to: '/builder', label: 'Builder', icon: '✎' },
  { to: '/learn', label: 'Learn', icon: 'ℹ' },
  { to: '/leaderboard', label: 'Ranks', icon: '♛' },
];

const AUTH_SECTION: React.CSSProperties = {
  marginTop: 'auto',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: '0.25rem',
  padding: '0.5rem 0',
};

const AUTH_BTN: React.CSSProperties = {
  width: '64px',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  gap: '0.2rem',
  padding: '0.5rem 0',
  borderRadius: '6px',
  color: '#888',
  backgroundColor: 'transparent',
  border: 'none',
  cursor: 'pointer',
  fontFamily: 'inherit',
  transition: 'background-color 0.15s, color 0.15s',
};

const USERNAME_STYLE: React.CSSProperties = {
  fontSize: '0.6rem',
  color: '#4fc3f7',
  textAlign: 'center',
  wordBreak: 'break-all',
  maxWidth: '70px',
  lineHeight: 1.2,
};

export default function AppLayout() {
  const { user, loading, logout } = useAuth();
  const [showAuth, setShowAuth] = useState(false);

  return (
    <div style={SHELL_STYLE}>
      <nav style={SIDEBAR_STYLE}>
        {ITEMS.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === '/'}
            title={item.label}
            style={({ isActive }) => navLinkStyle(isActive)}
          >
            <span style={ICON_STYLE}>{item.icon}</span>
            <span style={LABEL_STYLE}>{item.label}</span>
          </NavLink>
        ))}

        <div style={AUTH_SECTION}>
          {loading ? null : user ? (
            <>
              <span style={USERNAME_STYLE}>{user.username}</span>
              <button style={AUTH_BTN} onClick={logout} title="Log out">
                <span style={ICON_STYLE}>{'←'}</span>
                <span style={LABEL_STYLE}>Logout</span>
              </button>
            </>
          ) : (
            <button style={AUTH_BTN} onClick={() => setShowAuth(true)} title="Log in or register">
              <span style={ICON_STYLE}>{'→'}</span>
              <span style={LABEL_STYLE}>Log In</span>
            </button>
          )}
        </div>
      </nav>
      <main style={MAIN_STYLE}>
        <Outlet />
      </main>
      {showAuth && <AuthModal onClose={() => setShowAuth(false)} />}
    </div>
  );
}
