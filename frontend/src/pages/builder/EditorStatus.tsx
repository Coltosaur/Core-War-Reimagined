import type { Warrior } from '../../warriors/library';
import type { ParseStatus } from './useBuilder';
import { STATUS_STYLE } from './styles';

const LOADING_STYLE: React.CSSProperties = { color: '#666' };
const SUCCESS_STYLE: React.CSSProperties = { color: '#a5d6a7' };
const ERROR_STYLE: React.CSSProperties = { color: '#e94560' };
const PRESET_HINT_STYLE: React.CSSProperties = {
  marginLeft: 'auto',
  color: '#666',
  fontStyle: 'italic',
};
const UNSAVED_STYLE: React.CSSProperties = {
  marginLeft: 'auto',
  color: '#f0c040',
};

type Props = {
  wasmReady: boolean;
  parseStatus: ParseStatus;
  selected: Warrior | undefined;
  dirty: boolean;
};

export default function EditorStatus({ wasmReady, parseStatus, selected, dirty }: Props) {
  return (
    <div style={STATUS_STYLE}>
      {!wasmReady && <span style={LOADING_STYLE}>Loading engine...</span>}
      {wasmReady && parseStatus?.ok && (
        <span style={SUCCESS_STYLE}>
          ✓ parsed
          {parseStatus.name ? ` — ${parseStatus.name}` : ''}
        </span>
      )}
      {wasmReady && parseStatus && !parseStatus.ok && (
        <span style={ERROR_STYLE}>✗ {parseStatus.message}</span>
      )}
      {selected?.isPreset && (
        <span style={PRESET_HINT_STYLE}>Classic warrior — read-only. Use Duplicate to edit.</span>
      )}
      {dirty && !selected?.isPreset && <span style={UNSAVED_STYLE}>● unsaved changes</span>}
    </div>
  );
}
