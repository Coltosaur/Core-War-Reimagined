export const PAGE_STYLE: React.CSSProperties = {
  display: 'flex',
  height: '100vh',
  minHeight: 0,
};

export const LIST_STYLE: React.CSSProperties = {
  width: '240px',
  flexShrink: 0,
  borderRight: '1px solid #222',
  display: 'flex',
  flexDirection: 'column',
  backgroundColor: '#0d0d0d',
};

export const LIST_HEADER_STYLE: React.CSSProperties = {
  padding: '0.75rem 1rem',
  fontSize: '0.7rem',
  letterSpacing: '0.1em',
  color: '#666',
  textTransform: 'uppercase',
  borderBottom: '1px solid #222',
};

export const MAIN_STYLE: React.CSSProperties = {
  flex: 1,
  minWidth: 0,
  display: 'flex',
  flexDirection: 'column',
};

export const TOOLBAR_STYLE: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: '0.5rem',
  padding: '0.75rem 1rem',
  borderBottom: '1px solid #222',
  backgroundColor: '#111',
};

export const BUTTON_STYLE: React.CSSProperties = {
  padding: '0.4rem 0.9rem',
  fontFamily: 'inherit',
  fontSize: '0.8rem',
  cursor: 'pointer',
  backgroundColor: '#1e1e1e',
  color: '#e0e0e0',
  border: '1px solid #444',
  borderRadius: '4px',
};

export const PRIMARY_BUTTON_STYLE: React.CSSProperties = {
  ...BUTTON_STYLE,
  backgroundColor: '#e9456022',
  borderColor: '#e9456066',
  color: '#e94560',
};

export const DANGER_BUTTON_STYLE: React.CSSProperties = {
  ...BUTTON_STYLE,
  color: '#e94560',
  borderColor: '#e9456066',
};

export const INPUT_STYLE: React.CSSProperties = {
  fontFamily: 'inherit',
  fontSize: '0.85rem',
  backgroundColor: '#1e1e1e',
  color: '#e0e0e0',
  border: '1px solid #444',
  borderRadius: '4px',
  padding: '0.35rem 0.6rem',
  flex: 1,
  minWidth: 0,
};

export const STATUS_STYLE: React.CSSProperties = {
  padding: '0.5rem 1rem',
  fontSize: '0.8rem',
  borderTop: '1px solid #222',
  backgroundColor: '#0d0d0d',
  minHeight: '2rem',
  display: 'flex',
  alignItems: 'center',
};

export const EMPTY_STATE_STYLE: React.CSSProperties = {
  padding: '0.5rem 1rem',
  fontSize: '0.8rem',
  color: '#555',
  fontStyle: 'italic',
};

export const LIST_FOOTER_STYLE: React.CSSProperties = {
  padding: '0.75rem 1rem',
  marginTop: 'auto',
  borderTop: '1px solid #222',
};

export const NEW_BUTTON_STYLE: React.CSSProperties = {
  ...BUTTON_STYLE,
  width: '100%',
};

export const EDITOR_CONTAINER_STYLE: React.CSSProperties = {
  flex: 1,
  minHeight: 0,
};

export const listItemStyle = (active: boolean, preset: boolean): React.CSSProperties => ({
  padding: '0.5rem 1rem',
  cursor: 'pointer',
  fontSize: '0.85rem',
  color: preset ? '#bbb' : '#e0e0e0',
  backgroundColor: active ? '#1a1a1a' : 'transparent',
  borderLeft: active ? '3px solid #e94560' : '3px solid transparent',
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
  gap: '0.5rem',
});

export const LIST_ITEM_LABEL_STYLE: React.CSSProperties = {
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
};

export const CLASSIC_BADGE_STYLE: React.CSSProperties = {
  fontSize: '0.65rem',
  color: '#666',
};
