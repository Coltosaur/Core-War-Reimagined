import { useRef } from 'react';
import type { Warrior } from '../../warriors/library';
import controls from '../../components/controls.module.css';
import styles from './EditorToolbar.module.css';

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

  const button = controls.button;

  return (
    <div className={styles.toolbar}>
      <input
        className={`${controls.input} ${styles.name}`}
        value={label}
        onChange={(e) => onLabelChange(e.target.value)}
        placeholder="Warrior name"
        aria-label="Warrior name"
        disabled={!selected || selected.isPreset}
      />
      {selected && !selected.isPreset && (
        <button
          type="button"
          className={`${button} ${controls.primary}`}
          onClick={onSave}
          disabled={!canSave}
          title="Save changes"
        >
          Save
        </button>
      )}
      <button type="button" className={button} onClick={onDuplicate} disabled={!selected}>
        Duplicate
      </button>
      {selected && !selected.isPreset && (
        <button type="button" className={`${button} ${controls.danger}`} onClick={onDelete}>
          Delete
        </button>
      )}
      <button
        type="button"
        className={button}
        onClick={handleExport}
        disabled={!selected}
        title="Export .red"
      >
        Export
      </button>
      <button type="button" className={button} onClick={handleImportClick} title="Import .red">
        Import
      </button>
      <input ref={fileInputRef} type="file" accept=".red,.rd" hidden onChange={handleFileChange} />
      <button type="button" className={button} onClick={onTestInBattle} disabled={!selected}>
        Test in Battlefield &rarr;
      </button>
    </div>
  );
}
