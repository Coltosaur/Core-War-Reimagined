import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const narrow = vi.hoisted(() => ({ value: false }));
vi.mock('../../ui/useIsNarrow', () => ({ useIsNarrow: () => narrow.value }));
// The full workspace pulls in Monaco + wasm; stub it so this test only
// covers the narrow/wide switch: rendering the workspace throws.
vi.mock('./useBuilder', () => ({
  useBuilder: () => {
    throw new Error('BuilderWorkspace rendered');
  },
}));

import BuilderPage from './BuilderPage';

beforeEach(() => {
  narrow.value = false;
});

describe('BuilderPage on small screens (#81)', () => {
  it('shows the desktop-only notice instead of the editor below 768px', () => {
    narrow.value = true;
    render(
      <MemoryRouter>
        <BuilderPage />
      </MemoryRouter>,
    );
    expect(screen.getByRole('note')).toHaveTextContent(/needs a larger screen/i);
    expect(screen.getByRole('link', { name: 'Battlefield' })).toHaveAttribute('href', '/battle');
    expect(screen.getByRole('link', { name: 'Learn Redcode' })).toHaveAttribute('href', '/learn');
  });

  it('renders the workspace (not the notice) on wide screens', () => {
    expect(() =>
      render(
        <MemoryRouter>
          <BuilderPage />
        </MemoryRouter>,
      ),
    ).toThrow(/BuilderWorkspace rendered/);
  });
});
