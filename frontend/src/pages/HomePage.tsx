import { Link } from 'react-router-dom';
import styles from './HomePage.module.css';

export default function HomePage() {
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>CORE WAR</h1>
      <p className={styles.lede}>
        A modernized rebuild of the 1984 programming game. Write programs in{' '}
        <strong>Redcode</strong> assembly, load them into <strong>MARS</strong> &mdash; the Memory
        Array Redcode Simulator &mdash; and watch your warriors battle for control of the core.
      </p>
      <nav aria-label="Get started" className={styles.actions}>
        <Link to="/battle" className={`${styles.action} ${styles.primary}`}>
          Enter Battlefield
        </Link>
        <Link to="/builder" className={styles.action}>
          Warrior Builder
        </Link>
        <Link to="/learn" className={styles.action}>
          Learn Redcode
        </Link>
      </nav>
    </div>
  );
}
