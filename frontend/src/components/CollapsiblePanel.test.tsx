import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import CollapsiblePanel from './CollapsiblePanel';

afterEach(cleanup);

describe('CollapsiblePanel', () => {
  it('starts collapsed by default, with the toggle wired to the body', () => {
    render(<CollapsiblePanel title="Processes">body text</CollapsiblePanel>);
    const toggle = screen.getByRole('button', { name: /Processes/ });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    const body = document.getElementById(toggle.getAttribute('aria-controls')!);
    expect(body).toHaveTextContent('body text');
    expect(body).not.toBeVisible();
  });

  it('starts open when defaultOpen is set', () => {
    render(
      <CollapsiblePanel title="Cell" defaultOpen>
        body text
      </CollapsiblePanel>,
    );
    expect(screen.getByRole('button', { name: /Cell/ })).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('body text')).toBeVisible();
  });

  it('toggles open and closed', async () => {
    const user = userEvent.setup();
    render(<CollapsiblePanel title="Processes">body text</CollapsiblePanel>);
    const toggle = screen.getByRole('button', { name: /Processes/ });
    await user.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('body text')).toBeVisible();
    await user.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
  });

  it('also renders the title as a heading for the desktop layout', () => {
    render(<CollapsiblePanel title="Processes">x</CollapsiblePanel>);
    expect(screen.getByRole('heading', { name: 'Processes' })).toBeInTheDocument();
  });
});
