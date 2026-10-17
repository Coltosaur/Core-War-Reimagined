import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import type { Monaco, OnMount } from '@monaco-editor/react';
import init, { parseWarrior } from 'core-war-engine';
import {
  useWarriorLibrary,
  createUserWarrior,
  updateUserWarrior,
  deleteUserWarrior,
  duplicateWarrior,
  createServerWarrior,
  updateServerWarrior,
  deleteServerWarrior,
  type Warrior,
} from '../../warriors/library';
import { registerRedcode, parseErrorToMarker } from '../../redcode/monaco';
import { useAuth } from '../../api/useAuth';
import { ApiError } from '../../api/client';
import { isServerId } from '../../warriors/serverId';

export type ParseStatus = { ok: true; name: string | null } | { ok: false; message: string } | null;

/** A blank name saves as "Untitled". */
function orUntitled(label: string): string {
  return label || 'Untitled';
}

// ApiError carries the backend's own message (or the session-expired copy).
// Anything else out of fetch is a network failure, whose raw text
// ("Failed to fetch", "NetworkError when attempting...") varies by browser.
function describeFailure(err: unknown): string {
  return err instanceof ApiError ? err.message : "the server couldn't be reached";
}

export function useBuilder() {
  const navigate = useNavigate();
  const { user } = useAuth();
  const library = useWarriorLibrary();
  const [searchParams, setSearchParams] = useSearchParams();

  // Selection = the user's own pick, else the warrior named in ?warrior=
  // (e.g. from the Battlefield's Edit link), else the first warrior.
  // Derived rather than set from an effect: a saved warrior named in the URL
  // only appears in the library once the server sync lands, and deriving
  // picks it up then without letting it override anything the user has
  // chosen in the meantime.
  const [requestedId] = useState(() => searchParams.get('warrior'));
  const [chosenId, setSelectedId] = useState<string | null>(null);
  const selectedId =
    chosenId ??
    (requestedId && library.some((w) => w.id === requestedId) ? requestedId : null) ??
    library[0]?.id ??
    '';
  const [source, setSource] = useState<string>(
    () => library.find((w) => w.id === selectedId)?.source ?? '',
  );
  const [label, setLabel] = useState<string>(
    () => library.find((w) => w.id === selectedId)?.label ?? '',
  );
  const [dirty, setDirty] = useState(false);
  const [wasmReady, setWasmReady] = useState(false);
  const [parseStatus, setParseStatus] = useState<ParseStatus>(null);
  const [saving, setSaving] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

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

  // Keep ?warrior= in step with the user's picks so a reload (or a copied
  // link) reopens the same warrior. Untouched until they pick, so a pending
  // ?warrior= for a not-yet-synced saved warrior isn't overwritten.
  useEffect(() => {
    if (chosenId === null || searchParams.get('warrior') === chosenId) return;
    const next = new URLSearchParams(searchParams);
    next.set('warrior', chosenId);
    setSearchParams(next, { replace: true });
  }, [chosenId, searchParams, setSearchParams]);

  const selected = library.find((w) => w.id === selectedId);

  const presets = useMemo(() => library.filter((w) => w.isPreset), [library]);
  const userWarriors = useMemo(() => library.filter((w) => !w.isPreset), [library]);

  const syncFromWarrior = useCallback((warrior: Warrior) => {
    setSource(warrior.source);
    setLabel(warrior.label);
    setDirty(false);
  }, []);

  // Select a warrior that was just created and show its saved state.
  const selectWarrior = (warrior: Warrior) => {
    setSelectedId(warrior.id);
    syncFromWarrior(warrior);
  };

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
    // Monaco measures glyph widths once, at mount. If the self-hosted
    // JetBrains Mono finishes loading afterwards, the cached fallback-font
    // metrics misplace the cursor and selections until we remeasure.
    void document.fonts.ready.then(() => monaco.editor.remeasureFonts());
    if (wasmReady) runParse(editor.getValue());
  };

  // Every warriors-API call goes through here: it owns the `saving` flag and
  // turns a rejection into a message in the status bar instead of an
  // unhandled rejection from a click handler. Resolves to whether `action`
  // succeeded, so callers can skip whatever should only follow a success.
  const runServerAction = async (verb: string, action: () => Promise<void>): Promise<boolean> => {
    setSaving(true);
    setActionError(null);
    try {
      await action();
      return true;
    } catch (err) {
      setActionError(`Couldn't ${verb}: ${describeFailure(err)}`);
      return false;
    } finally {
      setSaving(false);
    }
  };

  const handleSelect = (id: string) => {
    if (dirty && !confirm('Discard unsaved changes?')) return;
    setActionError(null);
    setSelectedId(id);
    const warrior = library.find((w) => w.id === id);
    if (warrior) syncFromWarrior(warrior);
  };

  const handleSave = async () => {
    if (!selected || saving) return;
    await runServerAction('save warrior', async () => {
      if (selected.isPreset) {
        if (user) {
          const created = await createServerWarrior(orUntitled(label), source);
          selectWarrior(created);
        } else {
          const created = createUserWarrior(orUntitled(label), source);
          selectWarrior(created);
        }
        return;
      }
      if (user && isServerId(selected.id)) {
        await updateServerWarrior(selected.id, { label: orUntitled(label), source });
      } else if (user && !isServerId(selected.id)) {
        const created = await createServerWarrior(orUntitled(label), source);
        selectWarrior(created);
      } else {
        updateUserWarrior(selected.id, { label: orUntitled(label), source });
      }
      setDirty(false);
    });
  };

  const handleDuplicate = async () => {
    if (!selected || saving) return;
    if (user) {
      await runServerAction('duplicate warrior', async () => {
        const newLabel = `${selected.label} (copy)`;
        const created = await createServerWarrior(newLabel, selected.source);
        selectWarrior(created);
      });
    } else {
      const created = duplicateWarrior(selected.id);
      if (created) {
        selectWarrior(created);
      }
    }
  };

  const handleDelete = async () => {
    if (!selected || selected.isPreset || saving) return;
    if (!confirm(`Delete "${selected.label}"? This cannot be undone.`)) return;
    if (user && isServerId(selected.id)) {
      // On failure the warrior still exists, so keep it selected.
      const deleted = await runServerAction('delete warrior', () =>
        deleteServerWarrior(selected.id),
      );
      if (!deleted) return;
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
      await runServerAction('import warrior', async () => {
        const created = await createServerWarrior(name || 'Imported', content);
        selectWarrior(created);
      });
    } else {
      const created = createUserWarrior(name || 'Imported', content);
      selectWarrior(created);
    }
  };

  const handleNew = async () => {
    const template = `;name New Warrior
;author you
        ORG    start
start   MOV.I  $0, $1
`;
    if (user) {
      await runServerAction('create warrior', async () => {
        const created = await createServerWarrior('New Warrior', template);
        selectWarrior(created);
      });
    } else {
      const created = createUserWarrior('New Warrior', template);
      selectWarrior(created);
    }
  };

  // Battles read the warrior from the library by id, and navigating away
  // unmounts the Builder (dropping unsaved React state), so pending edits must
  // be written to the library first. For a saved server warrior that means
  // saving to the server — same as handleSave — so the battle runs exactly the
  // code the user is looking at. If that save fails, stay here with the edits
  // intact rather than silently battling the old version.
  const handleTestInBattle = async () => {
    if (!selected || saving) return;
    let targetId = selected.id;
    if (dirty) {
      if (selected.isPreset) {
        const created = createUserWarrior(orUntitled(label), source);
        targetId = created.id;
      } else if (isServerId(selected.id)) {
        const saved = await runServerAction('save warrior', async () => {
          await updateServerWarrior(selected.id, { label: orUntitled(label), source });
          setDirty(false);
        });
        if (!saved) return;
      } else {
        updateUserWarrior(selected.id, { label: orUntitled(label), source });
      }
    }
    const opponent =
      presets.find((w) => w.id !== targetId) ?? library.find((w) => w.id !== targetId);
    const opponentId = opponent?.id ?? targetId;
    navigate(`/battle?red=${encodeURIComponent(targetId)}&green=${encodeURIComponent(opponentId)}`);
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
    actionError,
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
