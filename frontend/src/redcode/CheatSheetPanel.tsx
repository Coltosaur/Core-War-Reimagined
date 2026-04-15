import { useState } from 'react';
import {
  OPCODES,
  MODIFIERS,
  ADDRESSING_MODES,
  PSEUDO_OPS,
  type CheatEntry,
} from './cheatSheet';

const PANEL_STYLE: React.CSSProperties = {
  flexShrink: 0,
  borderLeft: '1px solid #222',
  backgroundColor: '#0d0d0d',
  display: 'flex',
  flexDirection: 'column',
  overflow: 'hidden',
  transition: 'width 0.15s',
};

const HEADER_STYLE: React.CSSProperties = {
  padding: '0.6rem 0.9rem',
  fontSize: '0.75rem',
  letterSpacing: '0.08em',
  color: '#bbb',
  textTransform: 'uppercase',
  borderBottom: '1px solid #222',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  cursor: 'pointer',
  userSelect: 'none',
};

const SECTION_STYLE: React.CSSProperties = {
  padding: '0.4rem 0',
  borderBottom: '1px solid #1a1a1a',
};

const SECTION_HEADER_STYLE: React.CSSProperties = {
  padding: '0.3rem 0.9rem',
  fontSize: '0.65rem',
  letterSpacing: '0.1em',
  color: '#666',
  textTransform: 'uppercase',
};

const ROW_STYLE: React.CSSProperties = {
  padding: '0.3rem 0.9rem',
  display: 'grid',
  gridTemplateColumns: '3rem 1fr',
  gap: '0.5rem',
  fontSize: '0.8rem',
  alignItems: 'baseline',
};

const SYMBOL_STYLE: React.CSSProperties = {
  color: '#e94560',
  fontWeight: 600,
  textAlign: 'right',
};

const DESC_STYLE: React.CSSProperties = {
  color: '#bbb',
  lineHeight: 1.4,
};

const NAME_STYLE: React.CSSProperties = {
  color: '#4fc3f7',
  fontSize: '0.7rem',
  textTransform: 'lowercase',
  marginLeft: '0.3rem',
};

const TOGGLE_STYLE: React.CSSProperties = {
  background: 'transparent',
  color: '#888',
  border: '1px solid #333',
  borderRadius: '4px',
  padding: '0.15rem 0.5rem',
  fontSize: '0.7rem',
  cursor: 'pointer',
  fontFamily: 'inherit',
};

type SectionProps = { title: string; entries: CheatEntry[] };

function Section({ title, entries }: SectionProps) {
  return (
    <div style={SECTION_STYLE}>
      <div style={SECTION_HEADER_STYLE}>{title}</div>
      {entries.map((e) => (
        <div key={e.symbol} style={ROW_STYLE}>
          <span style={SYMBOL_STYLE}>{e.symbol}</span>
          <span>
            <span style={DESC_STYLE}>{e.desc}</span>
            <span style={NAME_STYLE}>{e.name}</span>
          </span>
        </div>
      ))}
    </div>
  );
}

export default function CheatSheetPanel() {
  const [open, setOpen] = useState(true);

  if (!open) {
    return (
      <div style={{ ...PANEL_STYLE, width: '36px', cursor: 'pointer' }} onClick={() => setOpen(true)} title="Show cheat sheet">
        <div
          style={{
            writingMode: 'vertical-rl',
            transform: 'rotate(180deg)',
            padding: '0.75rem 0',
            fontSize: '0.7rem',
            letterSpacing: '0.15em',
            color: '#888',
            textTransform: 'uppercase',
            textAlign: 'center',
          }}
        >
          Cheat Sheet
        </div>
      </div>
    );
  }

  return (
    <div style={{ ...PANEL_STYLE, width: '320px' }}>
      <div style={HEADER_STYLE} onClick={() => setOpen(false)}>
        <span>Redcode Cheat Sheet</span>
        <button
          style={TOGGLE_STYLE}
          onClick={(e) => {
            e.stopPropagation();
            setOpen(false);
          }}
        >
          hide
        </button>
      </div>
      <div style={{ overflowY: 'auto', flex: 1 }}>
        <Section title="Opcodes" entries={OPCODES} />
        <Section title="Modifiers" entries={MODIFIERS} />
        <Section title="Addressing Modes" entries={ADDRESSING_MODES} />
        <Section title="Pseudo-ops" entries={PSEUDO_OPS} />
      </div>
    </div>
  );
}
