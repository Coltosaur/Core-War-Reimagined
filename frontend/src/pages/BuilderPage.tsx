import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import Editor, { type Monaco, type OnMount } from '@monaco-editor/react';
import init, { parseWarrior } from 'core-war-engine';
import {
  useWarriorLibrary,
  createUserWarrior,
  updateUserWarrior,
  deleteUserWarrior,
  duplicateWarrior,
  type Warrior,
} from '../warriors/library';
import {
  REDCODE_LANGUAGE_ID,
  registerRedcode,
  parseErrorToMarker,
} from '../redcode/monaco';
import CheatSheetPanel from '../redcode/CheatSheetPanel';

const PAGE_STYLE: React.CSSProperties = {
  display: 'flex',
  height: '100vh',
  minHeight: 0,
};

const LIST_STYLE: React.CSSProperties = {
  width: '240px',
  flexShrink: 0,
  borderRight: '1px solid #222',
  display: 'flex',
  flexDirection: 'column',
  backgroundColor: '#0d0d0d',
};

const LIST_HEADER_STYLE: React.CSSProperties = {
  padding: '0.75rem 1rem',
  fontSize: '0.7rem',
  letterSpacing: '0.1em',
  color: '#666',
  textTransform: 'uppercase',
  borderBottom: '1px solid #222',
};

const MAIN_STYLE: React.CSSProperties = {
  flex: 1,
  minWidth: 0,
  display: 'flex',
  flexDirection: 'column',
};

const TOOLBAR_STYLE: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: '0.5rem',
  padding: '0.75rem 1rem',
  borderBottom: '1px solid #222',
  backgroundColor: '#111',
};

const BUTTON_STYLE: React.CSSProperties = {
  padding: '0.4rem 0.9rem',
  fontFamily: 'inherit',
  fontSize: '0.8rem',
  cursor: 'pointer',
  backgroundColor: '#1e1e1e',
  color: '#e0e0e0',
  border: '1px solid #444',
  borderRadius: '4px',
};

const PRIMARY_BUTTON_STYLE: React.CSSProperties = {
  ...BUTTON_STYLE,
  backgroundColor: '#e9456022',
  borderColor: '#e9456066',
  color: '#e94560',
};

const DANGER_BUTTON_STYLE: React.CSSProperties = {
  ...BUTTON_STYLE,
  color: '#e94560',
  borderColor: '#e9456066',
};

const INPUT_STYLE: React.CSSProperties = {
  fontFamily: 'inherit',
  fontSize: '0.85rem',
  backgroundColor: '#1e1e1e',
  color: '#e0e0e0',
  border: '1px solid #444',
  borderRadius: '4px',
  padding: '0.35rem 0.6rem',
  flex: 1,
  minWidth: 0,
};

const STATUS_STYLE: React.CSSProperties = {
  padding: '0.5rem 1rem',
  fontSize: '0.8rem',
  borderTop: '1px solid #222',
  backgroundColor: '#0d0d0d',
  minHeight: '2rem',
  display: 'flex',
  alignItems: 'center',
};

const listItemStyle = (active: boolean, preset: boolean): React.CSSProperties => ({
  padding: '0.5rem 1rem',
  cursor: 'pointer',
  fontSize: '0.85rem',
  color: preset ? '#bbb' : '#e0e0e0',
  backgroundColor: active ? '#1a1a1a' : 'transparent',
  borderLeft: active ? '3px solid #e94560' : '3px solid transparent',
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
  gap: '0.5rem',
});

