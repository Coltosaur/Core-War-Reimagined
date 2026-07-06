// Shared style tokens and small formatters for the profile pages. Kept
// separate from ProfileContent.tsx so a component file exports only
// components (react-refresh happy path).

export const SECTION_HEADER: React.CSSProperties = {
  fontSize: '0.7rem',
  letterSpacing: '0.1em',
  color: '#666',
  textTransform: 'uppercase',
  padding: '0.5rem 0',
  borderBottom: '1px solid #222',
  marginBottom: '0.5rem',
};

export const actionLink = (color: string): React.CSSProperties => ({
  padding: '0.5rem 1rem',
  fontSize: '0.8rem',
  color,
  textDecoration: 'none',
  border: `1px solid ${color}66`,
  borderRadius: '4px',
  backgroundColor: `${color}11`,
});

export function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

/**
 * A single item in the warriors list. Only the fields the row actually
 * renders are required — callers can pass either the public-profile shape
 * (with `updated_at`) or the raw warriors-list shape.
 */
export type ProfileWarrior = {
  id: string;
  name: string;
  updated_at: string;
};
