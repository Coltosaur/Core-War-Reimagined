import Editor from '@monaco-editor/react';
import { REDCODE_LANGUAGE_ID } from '../../redcode/monaco';
import CheatSheetPanel from '../../redcode/CheatSheetPanel';
import WarriorListPanel from './WarriorListPanel';
import EditorToolbar from './EditorToolbar';
import EditorStatus from './EditorStatus';
import { useBuilder } from './useBuilder';
import { fontMono } from '../../styles/tokens';
import styles from './BuilderPage.module.css';

export default function BuilderPage() {
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
    <div className={styles.page}>
      <WarriorListPanel
        presets={presets}
        userWarriors={userWarriors}
        selectedId={selectedId}
        onSelect={handleSelect}
        onNew={handleNew}
      />

      <section className={styles.main}>
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

        <div className={styles.editor}>
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
              fontFamily: fontMono,
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
