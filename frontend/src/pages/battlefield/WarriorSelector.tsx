import { Link } from 'react-router-dom';
import type { Warrior } from '../../warriors/library';
import { warriorText } from '../../core/warriorColors';
import controls from '../../components/controls.module.css';

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
    <select className={controls.select} value={value} onChange={(e) => onChange(e.target.value)}>
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

/** Builder URL that opens this warrior; see useBuilder's ?warrior= param. */
function builderLinkFor(id: string): string {
  return `/builder?warrior=${encodeURIComponent(id)}`;
}

// Eight-tooth gear with a center hole (evenodd). Inline rather than a font
// glyph so it renders the same everywhere and takes the link's color.
const COG_PATH =
  'M21.77 9.86L21.77 14.14L19.08 14.15L18.53 15.49L20.42 17.39L17.39 20.42L15.49 18.53' +
  'L14.15 19.08L14.14 21.77L9.86 21.77L9.85 19.08L8.51 18.53L6.61 20.42L3.58 17.39' +
  'L5.47 15.49L4.92 14.15L2.23 14.14L2.23 9.86L4.92 9.85L5.47 8.51L3.58 6.61L6.61 3.58' +
  'L8.51 5.47L9.85 4.92L9.86 2.23L14.14 2.23L14.15 4.92L15.49 5.47L17.39 3.58' +
  'L20.42 6.61L18.53 8.51L19.08 9.85Z M12 8.5a3.5 3.5 0 1 0 0 7a3.5 3.5 0 1 0 0 -7Z';

function EditLink({ id, name }: { id: string; name: string }) {
  const label = `Edit ${name} in the Builder`;
  return (
    <Link to={builderLinkFor(id)} className={controls.iconLink} aria-label={label} title={label}>
      <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" focusable="false">
        <path d={COG_PATH} fill="currentColor" fillRule="evenodd" />
      </svg>
    </Link>
  );
}

const labelFor = (warriors: Warrior[], id: string) =>
  warriors.find((w) => w.id === id)?.label ?? 'warrior';

export default function WarriorSelector({
  redId,
  blueId,
  presets,
  userWarriors,
  onPickChange,
}: Props) {
  return (
    <div className={controls.row}>
      <label className={controls.field} style={{ color: warriorText(0) }}>
        Red
        <WarriorDropdown
          warriors={presets}
          userWarriors={userWarriors}
          value={redId}
          onChange={(id) => onPickChange(0, id)}
        />
      </label>
      <EditLink id={redId} name={labelFor([...presets, ...userWarriors], redId)} />
      <span className={controls.field}>vs</span>
      <label className={controls.field} style={{ color: warriorText(1) }}>
        Green
        <WarriorDropdown
          warriors={presets}
          userWarriors={userWarriors}
          value={blueId}
          onChange={(id) => onPickChange(1, id)}
        />
      </label>
      <EditLink id={blueId} name={labelFor([...presets, ...userWarriors], blueId)} />
    </div>
  );
}
