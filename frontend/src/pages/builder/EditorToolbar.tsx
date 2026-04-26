import type { Warrior } from '../../warriors/library';
import {
  BUTTON_STYLE,
  DANGER_BUTTON_STYLE,
  INPUT_STYLE,
  PRIMARY_BUTTON_STYLE,
  TOOLBAR_STYLE,
} from './styles';

type Props = {
  label: string;
  selected: Warrior | undefined;
  canSave: boolean;
  onLabelChange: (value: string) => void;
  onSave: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onTestInBattle: () => void;
};

export default function EditorToolbar({
  label,
  selected,
  canSave,
  onLabelChange,
  onSave,
  onDuplicate,
  onDelete,
  onTestInBattle,
}: Props) {
  return (
    <div style={TOOLBAR_STYLE}>
      <input
        style={INPUT_STYLE}
        value={label}
        onChange={(e) => onLabelChange(e.target.value)}
        placeholder="Warrior name"
        disabled={!selected || selected.isPreset}
      />
      {selected && !selected.isPreset && (
        <button
          style={PRIMARY_BUTTON_STYLE}
          onClick={onSave}
          disabled={!canSave}
          title="Save changes"
        >
          Save
        </button>
      )}
      <button style={BUTTON_STYLE} onClick={onDuplicate} disabled={!selected}>
        Duplicate
      </button>
      {selected && !selected.isPreset && (
        <button style={DANGER_BUTTON_STYLE} onClick={onDelete}>
          Delete
        </button>
      )}
      <button style={BUTTON_STYLE} onClick={onTestInBattle} disabled={!selected}>
        Test in Battlefield &rarr;
      </button>
    </div>
  );
}
