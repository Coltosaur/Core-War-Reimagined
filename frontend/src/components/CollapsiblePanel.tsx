import { useId, useState, type ReactNode } from 'react';
import styles from './CollapsiblePanel.module.css';

type Props = {
  title: string;
  /** Initial state on phones. Desktop always shows the body. */
  defaultOpen?: boolean;
  className?: string;
  children: ReactNode;
};

export default function CollapsiblePanel({
  title,
  defaultOpen = false,
  className,
  children,
}: Props) {
  const [open, setOpen] = useState(defaultOpen);
  const bodyId = useId();

  return (
    <section className={className ? `${styles.panel} ${className}` : styles.panel}>
      <button
        type="button"
        className={styles.toggle}
        aria-expanded={open}
        aria-controls={bodyId}
        onClick={() => setOpen((o) => !o)}
      >
        {title}
        <span className={styles.chevron} aria-hidden>
          ›
        </span>
      </button>
      <h2 className={styles.heading}>{title}</h2>
      <div id={bodyId} className={styles.body} hidden={!open}>
        {children}
      </div>
    </section>
  );
}
