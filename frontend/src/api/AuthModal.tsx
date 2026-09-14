import { useState } from 'react';
import { useAuth } from './AuthContext';
import { ApiError } from './client';
import { PASSWORD_MIN_LEN, passwordMeetsRules } from '../auth/passwordRules';
import PasswordChecklist from '../auth/PasswordChecklist';

type Props = { onClose: () => void };

export default function AuthModal({ onClose }: Props) {
  const { login, register } = useAuth();
  const [mode, setMode] = useState<'login' | 'register'>('login');
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const isRegister = mode === 'register';
  const passwordValid = passwordMeetsRules(password);
  // Only complain about a mismatch once the user has actually started the
  // confirm field — otherwise the error flashes on every keystroke of the
  // first password box.
  const showMismatch = isRegister && confirmPassword.length > 0 && password !== confirmPassword;
  const canSubmit =
    !submitting &&
    username.length > 0 &&
    password.length > 0 &&
    // Login must accept whatever the user already has: accounts created
    // before the 12-char minimum still need to be able to sign in.
    (!isRegister || (passwordValid && password === confirmPassword));

  const switchMode = (next: 'login' | 'register') => {
    setMode(next);
    setError('');
    setConfirmPassword('');
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSubmit) return;
    setError('');
    setSubmitting(true);
    try {
      if (!isRegister) {
        await login({ username_or_email: username, password });
      } else {
        const trimmedEmail = email.trim();
        await register({
          username,
          password,
          ...(trimmedEmail ? { email: trimmedEmail } : {}),
        });
      }
      onClose();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Something went wrong');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div style={OVERLAY}>
      <div style={MODAL}>
        <div style={HEADER}>
          <h2 style={{ margin: 0, fontSize: '1.1rem' }}>
            {isRegister ? 'Create Account' : 'Log In'}
          </h2>
          <button onClick={onClose} style={CLOSE_BTN} aria-label="Close">
            &times;
          </button>
        </div>

        <form onSubmit={handleSubmit} style={FORM}>
          <label style={LABEL}>
            {isRegister ? 'Username' : 'Username or Email'}
            <input
              style={INPUT_BASE}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoFocus
              required
            />
          </label>

          {isRegister && (
            <label style={LABEL}>
              Email <span style={OPTIONAL_HINT}>(optional)</span>
              <input
                style={INPUT_BASE}
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
              <span style={HELPER_TEXT}>
                Not used yet — password recovery and verification arrive later.
              </span>
            </label>
          )}

          <label style={LABEL}>
            Password
            <input
              style={
                isRegister ? inputStyle(password.length === 0 ? null : passwordValid) : INPUT_BASE
              }
              type="password"
              autoComplete={isRegister ? 'new-password' : 'current-password'}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              minLength={isRegister ? PASSWORD_MIN_LEN : undefined}
              aria-describedby={isRegister ? 'register-password-rules' : undefined}
            />
          </label>
          {/* Checklist and mismatch error sit outside their <label> on purpose:
              nested inside, their text joins the input's accessible name and a
              screen reader announces every rule as part of the field label. */}
          {isRegister && <PasswordChecklist id="register-password-rules" password={password} />}

          {isRegister && (
            <>
              <label style={LABEL}>
                Confirm password
                <input
                  style={inputStyle(confirmPassword.length === 0 ? null : !showMismatch)}
                  type="password"
                  autoComplete="new-password"
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  required
                  minLength={PASSWORD_MIN_LEN}
                  aria-describedby={showMismatch ? 'confirm-mismatch' : undefined}
                />
              </label>
              {showMismatch && (
                <span id="confirm-mismatch" style={ERROR} role="alert">
                  Passwords do not match
                </span>
              )}
            </>
          )}

          {error && <div style={ERROR}>{error}</div>}

          <button
            type="submit"
            disabled={!canSubmit}
            style={canSubmit ? SUBMIT_BTN : SUBMIT_DISABLED}
          >
            {submitting ? '...' : isRegister ? 'Register' : 'Log In'}
          </button>
        </form>

        <div style={TOGGLE}>
          {isRegister ? (
            <>
              Already have an account?{' '}
              <button style={LINK_BTN} onClick={() => switchMode('login')}>
                Log in
              </button>
            </>
          ) : (
            <>
              No account?{' '}
              <button style={LINK_BTN} onClick={() => switchMode('register')}>
                Register
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

const OVERLAY: React.CSSProperties = {
  position: 'fixed',
  inset: 0,
  backgroundColor: 'rgba(0,0,0,0.6)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  zIndex: 1000,
};

const MODAL: React.CSSProperties = {
  backgroundColor: '#1a1a1a',
  border: '1px solid #333',
  borderRadius: '8px',
  padding: '1.5rem',
  width: '340px',
  maxWidth: '90vw',
  maxHeight: '90vh',
  overflowY: 'auto',
  fontFamily: '"JetBrains Mono", "Fira Code", monospace',
};

const HEADER: React.CSSProperties = {
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
  marginBottom: '1rem',
};

const CLOSE_BTN: React.CSSProperties = {
  background: 'none',
  border: 'none',
  color: '#888',
  fontSize: '1.5rem',
  cursor: 'pointer',
  padding: '0 0.25rem',
};

const FORM: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: '0.75rem',
};

const LABEL: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: '0.25rem',
  fontSize: '0.8rem',
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

/** `null` = untouched (stay neutral, don't shout before they've typed). */
function inputStyle(showValid: boolean | null): React.CSSProperties {
  if (showValid === true) return { ...INPUT_BASE, borderColor: '#4caf5066' };
  if (showValid === false) return { ...INPUT_BASE, borderColor: '#e9456099' };
  return INPUT_BASE;
}

const ERROR: React.CSSProperties = {
  color: '#e94560',
  fontSize: '0.8rem',
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

const TOGGLE: React.CSSProperties = {
  marginTop: '1rem',
  textAlign: 'center',
  fontSize: '0.75rem',
  color: '#888',
};

const OPTIONAL_HINT: React.CSSProperties = {
  color: '#666',
  textTransform: 'none',
  fontSize: '0.7rem',
};

const HELPER_TEXT: React.CSSProperties = {
  color: '#666',
  fontSize: '0.65rem',
  marginTop: '0.15rem',
};

const LINK_BTN: React.CSSProperties = {
  background: 'none',
  border: 'none',
  color: '#4fc3f7',
  cursor: 'pointer',
  fontFamily: 'inherit',
  fontSize: '0.75rem',
  textDecoration: 'underline',
  padding: 0,
};
