import { useState } from 'react';
import { NavLink, Outlet, Link } from 'react-router-dom';
import { useAuth } from './api/AuthContext';
import AuthModal from './api/AuthModal';
import { useIsNarrow } from './ui/useIsNarrow';

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

// Small screens: the 80px sidebar becomes a bottom tab bar so page content
// gets the full viewport width. column-reverse keeps the DOM order (nav
// first) while placing the bar at the bottom, within thumb reach.
const NARROW_SHELL_STYLE: React.CSSProperties = {
  ...SHELL_STYLE,
  flexDirection: 'column-reverse',
  height: '100dvh',
};

const NARROW_NAV_STYLE: React.CSSProperties = {
  flexShrink: 0,
  backgroundColor: '#111',
  borderTop: '1px solid #222',
  display: 'flex',
  flexDirection: 'row',
  alignItems: 'stretch',
  padding: '0.25rem',
  gap: '0.125rem',
  overflowX: 'auto',
};

const NARROW_MAIN_STYLE: React.CSSProperties = {
  ...MAIN_STYLE,
  minHeight: 0,
};

const navLinkStyle = (active: boolean, narrow = false): React.CSSProperties => ({
  ...(narrow ? { flex: '1 1 0', minWidth: '44px' } : { width: '64px' }),
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

const NARROW_ICON_STYLE: React.CSSProperties = { ...ICON_STYLE, fontSize: '1.1rem' };
const NARROW_LABEL_STYLE: React.CSSProperties = {
  ...LABEL_STYLE,
  fontSize: '0.55rem',
  letterSpacing: 0,
  whiteSpace: 'nowrap',
};

type Item = { to: string; label: string; icon: string };

const ITEMS: Item[] = [
  { to: '/', label: 'Home', icon: '⌂' },
  { to: '/battle', label: 'Battle', icon: '⚔' },
  { to: '/builder', label: 'Builder', icon: '✎' },
  { to: '/learn', label: 'Learn', icon: 'ℹ' },
  { to: '/lobby', label: 'Play', icon: '▶' },
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

const NARROW_AUTH_SECTION: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'row',
  alignItems: 'center',
  gap: '0.125rem',
};

export default function AppLayout() {
  const { user, loading, logout } = useAuth();
  const narrow = useIsNarrow();
  const [showAuth, setShowAuth] = useState(false);

  const iconStyle = narrow ? NARROW_ICON_STYLE : ICON_STYLE;
  const labelStyle = narrow ? NARROW_LABEL_STYLE : LABEL_STYLE;
  const authBtnStyle = narrow ? { ...AUTH_BTN, width: 'auto', minWidth: '44px' } : AUTH_BTN;

  return (
    <div style={narrow ? NARROW_SHELL_STYLE : SHELL_STYLE}>
      <nav style={narrow ? NARROW_NAV_STYLE : SIDEBAR_STYLE}>
        {ITEMS.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === '/'}
            title={item.label}
            style={({ isActive }) => navLinkStyle(isActive, narrow)}
          >
            <span style={iconStyle}>{item.icon}</span>
            <span style={labelStyle}>{item.label}</span>
          </NavLink>
        ))}

        <div style={narrow ? NARROW_AUTH_SECTION : AUTH_SECTION}>
          {loading ? null : user ? (
            <>
              {narrow ? (
                // No room for the username text in the tab bar — keep the
                // profile reachable as a regular tab instead.
                <NavLink
                  to="/profile"
                  title={`Dashboard (${user.username})`}
                  style={({ isActive }) => navLinkStyle(isActive, true)}
                >
                  <span style={iconStyle}>{'◉'}</span>
                  <span style={labelStyle}>Me</span>
                </NavLink>
              ) : (
                <Link
                  to="/profile"
                  style={{ ...USERNAME_STYLE, textDecoration: 'none' }}
                  title="Dashboard"
                >
                  {user.username}
                </Link>
              )}
              <button style={authBtnStyle} onClick={logout} title="Log out">
                <span style={iconStyle}>{'←'}</span>
                <span style={labelStyle}>Logout</span>
              </button>
            </>
          ) : (
            <button
              style={authBtnStyle}
              onClick={() => setShowAuth(true)}
              title="Log in or register"
            >
              <span style={iconStyle}>{'→'}</span>
              <span style={labelStyle}>Log In</span>
            </button>
          )}
        </div>
      </nav>
      <main style={narrow ? NARROW_MAIN_STYLE : MAIN_STYLE}>
        <Outlet />
      </main>
      {showAuth && <AuthModal onClose={() => setShowAuth(false)} />}
    </div>
  );
}
