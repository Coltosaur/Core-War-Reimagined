import type { Warrior } from '../../warriors/library';
import styles from './WarriorListItem.module.css';

type Props = {
  warrior: Warrior;
  active: boolean;
  onSelect: (id: string) => void;
};

export default function WarriorListItem({ warrior, active, onSelect }: Props) {
  const className = [styles.item, warrior.isPreset && styles.preset, active && styles.active]
    .filter(Boolean)
    .join(' ');

  return (
    <li>
      <button
        type="button"
        className={className}
        aria-current={active ? 'true' : undefined}
        onClick={() => onSelect(warrior.id)}
      >
        <span className={styles.label}>{warrior.label}</span>
        {warrior.isPreset && <span className={styles.badge}>CLASSIC</span>}
      </button>
    </li>
  );
}
