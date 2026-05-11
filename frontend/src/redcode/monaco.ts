import type * as MonacoNs from 'monaco-editor';
import { OPCODES, MODIFIERS, ADDRESSING_MODES, PSEUDO_OPS, type CheatEntry } from './cheatSheet';

export const REDCODE_LANGUAGE_ID = 'redcode';

const OPCODE_SYMBOLS = [...OPCODES.map((e) => e.symbol), 'CMP'];
const PSEUDO_SYMBOLS = PSEUDO_OPS.map((e) => e.symbol);

let providersRegistered = false;

export function registerRedcode(monaco: typeof MonacoNs): void {
  const langs = monaco.languages.getLanguages();
  const alreadyRegistered = langs.some((l) => l.id === REDCODE_LANGUAGE_ID);

  if (!alreadyRegistered) {
    monaco.languages.register({ id: REDCODE_LANGUAGE_ID });
  }

  monaco.languages.setLanguageConfiguration(REDCODE_LANGUAGE_ID, {
    comments: { lineComment: ';' },
    brackets: [],
    autoClosingPairs: [],
    surroundingPairs: [],
  });

  monaco.languages.setMonarchTokensProvider(REDCODE_LANGUAGE_ID, {
    ignoreCase: true,
    defaultToken: '',
    opcodes: OPCODE_SYMBOLS,
    pseudo: PSEUDO_SYMBOLS,
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

  if (!providersRegistered) {
    registerCompletionProvider(monaco);
    registerHoverProvider(monaco);
    providersRegistered = true;
  }
}

function entryToCompletion(
  monaco: typeof MonacoNs,
  entry: CheatEntry,
  kind: MonacoNs.languages.CompletionItemKind,
  range: MonacoNs.IRange,
): MonacoNs.languages.CompletionItem {
  return {
    label: entry.symbol,
    kind,
    detail: entry.name,
    documentation: entry.desc,
    insertText: entry.symbol,
    range,
  };
}

function registerCompletionProvider(monaco: typeof MonacoNs): void {
  monaco.languages.registerCompletionItemProvider(REDCODE_LANGUAGE_ID, {
    triggerCharacters: ['.', '#', '$', '@', '*', '<', '>', '{', '}'],
    provideCompletionItems(model, position) {
      const word = model.getWordUntilPosition(position);
      const range: MonacoNs.IRange = {
        startLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endLineNumber: position.lineNumber,
        endColumn: word.endColumn,
      };

      const lineContent = model.getLineContent(position.lineNumber);
      const charBefore = lineContent[position.column - 2] ?? '';

      if (charBefore === '.') {
        const dotRange: MonacoNs.IRange = {
          ...range,
          startColumn: position.column - 1,
        };
        return {
          suggestions: MODIFIERS.map((m) =>
            entryToCompletion(monaco, m, monaco.languages.CompletionItemKind.Enum, dotRange),
          ),
        };
      }

      const suggestions: MonacoNs.languages.CompletionItem[] = [
        ...OPCODES.map((e) =>
          entryToCompletion(monaco, e, monaco.languages.CompletionItemKind.Keyword, range),
        ),
        ...PSEUDO_OPS.map((e) =>
          entryToCompletion(monaco, e, monaco.languages.CompletionItemKind.Function, range),
        ),
        ...ADDRESSING_MODES.map((e) =>
          entryToCompletion(monaco, e, monaco.languages.CompletionItemKind.Operator, range),
        ),
      ];

      return { suggestions };
    },
  });
}

function registerHoverProvider(monaco: typeof MonacoNs): void {
  const lookup = new Map<string, CheatEntry>();
  for (const list of [OPCODES, PSEUDO_OPS]) {
    for (const e of list) lookup.set(e.symbol.toUpperCase(), e);
  }

  monaco.languages.registerHoverProvider(REDCODE_LANGUAGE_ID, {
    provideHover(model, position) {
      const word = model.getWordAtPosition(position);
      if (word) {
        const entry = lookup.get(word.word.toUpperCase());
        if (entry) {
          return {
            range: new monaco.Range(
              position.lineNumber,
              word.startColumn,
              position.lineNumber,
              word.endColumn,
            ),
            contents: [{ value: `**${entry.symbol}** — ${entry.name}` }, { value: entry.desc }],
          };
        }
      }

      const lineContent = model.getLineContent(position.lineNumber);
      const col = position.column - 1;

      for (const mode of ADDRESSING_MODES) {
        if (lineContent[col] === mode.symbol) {
          return {
            range: new monaco.Range(position.lineNumber, col + 1, position.lineNumber, col + 2),
            contents: [{ value: `**${mode.symbol}** — ${mode.name}` }, { value: mode.desc }],
          };
        }
      }

      const dotMatch = lineContent.substring(0, position.column).match(/\.([A-Za-z]+)$/);
      if (dotMatch) {
        const modSym = '.' + dotMatch[1].toUpperCase();
        const mod = MODIFIERS.find((m) => m.symbol === modSym);
        if (mod) {
          const start = col - dotMatch[0].length + 1;
          return {
            range: new monaco.Range(
              position.lineNumber,
              start + 1,
              position.lineNumber,
              start + dotMatch[0].length + 1,
            ),
            contents: [{ value: `**${mod.symbol}** — ${mod.name}` }, { value: mod.desc }],
          };
        }
      }

      return null;
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
  const line = lineMatch ? Math.min(Math.max(parseInt(lineMatch[1], 10), 1), sourceLineCount) : 1;
  return {
    severity: monaco.MarkerSeverity.Error,
    message,
    startLineNumber: line,
    startColumn: 1,
    endLineNumber: line,
    endColumn: 1000,
  };
}
