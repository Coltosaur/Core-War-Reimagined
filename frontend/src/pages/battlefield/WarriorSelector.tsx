import type { Warrior } from '../../warriors/library';
import { CONTROLS_STYLE, SELECT_STYLE, WARRIOR_HEX } from './styles';

const SELECTOR_STYLE: React.CSSProperties = {
  ...CONTROLS_STYLE,
  gap: '0.75rem',
};

const RED_LABEL_STYLE: React.CSSProperties = {
  color: WARRIOR_HEX[1],
  fontSize: '0.85rem',
};

const BLUE_LABEL_STYLE: React.CSSProperties = {
  color: WARRIOR_HEX[2],
  fontSize: '0.85rem',
};

const VS_STYLE: React.CSSProperties = {
  color: '#555',
};

type Props = {
  redId: string;
  blueId: string;
  presets: Warrior[];
  userWarriors: Warrior[];
  onPickChange: (side: 0 | 1, id: string) => void;
};

function WarriorDropdown({
  warriors,
  userWarriors,
  value,
  onChange,
}: {
  warriors: Warrior[];
  userWarriors: Warrior[];
  value: string;
  onChange: (id: string) => void;
}) {
  return (
    <select style={SELECT_STYLE} value={value} onChange={(e) => onChange(e.target.value)}>
      <optgroup label="Classic">
        {warriors.map((w) => (
          <option key={w.id} value={w.id}>
            {w.label}
          </option>
        ))}
      </optgroup>
      {userWarriors.length > 0 && (
        <optgroup label="My Warriors">
          {userWarriors.map((w) => (
            <option key={w.id} value={w.id}>
              {w.label}
            </option>
          ))}
        </optgroup>
      )}
    </select>
  );
}

export default function WarriorSelector({
  redId,
  blueId,
  presets,
  userWarriors,
  onPickChange,
}: Props) {
  return (
    <div style={SELECTOR_STYLE}>
      <label style={RED_LABEL_STYLE}>
        Red:{' '}
        <WarriorDropdown
          warriors={presets}
          userWarriors={userWarriors}
          value={redId}
          onChange={(id) => onPickChange(0, id)}
        />
      </label>
      <span style={VS_STYLE}>vs</span>
      <label style={BLUE_LABEL_STYLE}>
        Blue:{' '}
        <WarriorDropdown
          warriors={presets}
          userWarriors={userWarriors}
          value={blueId}
          onChange={(id) => onPickChange(1, id)}
        />
      </label>
    </div>
  );
}
