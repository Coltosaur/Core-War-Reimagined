import { formatInstruction } from '../../core/redcodeFormat';
import CollapsiblePanel from '../../components/CollapsiblePanel';
import { NEUTRAL_TEXT, warriorText } from '../../core/warriorColors';
import styles from './InspectorPanel.module.css';

export type CellInfo = {
  addr: number;
  opcode: number;
  modifier: number;
  aMode: number;
  aValue: number;
  bMode: number;
  bValue: number;
  owner: number;
};

type ProcessInfo = {
  warriorIdx: number;
  name: string;
  alive: boolean;
  pcs: number[];
};

type Props = {
  cellInfo: CellInfo | null;
  warriors: ProcessInfo[];
  onCellSelect: (addr: number) => void;
  onClearCell: () => void;
  className?: string;
};

const OPCODE_NAMES = [
  'DAT',
  'MOV',
  'ADD',
  'SUB',
  'MUL',
  'DIV',
  'MOD',
  'JMP',
  'JMZ',
  'JMN',
  'DJN',
  'SPL',
  'SLT',
  'SEQ',
  'SNE',
  'NOP',
];

const MODIFIER_NAMES = ['A', 'B', 'AB', 'BA', 'F', 'X', 'I'];

const MODE_NAMES = [
  'Immediate (#)',
  'Direct ($)',
  'A-Indirect (*)',
  'B-Indirect (@)',
  'A-Predec ({)',
  'B-Predec (<)',
  'A-Postinc (})',
  'B-Postinc (>)',
];

const pad4 = (n: number) => String(n).padStart(4, '0');

function CellDetail({ cell, onClear }: { cell: CellInfo; onClear: () => void }) {
  const ownerLabel = cell.owner === 0 ? 'None' : `Warrior ${cell.owner - 1}`;
  const ownerColor = cell.owner === 0 ? NEUTRAL_TEXT : warriorText(cell.owner - 1);
  const rows: [string, string | number][] = [
    ['Opcode', OPCODE_NAMES[cell.opcode] ?? '???'],
    ['Modifier', `.${MODIFIER_NAMES[cell.modifier] ?? '?'}`],
    ['A-mode', MODE_NAMES[cell.aMode] ?? '?'],
    ['A-value', cell.aValue],
    ['B-mode', MODE_NAMES[cell.bMode] ?? '?'],
    ['B-value', cell.bValue],
  ];

  return (
    <div className={styles.detail}>
      <div className={styles.detailHeader}>
        <span className={styles.addr}>#{pad4(cell.addr)}</span>
        <button type="button" className={styles.clear} onClick={onClear}>
          clear
        </button>
      </div>
      <div className={styles.instruction}>
        {formatInstruction(
          cell.opcode,
          cell.modifier,
          cell.aMode,
          cell.aValue,
          cell.bMode,
          cell.bValue,
        )}
      </div>
      {rows.map(([label, value]) => (
        <div key={label} className={styles.row}>
          <span className={styles.label}>{label}</span>
          <span>{value}</span>
        </div>
      ))}
      <div className={styles.row}>
        <span className={styles.label}>Owner</span>
        <span style={{ color: ownerColor }}>{ownerLabel}</span>
      </div>
    </div>
  );
}

export default function InspectorPanel({
  cellInfo,
  warriors,
  onCellSelect,
  onClearCell,
  className,
}: Props) {
  return (
    <aside
      className={className ? `${styles.inspector} ${className}` : styles.inspector}
      aria-label="Inspector"
    >
      <CollapsiblePanel title="Cell Inspector" defaultOpen>
        {cellInfo ? (
          <CellDetail cell={cellInfo} onClear={onClearCell} />
        ) : (
          <p className={styles.empty}>Tap or click a cell in the grid to inspect it.</p>
        )}
      </CollapsiblePanel>

      <CollapsiblePanel title="Processes">
        {warriors.map((w) => (
          <div key={w.warriorIdx}>
            <h3 className={styles.warrior} style={{ color: warriorText(w.warriorIdx) }}>
              {w.name}{' '}
              {w.alive ? `(${w.pcs.length} proc${w.pcs.length !== 1 ? 's' : ''})` : '(dead)'}
            </h3>
            {w.alive && (
              <ul className={styles.pcList}>
                {w.pcs.map((pc, i) => (
                  <li key={i}>
                    <button
                      type="button"
                      className={
                        cellInfo?.addr === pc ? `${styles.pc} ${styles.pcSelected}` : styles.pc
                      }
                      aria-current={cellInfo?.addr === pc ? 'true' : undefined}
                      onClick={() => onCellSelect(pc)}
                      title={`Process ${i}: PC = ${pc}`}
                    >
                      [{i}] #{pad4(pc)}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        ))}
      </CollapsiblePanel>
    </aside>
  );
}

export type { ProcessInfo };
