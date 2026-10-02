import Editor from '@monaco-editor/react';
import { REDCODE_LANGUAGE_ID } from '../../redcode/monaco';
import CheatSheetPanel from '../../redcode/CheatSheetPanel';
import WarriorListPanel from './WarriorListPanel';
import EditorToolbar from './EditorToolbar';
import EditorStatus from './EditorStatus';
import { useBuilder } from './useBuilder';
import { Link } from 'react-router-dom';
import { EDITOR_CONTAINER_STYLE, MAIN_STYLE, PAGE_STYLE } from './styles';
import { useIsNarrow } from '../../ui/useIsNarrow';

const NOTICE_STYLE: React.CSSProperties = {
  maxWidth: '28rem',
  margin: '0 auto',
  padding: '2.5rem 1.25rem',
  display: 'flex',
  flexDirection: 'column',
  gap: '1rem',
  textAlign: 'center',
  lineHeight: 1.5,
};

const NOTICE_TITLE_STYLE: React.CSSProperties = {
  margin: 0,
  fontSize: '1.1rem',
  letterSpacing: '0.1em',
};

const NOTICE_TEXT_STYLE: React.CSSProperties = {
  margin: 0,
  fontSize: '0.85rem',
  color: '#aaa',
};

const NOTICE_LINKS_STYLE: React.CSSProperties = {
  display: 'flex',
  gap: '0.75rem',
  justifyContent: 'center',
  flexWrap: 'wrap',
};

const NOTICE_LINK_STYLE: React.CSSProperties = {
  padding: '0.5rem 1rem',
  border: '1px solid #444',
  borderRadius: '4px',
  color: '#e0e0e0',
  backgroundColor: '#1e1e1e',
  textDecoration: 'none',
  fontSize: '0.85rem',
};

/** Shown instead of the editor below 768px (#81): Monaco plus the warrior
 *  list and cheat sheet don't fit a phone, and on-screen-keyboard editing of
 *  Redcode isn't a supported flow. */
export function BuilderDesktopNotice() {
  return (
    <div style={NOTICE_STYLE} role="note">
      <h1 style={NOTICE_TITLE_STYLE}>WARRIOR BUILDER</h1>
      <p style={NOTICE_TEXT_STYLE}>
        The Builder needs a larger screen. Open it on a desktop or laptop (768px or wider) to write
        and save warriors.
      </p>
      <p style={NOTICE_TEXT_STYLE}>In the meantime you can still watch battles or learn Redcode.</p>
      <div style={NOTICE_LINKS_STYLE}>
        <Link to="/battle" style={NOTICE_LINK_STYLE}>
          Battlefield
        </Link>
        <Link to="/learn" style={NOTICE_LINK_STYLE}>
          Learn Redcode
        </Link>
      </div>
    </div>
  );
}

export default function BuilderPage() {
  const narrow = useIsNarrow();
  return narrow ? <BuilderDesktopNotice /> : <BuilderWorkspace />;
}

function BuilderWorkspace() {
  const {
    selectedId,
    selected,
    source,
    label,
    dirty,
    wasmReady,
    parseStatus,
    presets,
    userWarriors,
    canSave,
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
  } = useBuilder();

  return (
    <div style={PAGE_STYLE}>
      <WarriorListPanel
        presets={presets}
        userWarriors={userWarriors}
        selectedId={selectedId}
        onSelect={handleSelect}
        onNew={handleNew}
      />

      <section style={MAIN_STYLE}>
        <EditorToolbar
          label={label}
          selected={selected}
          canSave={canSave}
          source={source}
          onLabelChange={handleLabelChange}
          onSave={handleSave}
          onDuplicate={handleDuplicate}
          onDelete={handleDelete}
          onTestInBattle={handleTestInBattle}
          onImport={handleImport}
        />

        <div style={EDITOR_CONTAINER_STYLE}>
          <Editor
            key={selectedId}
            height="100%"
            language={REDCODE_LANGUAGE_ID}
            theme="redcode-dark"
            defaultValue={selected?.source ?? ''}
            onChange={handleSourceChange}
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
              quickSuggestions: {
                other: 'on',
                comments: 'off',
                strings: 'off',
              },
              suggestOnTriggerCharacters: true,
              wordBasedSuggestions: 'off',
            }}
          />
        </div>

        <EditorStatus
          wasmReady={wasmReady}
          parseStatus={parseStatus}
          selected={selected}
          dirty={dirty}
        />
      </section>

      <CheatSheetPanel />
    </div>
  );
}
