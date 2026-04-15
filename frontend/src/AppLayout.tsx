import { NavLink, Outlet } from 'react-router-dom';

const SHELL_STYLE: React.CSSProperties = {
  display: 'flex',
  minHeight: '100vh',
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
  { to: '/', label: 'Home', icon: '\u2302' },
  { to: '/battle', label: 'Battle', icon: '\u2694' },
  { to: '/builder', label: 'Builder', icon: '\u270E' },
  { to: '/learn', label: 'Learn', icon: '\u2139' },
];

export default function AppLayout() {
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
      </nav>
      <main style={MAIN_STYLE}>
        <Outlet />
      </main>
    </div>
  );
}
