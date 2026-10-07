import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import LearnPage from './LearnPage';

describe('LearnPage', () => {
  it('renders the title and every numbered section', () => {
    render(<LearnPage />, { wrapper: MemoryRouter });
    expect(screen.getByRole('heading', { level: 1, name: 'LEARN REDCODE' })).toBeInTheDocument();
    expect(screen.getAllByRole('heading', { level: 2 })).toHaveLength(8);
  });

  it('wraps each reference table so it can scroll instead of widening the page', () => {
    render(<LearnPage />, { wrapper: MemoryRouter });
    const tables = screen.getAllByRole('table');
    expect(tables).toHaveLength(2);
    for (const table of tables) {
      expect(table.parentElement?.tagName).toBe('DIV');
    }
  });

  it('links to the Warrior Builder', () => {
    render(<LearnPage />, { wrapper: MemoryRouter });
    for (const link of screen.getAllByRole('link', { name: /Warrior Builder/ })) {
      expect(link).toHaveAttribute('href', '/builder');
    }
  });
});