export default function BuilderPage() {
  const navigate = useNavigate();
  const library = useWarriorLibrary();
  const [selectedId, setSelectedId] = useState<string>(() => library[0]?.id ?? '');
  const [source, setSource] = useState<string>('');
  const [label, setLabel] = useState<string>('');
  const [dirty, setDirty] = useState(false);
  const [wasmReady, setWasmReady] = useState(false);
  const [parseStatus, setParseStatus] = useState<
    { ok: true; name: string | null } | { ok: false; message: string } | null
  >(null);

  const monacoRef = useRef<Monaco | null>(null);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);

  useEffect(() => {
    let cancelled = false;
    init().then(() => {
      if (!cancelled) setWasmReady(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // The Editor below uses `key={selectedId}` so it remounts cleanly per
  // warrior. Pair that with `defaultValue` (uncontrolled) and `onChange`
  // only fires on real keystrokes — never spuriously on warrior switch.
  const selected = library.find((w) => w.id === selectedId);
  useEffect(() => {
    if (!selected) return;
    setSource(selected.source);
    setLabel(selected.label);
    setDirty(false);
  }, [selectedId, selected?.source, selected?.label]);

  // Re-run parser whenever source or wasm readiness changes.
  useEffect(() => {
    if (!wasmReady) return;
    runParse(source);
  }, [source, wasmReady]);

  function runParse(text: string): void {
    const monaco = monacoRef.current;
    const editor = editorRef.current;
    const model = editor?.getModel();
    try {
      const w = parseWarrior(text);
      setParseStatus({ ok: true, name: w.name() ?? null });
      if (monaco && model) {
        monaco.editor.setModelMarkers(model, 'redcode-parser', []);
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setParseStatus({ ok: false, message: msg });
      if (monaco && model) {
        monaco.editor.setModelMarkers(model, 'redcode-parser', [
          parseErrorToMarker(monaco, msg, model.getLineCount()),
        ]);
      }
    }
  }

  const handleEditorWillMount = (monaco: Monaco) => {
    registerRedcode(monaco);
  };

  const handleEditorDidMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    monacoRef.current = monaco;
    if (wasmReady) runParse(editor.getValue());
  };

  const handleSelect = (id: string) => {
    if (dirty && !confirm('Discard unsaved changes?')) return;
    setSelectedId(id);
  };

  const handleSave = () => {
    if (!selected) return;
    if (selected.isPreset) {
      const created = createUserWarrior(label || 'Untitled', source);
      setSelectedId(created.id);
      setDirty(false);
      return;
    }
    updateUserWarrior(selected.id, { label: label || 'Untitled', source });
    setDirty(false);
  };

  const handleDuplicate = () => {
    if (!selected) return;
    const created = duplicateWarrior(selected.id);
    if (created) setSelectedId(created.id);
  };

  const handleDelete = () => {
    if (!selected || selected.isPreset) return;
    if (!confirm(`Delete "${selected.label}"? This cannot be undone.`)) return;
    deleteUserWarrior(selected.id);
    const remaining = library.filter((w) => w.id !== selected.id);
    setSelectedId(remaining[0]?.id ?? '');
  };

  const handleNew = () => {
    const template = `;name New Warrior
;author you
        ORG    start
start   MOV.I  $0, $1
`;
    const created = createUserWarrior('New Warrior', template);
    setSelectedId(created.id);
  };

  const handleTestInBattle = () => {
    if (!selected) return;
    // If there are unsaved edits on a user warrior, save first so the
    // battlefield picks up the current contents.
    let targetId = selected.id;
    if (dirty) {
      if (selected.isPreset) {
        const created = createUserWarrior(label || 'Untitled', source);
        targetId = created.id;
      } else {
        updateUserWarrior(selected.id, { label: label || 'Untitled', source });
      }
    }
    // Pick an opponent: first preset that isn't us, else first warrior that isn't us.
    const presets = library.filter((w) => w.isPreset);
    const opponent =
      presets.find((w) => w.id !== targetId) ??
      library.find((w) => w.id !== targetId);
    const opponentId = opponent?.id ?? targetId;
    navigate(`/battle?red=${encodeURIComponent(targetId)}&blue=${encodeURIComponent(opponentId)}`);
  };

  const presets = library.filter((w) => w.isPreset);
  const userWarriors = library.filter((w) => !w.isPreset);

  const canSave = !!selected && dirty;
  const canDelete = !!selected && !selected.isPreset;

  return (
    <div style={PAGE_STYLE}>
      <aside style={LIST_STYLE}>
        <div style={LIST_HEADER_STYLE}>Classic (read-only)</div>
        {presets.map((w) => (
          <WarriorListItem
            key={w.id}
            warrior={w}
            active={w.id === selectedId}
            onSelect={handleSelect}
          />
        ))}
        <div style={LIST_HEADER_STYLE}>My Warriors</div>
        {userWarriors.length === 0 && (
          <div style={{ padding: '0.5rem 1rem', fontSize: '0.8rem', color: '#555', fontStyle: 'italic' }}>
            None yet. Duplicate a classic or create a new one.
          </div>
        )}
        {userWarriors.map((w) => (
          <WarriorListItem
            key={w.id}
            warrior={w}
            active={w.id === selectedId}
            onSelect={handleSelect}
          />
        ))}
        <div style={{ padding: '0.75rem 1rem', marginTop: 'auto', borderTop: '1px solid #222' }}>
          <button style={{ ...BUTTON_STYLE, width: '100%' }} onClick={handleNew}>
            + New Warrior
          </button>
        </div>
      </aside>

      <section style={MAIN_STYLE}>
        <div style={TOOLBAR_STYLE}>
          <input
            style={INPUT_STYLE}
            value={label}
            onChange={(e) => {
              setLabel(e.target.value);
              setDirty(true);
            }}
            placeholder="Warrior name"
            disabled={!selected || selected.isPreset}
          />
          {selected && !selected.isPreset && (
            <button
              style={PRIMARY_BUTTON_STYLE}
              onClick={handleSave}
              disabled={!canSave}
              title="Save changes"
            >
              Save
            </button>
          )}
          <button style={BUTTON_STYLE} onClick={handleDuplicate} disabled={!selected}>
            Duplicate
          </button>
          {selected && !selected.isPreset && (
            <button style={DANGER_BUTTON_STYLE} onClick={handleDelete}>
              Delete
            </button>
          )}
          <button style={BUTTON_STYLE} onClick={handleTestInBattle} disabled={!selected}>
            Test in Battlefield →
          </button>
        </div>

        <div style={{ flex: 1, minHeight: 0 }}>
          <Editor
            key={selectedId}
            height="100%"
            language={REDCODE_LANGUAGE_ID}
            theme="redcode-dark"
            defaultValue={selected?.source ?? ''}
            onChange={(v) => {
              setSource(v ?? '');
              setDirty(true);
            }}
            beforeMount={handleEditorWillMount}
            onMount={handleEditorDidMount}
            options={{
              minimap: { enabled: false },
              fontFamily: '"JetBrains Mono", "Fira Code", monospace',
              fontSize: 14,
              lineNumbers: 'on',
              scrollBeyondLastLine: false,
              renderWhitespace: 'selection',
              readOnly: selected?.isPreset ?? false,
              wordWrap: 'off',
              tabSize: 8,
            }}
          />
        </div>

        <div style={STATUS_STYLE}>
          {!wasmReady && <span style={{ color: '#666' }}>Loading engine...</span>}
          {wasmReady && parseStatus?.ok && (
            <span style={{ color: '#a5d6a7' }}>
              ✓ parsed
              {parseStatus.name ? ` — ${parseStatus.name}` : ''}
            </span>
          )}
          {wasmReady && parseStatus && !parseStatus.ok && (
            <span style={{ color: '#e94560' }}>✗ {parseStatus.message}</span>
          )}
          {selected?.isPreset && (
            <span style={{ marginLeft: 'auto', color: '#666', fontStyle: 'italic' }}>
              Classic warrior — read-only. Use Duplicate to edit.
            </span>
          )}
          {dirty && !selected?.isPreset && (
            <span style={{ marginLeft: 'auto', color: '#f0c040' }}>● unsaved changes</span>
          )}
        </div>
      </section>

      <CheatSheetPanel />
    </div>
  );
}

function WarriorListItem({
  warrior,
  active,
  onSelect,
}: {
  warrior: Warrior;
  active: boolean;
  onSelect: (id: string) => void;
}) {
  return (
    <div
      style={listItemStyle(active, warrior.isPreset)}
      onClick={() => onSelect(warrior.id)}
    >
      <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
        {warrior.label}
      </span>
      {warrior.isPreset && (
        <span style={{ fontSize: '0.65rem', color: '#666' }}>CLASSIC</span>
      )}
    </div>
  );
}
