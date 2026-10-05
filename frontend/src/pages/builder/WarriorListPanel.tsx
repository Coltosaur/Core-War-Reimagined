import type { Warrior } from '../../warriors/library';
import CollapsiblePanel from '../../components/CollapsiblePanel';
import controls from '../../components/controls.module.css';
import WarriorListItem from './WarriorListItem';
import styles from './WarriorListPanel.module.css';

type Props = {
  presets: Warrior[];
  userWarriors: Warrior[];
  selectedId: string;
  onSelect: (id: string) => void;
  onNew: () => void;
};

export default function WarriorListPanel({
  presets,
  userWarriors,
  selectedId,
  onSelect,
  onNew,
}: Props) {
  return (
    <aside className={styles.list} aria-label="Warriors">
      <CollapsiblePanel title="Warriors" className={styles.panel}>
        <h3 className={styles.group}>Classic (read-only)</h3>
        <ul className={styles.items}>
          {presets.map((w) => (
            <WarriorListItem
              key={w.id}
              warrior={w}
              active={w.id === selectedId}
              onSelect={onSelect}
            />
          ))}
        </ul>
        <h3 className={styles.group}>My Warriors</h3>
        {userWarriors.length === 0 ? (
          <p className={styles.empty}>None yet. Duplicate a classic or create a new one.</p>
        ) : (
          <ul className={styles.items}>
            {userWarriors.map((w) => (
              <WarriorListItem
                key={w.id}
                warrior={w}
                active={w.id === selectedId}
                onSelect={onSelect}
              />
            ))}
          </ul>
        )}
        <div className={styles.footer}>
          <button type="button" className={controls.button} onClick={onNew}>
            + New Warrior
          </button>
        </div>
      </CollapsiblePanel>
    </aside>
  );
}
