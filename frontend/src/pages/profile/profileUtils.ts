// Small formatters and types shared by the profile pages. Kept separate
// from ProfileContent.tsx so a component file exports only components
// (react-refresh happy path).

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
