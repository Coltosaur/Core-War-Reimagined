import { formatInstruction } from '../../core/redcodeFormat';
import { WARRIOR_HEX } from './styles';

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
};

const PANEL_STYLE: React.CSSProperties = {
  width: '260px',
  flexShrink: 0,
  borderLeft: '1px solid #222',
  backgroundColor: '#0d0d0d',
  display: 'flex',
  flexDirection: 'column',
  overflow: 'auto',
  fontSize: '0.8rem',
};

const SECTION_HEADER_STYLE: React.CSSProperties = {
  padding: '0.5rem 0.75rem',
  fontSize: '0.7rem',
  letterSpacing: '0.1em',
  color: '#666',
  textTransform: 'uppercase',
  borderBottom: '1px solid #222',
  backgroundColor: '#111',
};

const CELL_DETAIL_STYLE: React.CSSProperties = {
  padding: '0.5rem 0.75rem',
  borderBottom: '1px solid #222',
};

const ROW_STYLE: React.CSSProperties = {
  display: 'flex',
  justifyContent: 'space-between',
  padding: '0.15rem 0',
};

const LABEL_STYLE: React.CSSProperties = {
  color: '#666',
};

const VALUE_STYLE: React.CSSProperties = {
  color: '#e0e0e0',
};

const INSTR_STYLE: React.CSSProperties = {
  color: '#e94560',
  fontWeight: 600,
  fontSize: '0.85rem',
  padding: '0.3rem 0',
};

const CLEAR_BUTTON_STYLE: React.CSSProperties = {
  background: 'none',
  border: 'none',
  color: '#666',
  cursor: 'pointer',
  fontSize: '0.75rem',
  padding: 0,
};

const PROCESS_ITEM_STYLE: React.CSSProperties = {
  padding: '0.15rem 0.75rem',
  cursor: 'pointer',
  fontSize: '0.75rem',
};

const EMPTY_STYLE: React.CSSProperties = {
  padding: '0.5rem 0.75rem',
  color: '#555',
  fontStyle: 'italic',
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

function CellDetail({ cell, onClear }: { cell: CellInfo; onClear: () => void }) {
  const ownerLabel = cell.owner === 0 ? 'None' : `Warrior ${cell.owner - 1}`;
  const ownerColor = WARRIOR_HEX[cell.owner] ?? '#888';

  return (
    <div style={CELL_DETAIL_STYLE}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span style={{ color: '#4fc3f7' }}>#{String(cell.addr).padStart(4, '0')}</span>
        <button style={CLEAR_BUTTON_STYLE} onClick={onClear}>
          clear
        </button>
      </div>
      <div style={INSTR_STYLE}>
        {formatInstruction(
          cell.opcode,
          cell.modifier,
          cell.aMode,
          cell.aValue,
          cell.bMode,
          cell.bValue,
        )}
      </div>
      <div style={ROW_STYLE}>
        <span style={LABEL_STYLE}>Opcode</span>
        <span style={VALUE_STYLE}>{OPCODE_NAMES[cell.opcode] ?? '???'}</span>
      </div>
      <div style={ROW_STYLE}>
        <span style={LABEL_STYLE}>Modifier</span>
        <span style={VALUE_STYLE}>.{MODIFIER_NAMES[cell.modifier] ?? '?'}</span>
      </div>
      <div style={ROW_STYLE}>
        <span style={LABEL_STYLE}>A-mode</span>
        <span style={VALUE_STYLE}>{MODE_NAMES[cell.aMode] ?? '?'}</span>
      </div>
      <div style={ROW_STYLE}>
        <span style={LABEL_STYLE}>A-value</span>
        <span style={VALUE_STYLE}>{cell.aValue}</span>
      </div>
      <div style={ROW_STYLE}>
        <span style={LABEL_STYLE}>B-mode</span>
        <span style={VALUE_STYLE}>{MODE_NAMES[cell.bMode] ?? '?'}</span>
      </div>
      <div style={ROW_STYLE}>
        <span style={LABEL_STYLE}>B-value</span>
        <span style={VALUE_STYLE}>{cell.bValue}</span>
      </div>
      <div style={ROW_STYLE}>
        <span style={LABEL_STYLE}>Owner</span>
        <span style={{ color: ownerColor }}>{ownerLabel}</span>
      </div>
    </div>
  );
}

export default function InspectorPanel({ cellInfo, warriors, onCellSelect, onClearCell }: Props) {
  return (
    <aside style={PANEL_STYLE}>
      <div style={SECTION_HEADER_STYLE}>Cell Inspector</div>
      {cellInfo ? (
        <CellDetail cell={cellInfo} onClear={onClearCell} />
      ) : (
        <div style={EMPTY_STYLE}>Click a cell in the grid to inspect it.</div>
      )}

      <div style={SECTION_HEADER_STYLE}>Processes</div>
      {warriors.map((w) => (
        <div key={w.warriorIdx}>
          <div
            style={{
              padding: '0.3rem 0.75rem',
              color: WARRIOR_HEX[w.warriorIdx + 1],
              fontWeight: 600,
              fontSize: '0.75rem',
              borderBottom: '1px solid #1a1a1a',
            }}
          >
            {w.name} {w.alive ? `(${w.pcs.length} proc${w.pcs.length !== 1 ? 's' : ''})` : '(dead)'}
          </div>
          {w.alive &&
            w.pcs.map((pc, i) => (
              <div
                key={i}
                style={{
                  ...PROCESS_ITEM_STYLE,
                  color: cellInfo?.addr === pc ? '#e0e0e0' : '#888',
                  backgroundColor: cellInfo?.addr === pc ? '#1a1a1a' : 'transparent',
                }}
                onClick={() => onCellSelect(pc)}
                title={`Process ${i}: PC = ${pc}`}
              >
                [{i}] #{String(pc).padStart(4, '0')}
              </div>
            ))}
        </div>
      ))}
    </aside>
  );
}

export type { ProcessInfo };
