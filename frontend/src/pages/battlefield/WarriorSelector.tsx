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

function EditLink({ id, name }: { id: string; name: string }) {
  return (
    <Link
      to={builderLinkFor(id)}
      className={controls.link}
      aria-label={`Edit ${name} in the Builder`}
    >
      Edit
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
