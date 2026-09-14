// Password rules mirror backend `validate_password` in auth/handlers.rs.
// Keep the numbers in sync: bumping the backend minimum without updating
// the checklist here means users see a green checkmark then get rejected
// on submit — the exact confusion the checklist was added to avoid.
export const PASSWORD_MIN_LEN = 12;
export const PASSWORD_MAX_LEN = 1000;

export type PasswordRuleId = 'length' | 'non_digit' | 'too_long';

export type PasswordRule = {
  id: PasswordRuleId;
  label: string;
  ok: boolean;
  /**
   * Some rules only matter when violated (e.g. "≤ 1000 chars"). We hide
   * those in the happy path to keep the checklist short and readable.
   */
  visible: boolean;
};

const encoder = new TextEncoder();

/**
 * Backend length checks are `str::len()` — UTF-8 *bytes*, not characters.
 * JS `String.length` counts UTF-16 code units, so the two disagree on any
 * non-ASCII input ("é" is 2 bytes but 1 unit; "🦖" is 4 bytes but 2 units).
 * Measuring bytes here keeps the checklist from failing a password the
 * server would have accepted.
 */
export function passwordByteLength(pw: string): number {
  return encoder.encode(pw).length;
}

export function checkPasswordRules(pw: string): PasswordRule[] {
  const byteLen = passwordByteLength(pw);
  const hasNonDigit = /[^0-9]/.test(pw);
  return [
    {
      id: 'length',
      label: `At least ${PASSWORD_MIN_LEN} characters`,
      ok: byteLen >= PASSWORD_MIN_LEN,
      visible: true,
    },
    {
      id: 'non_digit',
      label: 'Contains at least one non-digit character',
      ok: hasNonDigit,
      visible: true,
    },
    {
      id: 'too_long',
      label: `At most ${PASSWORD_MAX_LEN} characters`,
      ok: byteLen <= PASSWORD_MAX_LEN,
      visible: byteLen > PASSWORD_MAX_LEN,
    },
  ];
}

export function passwordMeetsRules(pw: string): boolean {
  return checkPasswordRules(pw).every((r) => r.ok);
}
