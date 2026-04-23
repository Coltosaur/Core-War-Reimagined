import { describe, expect, it } from 'vitest';
import { parseErrorToMarker, REDCODE_LANGUAGE_ID } from './monaco';

const mockMonaco = {
  MarkerSeverity: { Error: 8 },
} as Parameters<typeof parseErrorToMarker>[0];

describe('parseErrorToMarker', () => {
  it('extracts line number from error message', () => {
    const marker = parseErrorToMarker(mockMonaco, 'Syntax error on line 5: unexpected token', 10);
    expect(marker.startLineNumber).toBe(5);
    expect(marker.endLineNumber).toBe(5);
    expect(marker.severity).toBe(8);
  });

  it('defaults to line 1 when no line number in message', () => {
    const marker = parseErrorToMarker(mockMonaco, 'Unknown opcode', 10);
    expect(marker.startLineNumber).toBe(1);
  });

  it('clamps line number to sourceLineCount', () => {
    const marker = parseErrorToMarker(mockMonaco, 'Error on line 999', 5);
    expect(marker.startLineNumber).toBe(5);
  });

  it('clamps line number to minimum of 1', () => {
    const marker = parseErrorToMarker(mockMonaco, 'Error on line 0', 10);
    expect(marker.startLineNumber).toBe(1);
  });

  it('preserves the full error message', () => {
    const msg = 'Invalid modifier on line 3';
    const marker = parseErrorToMarker(mockMonaco, msg, 10);
    expect(marker.message).toBe(msg);
  });

  it('is case-insensitive for "line" keyword', () => {
    const marker = parseErrorToMarker(mockMonaco, 'Error on LINE 7', 10);
    expect(marker.startLineNumber).toBe(7);
  });
});

describe('REDCODE_LANGUAGE_ID', () => {
  it('is "redcode"', () => {
    expect(REDCODE_LANGUAGE_ID).toBe('redcode');
  });
});
