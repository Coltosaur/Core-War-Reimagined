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
  syncFromServer,
  createServerWarrior,
  updateServerWarrior,
  deleteServerWarrior,
  type Warrior,
} from '../../warriors/library';
import { registerRedcode, parseErrorToMarker } from '../../redcode/monaco';
import { useAuth } from '../../api/AuthContext';

export type ParseStatus = { ok: true; name: string | null } | { ok: false; message: string } | null;

function isServerWarrior(id: string): boolean {
  return id.startsWith('server:');
}

export function useBuilder() {
  const navigate = useNavigate();
  const { user } = useAuth();
  const library = useWarriorLibrary();
  const [selectedId, setSelectedId] = useState<string>(() => library[0]?.id ?? '');
  const [source, setSource] = useState<string>(() => library[0]?.source ?? '');
  const [label, setLabel] = useState<string>(() => library[0]?.label ?? '');
  const [dirty, setDirty] = useState(false);
  const [wasmReady, setWasmReady] = useState(false);
  const [parseStatus, setParseStatus] = useState<ParseStatus>(null);
  const [saving, setSaving] = useState(false);

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

  useEffect(() => {
    if (user) {
      syncFromServer();
    }
  }, [user]);

  const selected = library.find((w) => w.id === selectedId);

  const presets = useMemo(() => library.filter((w) => w.isPreset), [library]);
  const userWarriors = useMemo(() => library.filter((w) => !w.isPreset), [library]);

  const syncFromWarrior = useCallback((warrior: Warrior) => {
    setSource(warrior.source);
    setLabel(warrior.label);
    setDirty(false);
  }, []);

  // Mirror the selected warrior into local source/label state whenever the
  // selection or the library snapshot changes and the user has no pending
  // edits. Combined with the lazy-init of `source` and `label` above, this
  // guarantees the React state matches what Monaco shows (via
  // `defaultValue={selected?.source}`) from the very first render — so the
  // parse-status effect below never sees an empty string and mis-reports
  // EmptyWarrior on a fresh load. Gated on `!dirty` so in-flight edits are
  // never clobbered by an async library update (e.g. syncFromServer).
  useEffect(() => {
    if (dirty) return;
    const warrior = library.find((w) => w.id === selectedId);
    if (!warrior) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    syncFromWarrior(warrior);
  }, [selectedId, dirty, library, syncFromWarrior]);

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

  const handleSave = async () => {
    if (!selected || saving) return;
    setSaving(true);
    try {
      if (selected.isPreset) {
        if (user) {
          const created = await createServerWarrior(label || 'Untitled', source);
          setSelectedId(created.id);
          syncFromWarrior(created);
        } else {
          const created = createUserWarrior(label || 'Untitled', source);
          setSelectedId(created.id);
          syncFromWarrior(created);
        }
        return;
      }
      if (user && isServerWarrior(selected.id)) {
        await updateServerWarrior(selected.id, { label: label || 'Untitled', source });
      } else if (user && !isServerWarrior(selected.id)) {
        const created = await createServerWarrior(label || 'Untitled', source);
        setSelectedId(created.id);
        syncFromWarrior(created);
      } else {
        updateUserWarrior(selected.id, { label: label || 'Untitled', source });
      }
      setDirty(false);
    } finally {
      setSaving(false);
    }
  };

  const handleDuplicate = async () => {
    if (!selected || saving) return;
    if (user) {
      setSaving(true);
      try {
        const newLabel = `${selected.label} (copy)`;
        const created = await createServerWarrior(newLabel, selected.source);
        setSelectedId(created.id);
        syncFromWarrior(created);
      } finally {
        setSaving(false);
      }
    } else {
      const created = duplicateWarrior(selected.id);
      if (created) {
        setSelectedId(created.id);
        syncFromWarrior(created);
      }
    }
  };

  const handleDelete = async () => {
    if (!selected || selected.isPreset || saving) return;
    if (!confirm(`Delete "${selected.label}"? This cannot be undone.`)) return;
    if (user && isServerWarrior(selected.id)) {
      setSaving(true);
      try {
        await deleteServerWarrior(selected.id);
      } finally {
        setSaving(false);
      }
    } else {
      deleteUserWarrior(selected.id);
    }
    const remaining = library.filter((w) => w.id !== selected.id);
    const nextId = remaining[0]?.id ?? '';
    setSelectedId(nextId);
    const next = remaining[0];
    if (next) syncFromWarrior(next);
  };

  const handleImport = async (name: string, content: string) => {
    if (user) {
      setSaving(true);
      try {
        const created = await createServerWarrior(name || 'Imported', content);
        setSelectedId(created.id);
        syncFromWarrior(created);
      } finally {
        setSaving(false);
      }
    } else {
      const created = createUserWarrior(name || 'Imported', content);
      setSelectedId(created.id);
      syncFromWarrior(created);
    }
  };

  const handleNew = async () => {
    const template = `;name New Warrior
;author you
        ORG    start
start   MOV.I  $0, $1
`;
    if (user) {
      setSaving(true);
      try {
        const created = await createServerWarrior('New Warrior', template);
        setSelectedId(created.id);
        syncFromWarrior(created);
      } finally {
        setSaving(false);
      }
    } else {
      const created = createUserWarrior('New Warrior', template);
      setSelectedId(created.id);
      syncFromWarrior(created);
    }
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
    saving,
    canSave: !!selected && dirty && !saving,
    handleEditorWillMount,
    handleEditorDidMount,
    handleSelect,
    handleSave,
    handleDuplicate,
    handleDelete,
    handleNew,
    handleImport,
    handleTestInBattle,
    handleLabelChange,
    handleSourceChange,
  };
}
