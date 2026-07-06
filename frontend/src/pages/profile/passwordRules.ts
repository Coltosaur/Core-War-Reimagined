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

export function checkPasswordRules(pw: string): PasswordRule[] {
  const hasNonDigit = /[^0-9]/.test(pw);
  return [
    {
      id: 'length',
      label: `At least ${PASSWORD_MIN_LEN} characters`,
      ok: pw.length >= PASSWORD_MIN_LEN,
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
      ok: pw.length <= PASSWORD_MAX_LEN,
      visible: pw.length > PASSWORD_MAX_LEN,
    },
  ];
}

export function passwordMeetsRules(pw: string): boolean {
  return checkPasswordRules(pw).every((r) => r.ok);
}
