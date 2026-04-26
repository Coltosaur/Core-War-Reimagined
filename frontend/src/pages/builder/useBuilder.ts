import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type { Monaco, OnMount } from '@monaco-editor/react';
import init, { parseWarrior } from 'core-war-engine';
import {
  useWarriorLibrary,
  createUserWarrior,
  updateUserWarrior,
  deleteUserWarrior,
  duplicateWarrior,
  type Warrior,
} from '../../warriors/library';
import { registerRedcode, parseErrorToMarker } from '../../redcode/monaco';

export type ParseStatus = { ok: true; name: string | null } | { ok: false; message: string } | null;

export function useBuilder() {
  const navigate = useNavigate();
  const library = useWarriorLibrary();
  const [selectedId, setSelectedId] = useState<string>(() => library[0]?.id ?? '');
  const [source, setSource] = useState<string>('');
  const [label, setLabel] = useState<string>('');
  const [dirty, setDirty] = useState(false);
  const [wasmReady, setWasmReady] = useState(false);
  const [parseStatus, setParseStatus] = useState<ParseStatus>(null);

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

  const selected = library.find((w) => w.id === selectedId);

  const presets = useMemo(() => library.filter((w) => w.isPreset), [library]);
  const userWarriors = useMemo(() => library.filter((w) => !w.isPreset), [library]);

  const syncFromWarrior = useCallback((warrior: Warrior) => {
    setSource(warrior.source);
    setLabel(warrior.label);
    setDirty(false);
  }, []);

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

  useEffect(() => {
    if (!wasmReady) return;
    // Parsing updates both React state (parseStatus) and Monaco markers (external system).
    // eslint-disable-next-line react-hooks/set-state-in-effect
    runParse(source);
  }, [source, wasmReady]);

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
    const warrior = library.find((w) => w.id === id);
    if (warrior) syncFromWarrior(warrior);
  };

  const handleSave = () => {
    if (!selected) return;
    if (selected.isPreset) {
      const created = createUserWarrior(label || 'Untitled', source);
      setSelectedId(created.id);
      syncFromWarrior(created);
      return;
    }
    updateUserWarrior(selected.id, { label: label || 'Untitled', source });
    setDirty(false);
  };

  const handleDuplicate = () => {
    if (!selected) return;
    const created = duplicateWarrior(selected.id);
    if (created) {
      setSelectedId(created.id);
      syncFromWarrior(created);
    }
  };

  const handleDelete = () => {
    if (!selected || selected.isPreset) return;
    if (!confirm(`Delete "${selected.label}"? This cannot be undone.`)) return;
    deleteUserWarrior(selected.id);
    const remaining = library.filter((w) => w.id !== selected.id);
    const nextId = remaining[0]?.id ?? '';
    setSelectedId(nextId);
    const next = remaining[0];
    if (next) syncFromWarrior(next);
  };

  const handleNew = () => {
    const template = `;name New Warrior
;author you
        ORG    start
start   MOV.I  $0, $1
`;
    const created = createUserWarrior('New Warrior', template);
    setSelectedId(created.id);
    syncFromWarrior(created);
  };

  const handleTestInBattle = () => {
    if (!selected) return;
    let targetId = selected.id;
    if (dirty) {
      if (selected.isPreset) {
        const created = createUserWarrior(label || 'Untitled', source);
        targetId = created.id;
      } else {
        updateUserWarrior(selected.id, { label: label || 'Untitled', source });
      }
    }
    const opponent =
      presets.find((w) => w.id !== targetId) ?? library.find((w) => w.id !== targetId);
    const opponentId = opponent?.id ?? targetId;
    navigate(`/battle?red=${encodeURIComponent(targetId)}&blue=${encodeURIComponent(opponentId)}`);
  };

  const handleLabelChange = (value: string) => {
    setLabel(value);
    setDirty(true);
  };

  const handleSourceChange = (v: string | undefined) => {
    setSource(v ?? '');
    setDirty(true);
  };

  return {
    selectedId,
    selected,
    source,
    label,
    dirty,
    wasmReady,
    parseStatus,
    presets,
    userWarriors,
    canSave: !!selected && dirty,
    handleEditorWillMount,
    handleEditorDidMount,
    handleSelect,
    handleSave,
    handleDuplicate,
    handleDelete,
    handleNew,
    handleTestInBattle,
    handleLabelChange,
    handleSourceChange,
  };
}
