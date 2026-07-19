import { useEffect, useState } from 'react';
import { useAuth } from '../../api/AuthContext';
import { ApiError } from '../../api/client';
import { getAccount, changePassword } from '../../api/account';
import { SECTION_HEADER } from './profileStyles';
import {
  PASSWORD_MIN_LEN,
  checkPasswordRules,
  passwordMeetsRules,
  type PasswordRule,
} from './passwordRules';

/**
 * Own-profile-only account settings panel. Renders email, change-password
 * form, logout button, and "coming soon" affordances for change-email and
 * delete-account. Deliberately absent from the public `/users/:username`
 * view — presence of this panel on the wrong route is a bug.
 */

// --- styles ---

const PANEL_STYLE: React.CSSProperties = {
  marginTop: '2rem',
};

const ROW_STYLE: React.CSSProperties = {
  display: 'flex',
  alignItems: 'baseline',
  justifyContent: 'space-between',
  padding: '0.5rem 0',
  borderBottom: '1px solid #1a1a1a',
};

const LABEL_STYLE: React.CSSProperties = {
  fontSize: '0.7rem',
  color: '#888',
  textTransform: 'uppercase',
  letterSpacing: '0.08em',
};

const VALUE_STYLE: React.CSSProperties = {
  color: '#e0e0e0',
  fontSize: '0.85rem',
};

const FORM_STYLE: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: '0.75rem',
  padding: '1rem 0',
};

const FIELD_LABEL: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: '0.25rem',
  fontSize: '0.75rem',
  color: '#aaa',
};

// Split into borderWidth/Style/Color instead of the `border` shorthand so
// per-instance overrides of borderColor don't collide with the shorthand
// (React logs a warning on that pattern during rerender).
const INPUT_BASE: React.CSSProperties = {
  backgroundColor: '#111',
  borderWidth: '1px',
  borderStyle: 'solid',
  borderColor: '#333',
  borderRadius: '4px',
  padding: '0.5rem',
  color: '#e0e0e0',
  fontSize: '0.85rem',
  fontFamily: 'inherit',
  outline: 'none',
};

const CHECKLIST_STYLE: React.CSSProperties = {
  listStyle: 'none',
  padding: 0,
  margin: '0.25rem 0 0 0',
  display: 'flex',
  flexDirection: 'column',
  gap: '0.15rem',
  fontSize: '0.7rem',
};

const SUBMIT_BTN: React.CSSProperties = {
  backgroundColor: '#e94560',
  color: '#fff',
  border: 'none',
  borderRadius: '4px',
  padding: '0.6rem',
  fontSize: '0.85rem',
  fontFamily: 'inherit',
  cursor: 'pointer',
  marginTop: '0.25rem',
};

const SUBMIT_DISABLED: React.CSSProperties = {
  ...SUBMIT_BTN,
  backgroundColor: '#333',
  color: '#666',
  cursor: 'not-allowed',
};

const ERROR_STYLE: React.CSSProperties = {
  color: '#e94560',
  fontSize: '0.75rem',
};

const SUCCESS_STYLE: React.CSSProperties = {
  color: '#4caf50',
  fontSize: '0.75rem',
};

const LOGOUT_BTN: React.CSSProperties = {
  backgroundColor: 'transparent',
  color: '#e94560',
  border: '1px solid #e9456066',
  borderRadius: '4px',
  padding: '0.5rem 1rem',
  fontSize: '0.8rem',
  fontFamily: 'inherit',
  cursor: 'pointer',
  marginTop: '0.5rem',
};

const COMING_SOON_GRID: React.CSSProperties = {
  display: 'flex',
  gap: '0.75rem',
  padding: '0.75rem 0',
  flexWrap: 'wrap',
};

const COMING_SOON_TILE: React.CSSProperties = {
  flex: '1 1 200px',
  padding: '0.75rem',
  border: '1px dashed #333',
  borderRadius: '6px',
  backgroundColor: '#0d0d0d',
  color: '#555',
};

const COMING_SOON_TITLE: React.CSSProperties = {
  fontSize: '0.85rem',
  color: '#888',
  marginBottom: '0.15rem',
};

const COMING_SOON_TAG: React.CSSProperties = {
  fontSize: '0.6rem',
  letterSpacing: '0.1em',
  textTransform: 'uppercase',
  color: '#e9456099',
};

function inputStyle(showValid: boolean | null): React.CSSProperties {
  if (showValid === true) return { ...INPUT_BASE, borderColor: '#4caf5066' };
  if (showValid === false) return { ...INPUT_BASE, borderColor: '#e9456099' };
  return INPUT_BASE;
}

