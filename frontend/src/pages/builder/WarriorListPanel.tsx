import type { Warrior } from '../../warriors/library';
import WarriorListItem from './WarriorListItem';
import {
  EMPTY_STATE_STYLE,
  LIST_FOOTER_STYLE,
  LIST_HEADER_STYLE,
  LIST_STYLE,
  NEW_BUTTON_STYLE,
} from './styles';

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
    <aside style={LIST_STYLE}>
      <div style={LIST_HEADER_STYLE}>Classic (read-only)</div>
      {presets.map((w) => (
        <WarriorListItem key={w.id} warrior={w} active={w.id === selectedId} onSelect={onSelect} />
      ))}
      <div style={LIST_HEADER_STYLE}>My Warriors</div>
      {userWarriors.length === 0 && (
        <div style={EMPTY_STATE_STYLE}>None yet. Duplicate a classic or create a new one.</div>
      )}
      {userWarriors.map((w) => (
        <WarriorListItem key={w.id} warrior={w} active={w.id === selectedId} onSelect={onSelect} />
      ))}
      <div style={LIST_FOOTER_STYLE}>
        <button style={NEW_BUTTON_STYLE} onClick={onNew}>
          + New Warrior
        </button>
      </div>
    </aside>
  );
}
