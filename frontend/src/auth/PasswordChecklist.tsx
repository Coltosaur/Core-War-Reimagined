import { checkPasswordRules, type PasswordRule } from './passwordRules';
import styles from './PasswordChecklist.module.css';

/**
 * Live pass/fail checklist for the backend password rules. Shared by the
 * register form (AuthModal) and the change-password form (AccountSettings)
 * so the two can't drift apart from each other or from the server.
 *
 * Accessibility: state is carried by the `[x]` / `[ ]` glyph and a
 * visually-hidden "(satisfied)" suffix, not by color alone.
 */

function RuleItem({ rule }: { rule: PasswordRule }) {
  return (
    <li className={rule.ok ? `${styles.rule} ${styles.ok}` : styles.rule}>
      <span aria-hidden>{rule.ok ? '[x]' : '[ ]'}</span>
      <span>{rule.label}</span>
      <span className={styles.visuallyHidden}>{rule.ok ? '(satisfied)' : '(not satisfied)'}</span>
    </li>
  );
}

export default function PasswordChecklist({ id, password }: { id: string; password: string }) {
  const rules = checkPasswordRules(password);
  return (
    <ul id={id} className={styles.list} aria-live="polite">
      {rules
        .filter((r) => r.visible)
        .map((r) => (
          <RuleItem key={r.id} rule={r} />
        ))}
    </ul>
  );
}