function RuleItem({ rule }: { rule: PasswordRule }) {
  const color = rule.ok ? '#4caf50' : '#e94560';
  return (
    <li style={{ color, display: 'flex', gap: '0.35rem', alignItems: 'baseline' }}>
      <span aria-hidden style={{ fontFamily: 'monospace' }}>
        {rule.ok ? '[x]' : '[ ]'}
      </span>
      <span>{rule.label}</span>
      <span
        style={{
          position: 'absolute',
          width: 1,
          height: 1,
          overflow: 'hidden',
          clip: 'rect(0 0 0 0)',
        }}
      >
        {rule.ok ? '(satisfied)' : '(not satisfied)'}
      </span>
    </li>
  );
}

export default function AccountSettings() {
  const { logout } = useAuth();
  // Three states: `undefined` = still loading, `null` = loaded, no email on
  // file, `string` = loaded, has email. Collapsing loading + not-set to a
  // single `null` (as before) meant "Not set" flashed during load.
  const [email, setEmail] = useState<string | null | undefined>(undefined);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [formSuccess, setFormSuccess] = useState(false);

  useEffect(() => {
    let cancelled = false;
    getAccount()
      .then((a) => {
        if (!cancelled) setEmail(a.email);
      })
      .catch((e) => {
        if (!cancelled) setLoadError(e instanceof Error ? e.message : 'Failed to load account');
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const rules = checkPasswordRules(newPassword);
  const newPasswordValid = passwordMeetsRules(newPassword);
  const canSubmit =
    !submitting &&
    currentPassword.length > 0 &&
    newPassword.length > 0 &&
    newPasswordValid &&
    currentPassword !== newPassword;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setFormError(null);
    setFormSuccess(false);
    if (!canSubmit) return;
    setSubmitting(true);
    try {
      await changePassword({
        current_password: currentPassword,
        new_password: newPassword,
      });
      setCurrentPassword('');
      setNewPassword('');
      setFormSuccess(true);
    } catch (err) {
      if (err instanceof ApiError) {
        if (err.status === 429) {
          setFormError('Too many attempts. Try again in a few minutes.');
        } else {
          setFormError(err.message);
        }
      } else {
        setFormError('Something went wrong');
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <section style={PANEL_STYLE} aria-label="Account settings">
      <div style={SECTION_HEADER}>Account Settings</div>

      <div style={ROW_STYLE}>
        <span style={LABEL_STYLE}>Email</span>
        <span style={VALUE_STYLE} data-testid="account-email">
          {loadError ? (
            <span style={ERROR_STYLE}>{loadError}</span>
          ) : email === undefined ? (
            <span style={{ color: '#666' }}>Loading...</span>
          ) : email === null ? (
            <span style={{ color: '#666', fontStyle: 'italic' }}>Not set</span>
          ) : (
            email
          )}
        </span>
      </div>

      <form onSubmit={handleSubmit} style={FORM_STYLE} aria-label="Change password">
        <label style={FIELD_LABEL}>
          Current password
          <input
            style={INPUT_BASE}
            type="password"
            autoComplete="current-password"
            value={currentPassword}
            onChange={(e) => {
              setCurrentPassword(e.target.value);
              setFormError(null);
              setFormSuccess(false);
            }}
          />
        </label>

        <label style={FIELD_LABEL}>
          New password
          <input
            style={inputStyle(newPassword.length === 0 ? null : newPasswordValid)}
            type="password"
            autoComplete="new-password"
            value={newPassword}
            onChange={(e) => {
              setNewPassword(e.target.value);
              setFormError(null);
              setFormSuccess(false);
            }}
            minLength={PASSWORD_MIN_LEN}
            aria-describedby="password-rules"
          />
          <ul id="password-rules" style={CHECKLIST_STYLE} aria-live="polite">
            {rules
              .filter((r) => r.visible)
              .map((r) => (
                <RuleItem key={r.id} rule={r} />
              ))}
          </ul>
        </label>

        {formError && <div style={ERROR_STYLE}>{formError}</div>}
        {formSuccess && (
          <div style={SUCCESS_STYLE}>Password updated. Other sessions have been signed out.</div>
        )}

        <button
          type="submit"
          disabled={!canSubmit}
          style={canSubmit ? SUBMIT_BTN : SUBMIT_DISABLED}
        >
          {submitting ? '...' : 'Change password'}
        </button>
      </form>

      <div style={SECTION_HEADER}>Session</div>
      <button style={LOGOUT_BTN} onClick={() => void logout()}>
        Log out
      </button>

      <div style={SECTION_HEADER}>Coming soon</div>
      <div style={COMING_SOON_GRID}>
        <div style={COMING_SOON_TILE} aria-disabled>
          <div style={COMING_SOON_TITLE}>Change email</div>
          <span style={COMING_SOON_TAG}>Coming soon</span>
        </div>
        <div style={COMING_SOON_TILE} aria-disabled>
          <div style={COMING_SOON_TITLE}>Delete account</div>
          <span style={COMING_SOON_TAG}>Coming soon</span>
        </div>
      </div>
    </section>
  );
}
