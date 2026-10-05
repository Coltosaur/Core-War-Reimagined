import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import CheatSheetPanel from './CheatSheetPanel';
import { OPCODES } from './cheatSheet';

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

function stubViewport(desktop: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn((query: string) => ({ matches: desktop, media: query }) as MediaQueryList),
  );
}

const toggle = () => screen.getByRole('button', { name: /cheat sheet/i });

describe('CheatSheetPanel', () => {
  it('starts closed on phones so it does not push the editor down', () => {
    stubViewport(false);
    render(<CheatSheetPanel />);
    expect(toggle()).toHaveAttribute('aria-expanded', 'false');
    expect(screen.getByText(OPCODES[0].desc, { exact: false })).not.toBeVisible();
  });

  it('starts open on desktop', () => {
    stubViewport(true);
    render(<CheatSheetPanel />);
    expect(toggle()).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText(OPCODES[0].desc, { exact: false })).toBeVisible();
  });

  it('toggles from the header button', async () => {
    stubViewport(false);
    const user = userEvent.setup();
    render(<CheatSheetPanel />);
    await user.click(toggle());
    expect(toggle()).toHaveAttribute('aria-expanded', 'true');
    await user.click(toggle());
    expect(toggle()).toHaveAttribute('aria-expanded', 'false');
  });

  it('lists every opcode as a term', () => {
    stubViewport(true);
    render(<CheatSheetPanel />);
    for (const op of OPCODES) {
      expect(screen.getByText(op.symbol, { selector: 'dt' })).toBeInTheDocument();
    }
  });
});
