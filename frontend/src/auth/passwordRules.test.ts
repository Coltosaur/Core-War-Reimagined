import { describe, expect, it } from 'vitest';
import {
  PASSWORD_MAX_LEN,
  checkPasswordRules,
  passwordByteLength,
  passwordMeetsRules,
} from './passwordRules';

const rule = (pw: string, id: string) => checkPasswordRules(pw).find((r) => r.id === id)!;

describe('checkPasswordRules', () => {
  it('marks length as ok only at >= 12 chars', () => {
    expect(rule('short', 'length').ok).toBe(false);
    expect(rule('a'.repeat(12), 'length').ok).toBe(true);
  });

  it('marks non_digit as failing on pure-numeric passwords', () => {
    expect(rule('1'.repeat(20), 'non_digit').ok).toBe(false);
    expect(rule('abcdefghijkl', 'non_digit').ok).toBe(true);
  });

  it('hides too_long rule until it is actually violated', () => {
    expect(rule('a'.repeat(20), 'too_long').visible).toBe(false);
    const long = rule('a'.repeat(PASSWORD_MAX_LEN + 1), 'too_long');
    expect(long.visible).toBe(true);
    expect(long.ok).toBe(false);
  });

  it('passwordMeetsRules mirrors backend validate_password', () => {
    expect(passwordMeetsRules('short')).toBe(false);
    expect(passwordMeetsRules('123456789012')).toBe(false); // all digits
    expect(passwordMeetsRules('abcdefghijkl')).toBe(true);
    expect(passwordMeetsRules('brand-new-password')).toBe(true);
  });
});

describe('length is measured in UTF-8 bytes, matching Rust str::len()', () => {
  it('counts multi-byte characters by their byte width', () => {
    expect(passwordByteLength('abc')).toBe(3);
    expect(passwordByteLength('é')).toBe(2); // 1 UTF-16 unit, 2 bytes
    expect(passwordByteLength('🦖')).toBe(4); // 2 UTF-16 units, 4 bytes
  });

  it('accepts a password the server accepts but JS .length would reject', () => {
    // 6 emoji = 12 UTF-8 bytes, so the backend's `len() < 12` check passes,
    // but String.length reports 12 UTF-16 units only by coincidence here —
    // use 4 emoji (16 bytes, 8 units) to make the two clearly disagree.
    const pw = '🦖'.repeat(4);
    expect(pw.length).toBeLessThan(12); // JS units: 8
    expect(passwordByteLength(pw)).toBe(16); // Rust bytes: 16
    expect(rule(pw, 'length').ok).toBe(true);
  });

  it('rejects an over-long password by bytes, not units', () => {
    const pw = 'é'.repeat(PASSWORD_MAX_LEN / 2 + 1); // 1002 bytes, 501 units
    expect(pw.length).toBeLessThanOrEqual(PASSWORD_MAX_LEN);
    expect(passwordMeetsRules(pw)).toBe(false);
  });
});
