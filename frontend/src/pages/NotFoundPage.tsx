import { Link, useLocation } from 'react-router-dom';

// Rendered by the catch-all `path="*"` route for any URL that matches no
// real page. Before this existed, an unmatched URL rendered nothing at all —
// not even the AppLayout shell — leaving a blank black screen.
//
// The HTTP status is handled separately: Cloudflare Pages serves
// dist/404.html (a copy of index.html, see vite.config.ts) with a real 404
// status for any path that public/_redirects doesn't rewrite. The SPA then
// boots and lands here.

const PAGE_STYLE: React.CSSProperties = {
  minHeight: '100vh',
  padding: '2rem',
  maxWidth: '720px',
  margin: '0 auto',
  display: 'flex',
  flexDirection: 'column',
  justifyContent: 'center',
};

const CODE_STYLE: React.CSSProperties = {
  margin: 0,
  fontSize: '4rem',
  color: '#e94560',
  letterSpacing: '0.08em',
};

const TITLE_STYLE: React.CSSProperties = {
  margin: '0 0 1.5rem',
  fontSize: '1.4rem',
  letterSpacing: '0.08em',
};

const CELL_STYLE: React.CSSProperties = {
  margin: '0 0 1.5rem',
  padding: '1rem',
  backgroundColor: '#0d0d0d',
  border: '1px solid #2a2a2a',
  borderRadius: '6px',
  color: '#888888',
  overflowWrap: 'anywhere',
};

const LINK_STYLE: React.CSSProperties = {
  color: '#4fc3f7',
};

export default function NotFoundPage() {
  const { pathname } = useLocation();

  return (
    <div style={PAGE_STYLE}>
      <h1 style={CODE_STYLE}>404</h1>
      <h2 style={TITLE_STYLE}>Address out of core</h2>
      <div style={CELL_STYLE}>
        <div>{pathname}</div>
        <div>DAT.F #0, #0 ; nothing executable lives here</div>
      </div>
      <p>
        That page doesn&apos;t exist.{' '}
        <Link to="/" style={LINK_STYLE}>
          Return home
        </Link>
      </p>
    </div>
  );
}
