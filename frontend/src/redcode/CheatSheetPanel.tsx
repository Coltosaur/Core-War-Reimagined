import { useId, useState } from 'react';
import { OPCODES, MODIFIERS, ADDRESSING_MODES, PSEUDO_OPS, type CheatEntry } from './cheatSheet';
import styles from './CheatSheetPanel.module.css';

// Open by default where there's room beside the editor; closed on phones,
// where it would push the editor down.
const DESKTOP_QUERY = '(min-width: 48rem)';
const startsOpen = () =>
  typeof window !== 'undefined' && (window.matchMedia?.(DESKTOP_QUERY).matches ?? false);

type SectionProps = { title: string; entries: CheatEntry[] };

function Section({ title, entries }: SectionProps) {
  return (
    <section className={styles.section}>
      <h3 className={styles.sectionTitle}>{title}</h3>
      <dl className={styles.entries}>
        {entries.map((e) => (
          <div key={e.symbol} className={styles.row}>
            <dt className={styles.symbol}>{e.symbol}</dt>
            <dd className={styles.desc}>
              {e.desc}
              <span className={styles.name}>{e.name}</span>
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

export default function CheatSheetPanel() {
  const [open, setOpen] = useState(startsOpen);
  const bodyId = useId();

  return (
    <aside
      className={open ? `${styles.panel} ${styles.open}` : styles.panel}
      aria-label="Redcode cheat sheet"
    >
      <button
        type="button"
        className={styles.header}
        aria-expanded={open}
        aria-controls={bodyId}
        onClick={() => setOpen((o) => !o)}
        title={open ? 'Hide cheat sheet' : 'Show cheat sheet'}
      >
        <span className={styles.title}>Redcode Cheat Sheet</span>
        <span className={styles.hint} aria-hidden>
          {open ? 'hide' : 'show'}
        </span>
      </button>
      <div id={bodyId} className={styles.body} hidden={!open}>
        <Section title="Opcodes" entries={OPCODES} />
        <Section title="Modifiers" entries={MODIFIERS} />
        <Section title="Addressing Modes" entries={ADDRESSING_MODES} />
        <Section title="Pseudo-ops" entries={PSEUDO_OPS} />
      </div>
    </aside>
  );
}
