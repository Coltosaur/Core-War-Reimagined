import Editor from '@monaco-editor/react';
import { REDCODE_LANGUAGE_ID } from '../../redcode/monaco';
import CheatSheetPanel from '../../redcode/CheatSheetPanel';
import WarriorListPanel from './WarriorListPanel';
import EditorToolbar from './EditorToolbar';
import EditorStatus from './EditorStatus';
import { useBuilder } from './useBuilder';
import { EDITOR_CONTAINER_STYLE, MAIN_STYLE, PAGE_STYLE } from './styles';

export default function BuilderPage() {
  const {
    selectedId,
    selected,
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
          onLabelChange={handleLabelChange}
          onSave={handleSave}
          onDuplicate={handleDuplicate}
          onDelete={handleDelete}
          onTestInBattle={handleTestInBattle}
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
