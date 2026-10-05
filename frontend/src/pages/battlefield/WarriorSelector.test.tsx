import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import WarriorSelector from './WarriorSelector';
import type { Warrior } from '../../warriors/library';

afterEach(cleanup);

const imp: Warrior = { id: 'preset:imp', label: 'Imp', source: '', isPreset: true };
const mine: Warrior = { id: 'server:abc', label: 'My Bomber', source: '', isPreset: false };

describe('WarriorSelector', () => {
  it('links each side to the Builder for its selected warrior', () => {
    render(
      <MemoryRouter>
        <WarriorSelector
          redId="preset:imp"
          blueId="server:abc"
          presets={[imp]}
          userWarriors={[mine]}
          onPickChange={vi.fn()}
        />
      </MemoryRouter>,
    );
    expect(screen.getByRole('link', { name: 'Edit Imp in the Builder' })).toHaveAttribute(
      'href',
      '/builder?warrior=preset%3Aimp',
    );
    expect(screen.getByRole('link', { name: 'Edit My Bomber in the Builder' })).toHaveAttribute(
      'href',
      '/builder?warrior=server%3Aabc',
    );
  });
});
