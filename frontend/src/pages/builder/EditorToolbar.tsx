import { useRef } from 'react';
import type { Warrior } from '../../warriors/library';
import {
  BUTTON_STYLE,
  DANGER_BUTTON_STYLE,
  INPUT_STYLE,
  PRIMARY_BUTTON_STYLE,
  TOOLBAR_STYLE,
} from './styles';

type Props = {
  label: string;
  selected: Warrior | undefined;
  canSave: boolean;
  onLabelChange: (value: string) => void;
  onSave: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onTestInBattle: () => void;
  onImport: (name: string, source: string) => void;
  source: string;
};

function downloadFile(filename: string, content: string): void {
  const blob = new Blob([content], { type: 'text/plain' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export default function EditorToolbar({
  label,
  selected,
  canSave,
  onLabelChange,
  onSave,
  onDuplicate,
  onDelete,
  onTestInBattle,
  onImport,
  source,
}: Props) {
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleImportClick = () => fileInputRef.current?.click();

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const name = file.name.replace(/\.re?d$/i, '');
    file.text().then((text) => onImport(name, text));
    e.target.value = '';
  };

  const handleExport = () => {
    if (!selected) return;
    const filename = `${label || 'warrior'}.red`;
    downloadFile(filename, source);
  };

  return (
    <div style={TOOLBAR_STYLE}>
      <input
        style={INPUT_STYLE}
        value={label}
        onChange={(e) => onLabelChange(e.target.value)}
        placeholder="Warrior name"
        disabled={!selected || selected.isPreset}
      />
      {selected && !selected.isPreset && (
        <button
          style={PRIMARY_BUTTON_STYLE}
          onClick={onSave}
          disabled={!canSave}
          title="Save changes"
        >
          Save
        </button>
      )}
      <button style={BUTTON_STYLE} onClick={onDuplicate} disabled={!selected}>
        Duplicate
      </button>
      {selected && !selected.isPreset && (
        <button style={DANGER_BUTTON_STYLE} onClick={onDelete}>
          Delete
        </button>
      )}
      <button style={BUTTON_STYLE} onClick={handleExport} disabled={!selected} title="Export .red">
        Export
      </button>
      <button style={BUTTON_STYLE} onClick={handleImportClick} title="Import .red">
        Import
      </button>
      <input
        ref={fileInputRef}
        type="file"
        accept=".red,.rd"
        style={{ display: 'none' }}
        onChange={handleFileChange}
      />
      <button style={BUTTON_STYLE} onClick={onTestInBattle} disabled={!selected}>
        Test in Battlefield &rarr;
      </button>
    </div>
  );
}
