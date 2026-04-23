import { beforeEach, describe, expect, it } from 'vitest';
import {
  listWarriors,
  getWarrior,
  createUserWarrior,
  updateUserWarrior,
  deleteUserWarrior,
  duplicateWarrior,
} from './library';

const STORAGE_KEY = 'corewar.warriors.v1';

beforeEach(() => {
  localStorage.clear();
});

describe('warrior library', () => {
  describe('listWarriors', () => {
    it('includes preset warriors', () => {
      const warriors = listWarriors();
      const presets = warriors.filter((w) => w.isPreset);
      expect(presets.length).toBeGreaterThanOrEqual(5);
      expect(presets.map((w) => w.label)).toContain('Imp');
      expect(presets.map((w) => w.label)).toContain('Dwarf');
      expect(presets.map((w) => w.label)).toContain('Mice');
      expect(presets.map((w) => w.label)).toContain('Scanner');
    });

    it('preset warrior ids start with "preset:"', () => {
      const presets = listWarriors().filter((w) => w.isPreset);
      for (const p of presets) {
        expect(p.id).toMatch(/^preset:/);
      }
    });
  });

  describe('createUserWarrior', () => {
    it('creates a warrior with correct fields', () => {
      const warrior = createUserWarrior('My Warrior', 'MOV.I $0, $1');
      expect(warrior.label).toBe('My Warrior');
      expect(warrior.source).toBe('MOV.I $0, $1');
      expect(warrior.isPreset).toBe(false);
      expect(warrior.id).toMatch(/^user:/);
    });

    it('adds warrior to the library', () => {
      const warrior = createUserWarrior('Test', 'DAT #0, #0');
      const all = listWarriors();
      expect(all.some((w) => w.id === warrior.id)).toBe(true);
    });

    it('persists to localStorage', () => {
      const warrior = createUserWarrior('Persisted', 'NOP');
      const stored = JSON.parse(localStorage.getItem(STORAGE_KEY)!);
      expect(stored.some((w: { id: string }) => w.id === warrior.id)).toBe(true);
    });

    it('generates unique ids', () => {
      const w1 = createUserWarrior('A', 'DAT');
      const w2 = createUserWarrior('B', 'DAT');
      expect(w1.id).not.toBe(w2.id);
    });
  });

  describe('updateUserWarrior', () => {
    it('updates label and source', () => {
      const warrior = createUserWarrior('Original', 'DAT #0, #0');
      updateUserWarrior(warrior.id, { label: 'Updated', source: 'MOV $0, $1' });
      const updated = getWarrior(warrior.id);
      expect(updated?.label).toBe('Updated');
      expect(updated?.source).toBe('MOV $0, $1');
    });

    it('supports partial updates (label only)', () => {
      const warrior = createUserWarrior('Name', 'MOV $0, $1');
      updateUserWarrior(warrior.id, { label: 'New Name' });
      const updated = getWarrior(warrior.id);
      expect(updated?.label).toBe('New Name');
      expect(updated?.source).toBe('MOV $0, $1');
    });

    it('does not modify preset warriors', () => {
      const preset = listWarriors().find((w) => w.isPreset)!;
      const originalLabel = preset.label;
      updateUserWarrior(preset.id, { label: 'Hacked' });
      expect(getWarrior(preset.id)?.label).toBe(originalLabel);
    });
  });

  describe('deleteUserWarrior', () => {
    it('removes a user warrior', () => {
      const warrior = createUserWarrior('Doomed', 'DAT #0, #0');
      expect(getWarrior(warrior.id)).toBeDefined();
      deleteUserWarrior(warrior.id);
      expect(getWarrior(warrior.id)).toBeUndefined();
    });

    it('updates localStorage after deletion', () => {
      const warrior = createUserWarrior('Temp', 'DAT');
      deleteUserWarrior(warrior.id);
      const stored = JSON.parse(localStorage.getItem(STORAGE_KEY)!);
      expect(stored.every((w: { id: string }) => w.id !== warrior.id)).toBe(true);
    });
  });

  describe('duplicateWarrior', () => {
    it('duplicates a preset warrior as a user warrior', () => {
      const preset = listWarriors().find((w) => w.isPreset)!;
      const dup = duplicateWarrior(preset.id);
      expect(dup).toBeDefined();
      expect(dup!.label).toBe(`${preset.label} (copy)`);
      expect(dup!.source).toBe(preset.source);
      expect(dup!.isPreset).toBe(false);
      expect(dup!.id).not.toBe(preset.id);
    });

    it('duplicates a user warrior', () => {
      const orig = createUserWarrior('Original', 'MOV $0, $1');
      const dup = duplicateWarrior(orig.id);
      expect(dup).toBeDefined();
      expect(dup!.label).toBe('Original (copy)');
      expect(dup!.source).toBe(orig.source);
    });

    it('returns undefined for non-existent warrior', () => {
      expect(duplicateWarrior('nonexistent')).toBeUndefined();
    });
  });

  describe('getWarrior', () => {
    it('finds warriors by id', () => {
      const first = listWarriors()[0];
      expect(getWarrior(first.id)).toEqual(first);
    });

    it('returns undefined for unknown id', () => {
      expect(getWarrior('unknown')).toBeUndefined();
    });
  });
});
