export const WARRIOR_HEX = ['#888888', '#e94560', '#4fc3f7', '#4caf50', '#ffab00'];

export const ROOT_STYLE: React.CSSProperties = {
  minHeight: '100vh',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  padding: '1.5rem',
  gap: '1rem',
};

export const GRID_CONTAINER_STYLE: React.CSSProperties = {
  border: '1px solid #333',
  lineHeight: 0,
  position: 'relative',
  cursor: 'crosshair',
};

export const TOOLTIP_STYLE: React.CSSProperties = {
  display: 'none',
  position: 'absolute',
  pointerEvents: 'none',
  backgroundColor: 'rgba(0, 0, 0, 0.85)',
  color: '#e0e0e0',
  fontFamily: '"JetBrains Mono", "Fira Code", monospace',
  fontSize: '0.75rem',
  padding: '0.25rem 0.5rem',
  borderRadius: '3px',
  border: '1px solid #444',
  whiteSpace: 'nowrap',
  zIndex: 10,
};

export const CONTROLS_STYLE: React.CSSProperties = {
  display: 'flex',
  gap: '0.5rem',
  alignItems: 'center',
  flexWrap: 'wrap',
  justifyContent: 'center',
};

export const BUTTON_STYLE: React.CSSProperties = {
  padding: '0.4rem 1rem',
  fontFamily: 'inherit',
  fontSize: '0.85rem',
  cursor: 'pointer',
  backgroundColor: '#1e1e1e',
  color: '#e0e0e0',
  border: '1px solid #444',
  borderRadius: '4px',
};

export const SELECT_STYLE: React.CSSProperties = {
  fontFamily: 'inherit',
  fontSize: '0.85rem',
  backgroundColor: '#1e1e1e',
  color: '#e0e0e0',
  border: '1px solid #444',
  borderRadius: '4px',
  padding: '0.3rem 0.5rem',
};

export const STATUS_STYLE: React.CSSProperties = {
  fontSize: '0.85rem',
  color: '#888',
  textAlign: 'center',
};

export const TITLE_STYLE: React.CSSProperties = {
  margin: 0,
  fontSize: '1.5rem',
  letterSpacing: '0.1em',
};

export const PARSE_ERROR_STYLE: React.CSSProperties = {
  padding: '0.5rem 1rem',
  color: '#e94560',
  border: '1px solid #e9456066',
  borderRadius: '4px',
  backgroundColor: '#e9456011',
  fontSize: '0.85rem',
};

export const SPEED_LABEL_STYLE: React.CSSProperties = {
  fontSize: '0.8rem',
  color: '#888',
  display: 'flex',
  alignItems: 'center',
  gap: '0.4rem',
};

export const SPEED_SLIDER_STYLE: React.CSSProperties = {
  verticalAlign: 'middle',
  width: '100px',
};

export const SPEED_INPUT_STYLE: React.CSSProperties = {
  width: '3.5rem',
  fontFamily: 'inherit',
  fontSize: '0.8rem',
  backgroundColor: '#1e1e1e',
  color: '#e0e0e0',
  border: '1px solid #444',
  borderRadius: '3px',
  padding: '0.15rem 0.3rem',
  textAlign: 'right',
};

export const STATUS_ROW_STYLE: React.CSSProperties = {
  marginTop: '0.3rem',
};

export const ENGINE_VERSION_STYLE: React.CSSProperties = {
  marginTop: '0.5rem',
  fontSize: '0.75rem',
  color: '#555',
};

export const ONGOING = 0;
export const VICTORY = 1;
export const TIE = 2;
export const ALL_DEAD = 3;

export function resultBanner(
  code: number,
  winnerId: number,
  names: string[],
): { text: string; color: string } | null {
  switch (code) {
    case VICTORY: {
      const name = names[winnerId] ?? `Warrior ${winnerId}`;
      const color = WARRIOR_HEX[winnerId + 1] ?? '#e0e0e0';
      return { text: `${name} wins!`, color };
    }
    case TIE:
      return { text: 'Tie \u2014 step limit reached', color: '#f0c040' };
    case ALL_DEAD:
      return { text: 'All warriors eliminated \u2014 no winner', color: '#888' };
    default:
      return null;
  }
}
