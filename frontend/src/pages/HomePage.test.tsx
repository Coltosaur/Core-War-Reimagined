import { describe, expect, it } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import HomePage from './HomePage';

describe('HomePage', () => {
  it('links to the three starting points', () => {
    render(<HomePage />, { wrapper: MemoryRouter });
    expect(screen.getByRole('heading', { level: 1, name: 'CORE WAR' })).toBeInTheDocument();
    const actions = within(screen.getByRole('navigation', { name: 'Get started' }));
    expect(actions.getByRole('link', { name: 'Enter Battlefield' })).toHaveAttribute(
      'href',
      '/battle',
    );
    expect(actions.getByRole('link', { name: 'Warrior Builder' })).toHaveAttribute(
      'href',
      '/builder',
    );
    expect(actions.getByRole('link', { name: 'Learn Redcode' })).toHaveAttribute('href', '/learn');
  });
});
