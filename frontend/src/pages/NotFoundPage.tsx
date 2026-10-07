import { Link, useLocation } from 'react-router-dom';
import styles from './NotFoundPage.module.css';

// Rendered by the catch-all `path="*"` route for any URL that matches no
// real page. Before this existed, an unmatched URL rendered nothing at all —
// not even the AppLayout shell — leaving a blank black screen.
//
// The HTTP status is handled separately: Cloudflare Pages serves
// dist/404.html (a copy of index.html, see vite.config.ts) with a real 404
// status for any path that public/_redirects doesn't rewrite. The SPA then
// boots and lands here.

export default function NotFoundPage() {
  const { pathname } = useLocation();

  return (
    <div className={styles.page}>
      <h1 className={styles.code}>404</h1>
      <h2 className={styles.title}>Address out of core</h2>
      <div className={styles.cell}>
        <div className={styles.address}>{pathname}</div>
        <div className={styles.instruction}>
          DAT.F #0, #0 <span className={styles.comment}>; nothing executable lives here</span>
        </div>
      </div>
      <p className={styles.message}>
        That page doesn&apos;t exist. <Link to="/">Return home</Link>
      </p>
    </div>
  );
}
