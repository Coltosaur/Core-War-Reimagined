import { useState } from 'react';
import { useAuth } from './AuthContext';
import { ApiError } from './client';

type Props = { onClose: () => void };

export default function AuthModal({ onClose }: Props) {
  const { login, register } = useAuth();
  const [mode, setMode] = useState<'login' | 'register'>('login');
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setSubmitting(true);
    try {
      if (mode === 'login') {
        await login({ username_or_email: username, password });
      } else {
        await register({ username, email, password });
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
            {mode === 'login' ? 'Log In' : 'Create Account'}
          </h2>
          <button onClick={onClose} style={CLOSE_BTN}>
            &times;
          </button>
        </div>

        <form onSubmit={handleSubmit} style={FORM}>
          <label style={LABEL}>
            {mode === 'login' ? 'Username or Email' : 'Username'}
            <input
              style={INPUT}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoFocus
              required
            />
          </label>

          {mode === 'register' && (
            <label style={LABEL}>
              Email
              <input
                style={INPUT}
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
              />
            </label>
          )}

          <label style={LABEL}>
            Password
            <input
              style={INPUT}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              minLength={8}
            />
          </label>

          {error && <div style={ERROR}>{error}</div>}

          <button type="submit" disabled={submitting} style={SUBMIT_BTN}>
            {submitting ? '...' : mode === 'login' ? 'Log In' : 'Register'}
          </button>
        </form>

        <div style={TOGGLE}>
          {mode === 'login' ? (
            <>
              No account?{' '}
              <button
                style={LINK_BTN}
                onClick={() => {
                  setMode('register');
                  setError('');
                }}
              >
                Register
              </button>
            </>
          ) : (
            <>
              Already have an account?{' '}
              <button
                style={LINK_BTN}
                onClick={() => {
                  setMode('login');
                  setError('');
                }}
              >
                Log in
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

const INPUT: React.CSSProperties = {
  backgroundColor: '#111',
  border: '1px solid #333',
  borderRadius: '4px',
  padding: '0.5rem',
  color: '#e0e0e0',
  fontSize: '0.85rem',
  fontFamily: 'inherit',
  outline: 'none',
};

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

const TOGGLE: React.CSSProperties = {
  marginTop: '1rem',
  textAlign: 'center',
  fontSize: '0.75rem',
  color: '#888',
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
