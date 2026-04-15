import type * as MonacoNs from 'monaco-editor';

export const REDCODE_LANGUAGE_ID = 'redcode';

const OPCODES = [
  'DAT', 'MOV', 'ADD', 'SUB', 'MUL', 'DIV', 'MOD',
  'JMP', 'JMZ', 'JMN', 'DJN', 'SPL',
  'SEQ', 'SNE', 'SLT', 'NOP', 'CMP',
];

const PSEUDO = ['ORG', 'END', 'EQU'];

export function registerRedcode(monaco: typeof MonacoNs): void {
  const langs = monaco.languages.getLanguages();
  if (langs.some((l) => l.id === REDCODE_LANGUAGE_ID)) return;

  monaco.languages.register({ id: REDCODE_LANGUAGE_ID });

  monaco.languages.setLanguageConfiguration(REDCODE_LANGUAGE_ID, {
    comments: { lineComment: ';' },
    brackets: [],
    autoClosingPairs: [],
    surroundingPairs: [],
  });

  monaco.languages.setMonarchTokensProvider(REDCODE_LANGUAGE_ID, {
    ignoreCase: true,
    defaultToken: '',
    opcodes: OPCODES,
    pseudo: PSEUDO,
    tokenizer: {
      root: [
        [/;.*$/, 'comment'],
        [/[#$@*<>{}]/, 'operator'],
        [/\.[A-Za-z]+/, 'type'],
        [/-?\d+/, 'number'],
        [/,/, 'delimiter'],
        [
          /[A-Za-z_][A-Za-z0-9_]*/,
          {
            cases: {
              '@opcodes': 'keyword',
              '@pseudo': 'keyword.control',
              '@default': 'identifier',
            },
          },
        ],
        [/[ \t]+/, 'white'],
      ],
    },
  });

  monaco.editor.defineTheme('redcode-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'keyword', foreground: 'e94560', fontStyle: 'bold' },
      { token: 'keyword.control', foreground: 'f0c040', fontStyle: 'bold' },
      { token: 'type', foreground: '4fc3f7' },
      { token: 'operator', foreground: 'ffab00' },
      { token: 'number', foreground: 'a5d6a7' },
      { token: 'identifier', foreground: 'e0e0e0' },
      { token: 'comment', foreground: '666666', fontStyle: 'italic' },
      { token: 'delimiter', foreground: '888888' },
    ],
    colors: {
      'editor.background': '#0f0f0f',
      'editor.foreground': '#e0e0e0',
      'editorLineNumber.foreground': '#444',
      'editorCursor.foreground': '#e94560',
      'editor.selectionBackground': '#4fc3f733',
      'editor.lineHighlightBackground': '#1a1a1a',
    },
  });
}

// Parse-error messages from the Rust parser vary in shape. Try to extract a
// line number if one is mentioned; otherwise place the marker on line 1 so
// the user still sees *something* squiggled.
export function parseErrorToMarker(
  monaco: typeof MonacoNs,
  message: string,
  sourceLineCount: number,
): MonacoNs.editor.IMarkerData {
  const lineMatch = message.match(/line\s+(\d+)/i);
  const line = lineMatch
    ? Math.min(Math.max(parseInt(lineMatch[1], 10), 1), sourceLineCount)
    : 1;
  return {
    severity: monaco.MarkerSeverity.Error,
    message,
    startLineNumber: line,
    startColumn: 1,
    endLineNumber: line,
    endColumn: 1000,
  };
}
