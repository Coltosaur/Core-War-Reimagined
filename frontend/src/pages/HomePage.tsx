import { Link } from 'react-router-dom';

const PAGE_STYLE: React.CSSProperties = {
  minHeight: '100vh',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  padding: '2rem',
  gap: '1.5rem',
  textAlign: 'center',
};

const TITLE_STYLE: React.CSSProperties = {
  margin: 0,
  fontSize: '3rem',
  letterSpacing: '0.15em',
  color: '#e94560',
  textShadow: '0 0 24px #e9456044',
};

const LEDE_STYLE: React.CSSProperties = {
  maxWidth: '620px',
  fontSize: '0.95rem',
  color: '#bbb',
  lineHeight: 1.6,
};

const BUTTONS_STYLE: React.CSSProperties = {
  display: 'flex',
  gap: '1rem',
  marginTop: '1rem',
};

const linkButton = (accent: string): React.CSSProperties => ({
  padding: '0.75rem 1.75rem',
  fontFamily: 'inherit',
  fontSize: '0.95rem',
  letterSpacing: '0.05em',
  color: accent,
  textDecoration: 'none',
  border: `1px solid ${accent}66`,
  borderRadius: '6px',
  backgroundColor: `${accent}11`,
  transition: 'background-color 0.15s',
});

export default function HomePage() {
  return (
    <div style={PAGE_STYLE}>
      <h1 style={TITLE_STYLE}>CORE WAR</h1>
      <p style={LEDE_STYLE}>
        A modernized rebuild of the 1984 programming game. Write programs in{' '}
        <strong>Redcode</strong> assembly, load them into <strong>MARS</strong>{' '}
        &mdash; the Memory Array Redcode Simulator &mdash; and watch your
        warriors battle for control of the core.
      </p>
      <div style={BUTTONS_STYLE}>
        <Link to="/battle" style={linkButton('#e94560')}>
          Enter Battlefield
        </Link>
        <Link to="/builder" style={linkButton('#4fc3f7')}>
          Warrior Builder
        </Link>
        <Link to="/learn" style={linkButton('#f0c040')}>
          Learn Redcode
        </Link>
      </div>
    </div>
  );
}
