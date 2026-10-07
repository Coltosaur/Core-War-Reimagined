import { useLayoutEffect, useRef, useState } from 'react';
import { useAuth } from './useAuth';
import { ApiError } from './client';
import { PASSWORD_MIN_LEN, passwordMeetsRules } from '../auth/passwordRules';
import PasswordChecklist from '../auth/PasswordChecklist';
import alertStyles from '../components/alert.module.css';
import controls from '../components/controls.module.css';
import styles from './AuthModal.module.css';

type Props = { onClose: () => void };

/** `null` = untouched (stay neutral, don't shout before they've typed). */
function validityClass(showValid: boolean | null): string {
  if (showValid === null) return controls.input;
  return `${controls.input} ${showValid ? styles.valid : styles.invalid}`;
}

export default function AuthModal({ onClose }: Props) {
  const { login, register } = useAuth();
  const [mode, setMode] = useState<'login' | 'register'>('login');
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const firstFieldRef = useRef<HTMLInputElement>(null);

  // The parent's mount/unmount is the open/closed state; the native dialog
  // only follows it. showModal() puts the panel in the top layer and makes
  // the rest of the page inert, so Tab can't leave it. A layout effect
  // because its cleanup runs while the <dialog> is still attached: close()
  // on a connected modal dialog is what hands focus back to the opener,
  // whereas a passive cleanup would only see an already-detached node.
  useLayoutEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    dialog.showModal();
    // React's autoFocus calls focus() before showModal() runs, while the
    // closed dialog is still display:none, so it would be a no-op and the
    // dialog would land on its first focusable (the close button) instead.
    firstFieldRef.current?.focus();
    return () => dialog.close();
  }, []);

  // Escape fires `cancel`. Cancel the browser's own close and ask the parent
  // to unmount us instead, so its state stays the single source of truth.
  const handleCancel = (e: React.SyntheticEvent<HTMLDialogElement>) => {
    e.preventDefault();
    onClose();
  };

  // Chrome won't let a page cancel repeated Escapes without fresh user
  // activation, and closes the dialog itself instead. Report that too, so
  // the parent never keeps a mounted-but-closed dialog. `close` is queued,
  // so check `open`: under StrictMode's dev effect re-run the dialog has
  // already been reopened by the time the cleanup's close event arrives.
  const handleNativeClose = (e: React.SyntheticEvent<HTMLDialogElement>) => {
    if (!e.currentTarget.open) onClose();
  };

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
    // No role/aria-modal: a <dialog> opened with showModal() is already
    // exposed as a modal dialog.
    <dialog
      ref={dialogRef}
      className={styles.modal}
      aria-labelledby="auth-modal-title"
      onCancel={handleCancel}
      onClose={handleNativeClose}
    >
      <div className={styles.header}>
        <h2 id="auth-modal-title" className={styles.title}>
          {isRegister ? 'Create Account' : 'Log In'}
        </h2>
        <button onClick={onClose} className={styles.close} aria-label="Close">
          &times;
        </button>
      </div>

      <form onSubmit={handleSubmit} className={styles.form}>
        <label className={styles.field}>
          {isRegister ? 'Username' : 'Username or Email'}
          <input
            className={controls.input}
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            ref={firstFieldRef}
            required
          />
        </label>

        {isRegister && (
          // A <div> rather than a wrapping <label> so the helper text can
          // sit outside the label and describe the field, not name it.
          <div className={styles.field}>
            <label htmlFor="register-email">
              Email <span className={styles.optional}>(optional)</span>
            </label>
            <input
              id="register-email"
              className={controls.input}
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              aria-describedby="register-email-help"
            />
            <span id="register-email-help" className={styles.helper}>
              Not used yet — password recovery and verification arrive later.
            </span>
          </div>
        )}

        <label className={styles.field}>
          Password
          <input
            className={validityClass(isRegister && password.length > 0 ? passwordValid : null)}
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
            <label className={styles.field}>
              Confirm password
              <input
                className={validityClass(confirmPassword.length === 0 ? null : !showMismatch)}
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
              <span id="confirm-mismatch" className={styles.mismatch} role="alert">
                Passwords do not match
              </span>
            )}
          </>
        )}

        {error && (
          <div role="alert" className={alertStyles.error}>
            {error}
          </div>
        )}

        <button
          type="submit"
          disabled={!canSubmit}
          className={`${controls.button} ${controls.primary} ${styles.submit}`}
        >
          {submitting ? '...' : isRegister ? 'Register' : 'Log In'}
        </button>
      </form>

      <div className={styles.toggle}>
        {isRegister ? (
          <>
            Already have an account?{' '}
            <button className={styles.linkButton} onClick={() => switchMode('login')}>
              Log in
            </button>
          </>
        ) : (
          <>
            No account?{' '}
            <button className={styles.linkButton} onClick={() => switchMode('register')}>
              Register
            </button>
          </>
        )}
      </div>
    </dialog>
  );
}
