import { checkPasswordRules, type PasswordRule } from './passwordRules';

/**
 * Live pass/fail checklist for the backend password rules. Shared by the
 * register form (AuthModal) and the change-password form (AccountSettings)
 * so the two can't drift apart from each other or from the server.
 *
 * Accessibility: state is carried by the `[x]` / `[ ]` glyph and a
 * visually-hidden "(satisfied)" suffix, not by color alone.
 */

const CHECKLIST_STYLE: React.CSSProperties = {
  listStyle: 'none',
  padding: 0,
  margin: '0.25rem 0 0 0',
  display: 'flex',
  flexDirection: 'column',
  gap: '0.15rem',
  fontSize: '0.7rem',
};

const VISUALLY_HIDDEN: React.CSSProperties = {
  position: 'absolute',
  width: 1,
  height: 1,
  overflow: 'hidden',
  clip: 'rect(0 0 0 0)',
};

function RuleItem({ rule }: { rule: PasswordRule }) {
  const color = rule.ok ? '#4caf50' : '#e94560';
  return (
    <li style={{ color, display: 'flex', gap: '0.35rem', alignItems: 'baseline' }}>
      <span aria-hidden style={{ fontFamily: 'monospace' }}>
        {rule.ok ? '[x]' : '[ ]'}
      </span>
      <span>{rule.label}</span>
      <span style={VISUALLY_HIDDEN}>{rule.ok ? '(satisfied)' : '(not satisfied)'}</span>
    </li>
  );
}

export default function PasswordChecklist({ id, password }: { id: string; password: string }) {
  const rules = checkPasswordRules(password);
  return (
    <ul id={id} style={CHECKLIST_STYLE} aria-live="polite">
      {rules
        .filter((r) => r.visible)
        .map((r) => (
          <RuleItem key={r.id} rule={r} />
        ))}
    </ul>
  );
}
