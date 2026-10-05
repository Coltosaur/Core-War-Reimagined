import type { Warrior } from '../../warriors/library';
import type { ParseStatus } from './useBuilder';
import styles from './EditorStatus.module.css';

type Props = {
  wasmReady: boolean;
  parseStatus: ParseStatus;
  selected: Warrior | undefined;
  dirty: boolean;
};

export default function EditorStatus({ wasmReady, parseStatus, selected, dirty }: Props) {
  return (
    <div className={styles.status} role="status">
      {!wasmReady && <span className={styles.loading}>Loading engine...</span>}
      {wasmReady && parseStatus?.ok && (
        <span className={styles.ok}>
          ✓ parsed
          {parseStatus.name ? ` — ${parseStatus.name}` : ''}
        </span>
      )}
      {wasmReady && parseStatus && !parseStatus.ok && (
        <span className={styles.error}>✗ {parseStatus.message}</span>
      )}
      {selected?.isPreset && (
        <span className={styles.hint}>Classic warrior — read-only. Use Duplicate to edit.</span>
      )}
      {dirty && !selected?.isPreset && <span className={styles.unsaved}>● unsaved changes</span>}
    </div>
  );
}
