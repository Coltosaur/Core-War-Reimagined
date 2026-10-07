import { useEffect, useState } from 'react';
import { useAuth } from '../../api/useAuth';
import { ApiError } from '../../api/client';
import { getAccount, changePassword } from '../../api/account';
import alertStyles from '../../components/alert.module.css';
import controls from '../../components/controls.module.css';
import profileStyles from './Profile.module.css';
import styles from './AccountSettings.module.css';
import { PASSWORD_MIN_LEN, passwordMeetsRules } from '../../auth/passwordRules';
import PasswordChecklist from '../../auth/PasswordChecklist';

/**
 * Own-profile-only account settings panel. Renders email, change-password
 * form, logout button, and "coming soon" affordances for change-email and
 * delete-account. Deliberately absent from the public `/users/:username`
 * view — presence of this panel on the wrong route is a bug.
 */

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
    <section className={styles.panel} aria-label="Account settings">
      <h2 className={profileStyles.sectionHeader}>Account Settings</h2>

      <div className={styles.row}>
        <span className={styles.label}>Email</span>
        <span className={styles.value} data-testid="account-email">
          {loadError ? (
            <span className={styles.loadError}>{loadError}</span>
          ) : email === undefined ? (
            <span className={styles.pending}>Loading...</span>
          ) : email === null ? (
            <span className={styles.notSet}>Not set</span>
          ) : (
            email
          )}
        </span>
      </div>

      <form onSubmit={handleSubmit} className={styles.form} aria-label="Change password">
        <label className={styles.field}>
          Current password
          <input
            className={controls.input}
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

        <label className={styles.field}>
          New password
          <input
            className={`${controls.input} ${newPassword.length === 0 ? '' : newPasswordValid ? styles.valid : styles.invalid}`}
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
        </label>
        {/* Outside the <label> on purpose: nested inside, the rule text joins
            the input's accessible name and is announced as part of it. */}
        <PasswordChecklist id="password-rules" password={newPassword} />

        {formError && (
          <div role="alert" className={alertStyles.error}>
            {formError}
          </div>
        )}
        {formSuccess && (
          <div role="status" className={styles.success}>
            Password updated. Other sessions have been signed out.
          </div>
        )}

        <button
          type="submit"
          disabled={!canSubmit}
          className={`${controls.button} ${controls.primary} ${styles.submit}`}
        >
          {submitting ? '...' : 'Change password'}
        </button>
      </form>

      <h2 className={profileStyles.sectionHeader}>Session</h2>
      <button
        className={`${controls.button} ${controls.danger} ${styles.logout}`}
        onClick={() => void logout()}
      >
        Log out
      </button>

      <h2 className={profileStyles.sectionHeader}>Coming soon</h2>
      <div className={styles.comingSoon}>
        <div className={styles.tile} aria-disabled>
          <div className={styles.tileTitle}>Change email</div>
          <span className={styles.tag}>Coming soon</span>
        </div>
        <div className={styles.tile} aria-disabled>
          <div className={styles.tileTitle}>Delete account</div>
          <span className={styles.tag}>Coming soon</span>
        </div>
      </div>
    </section>
  );
}
