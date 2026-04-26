import { engineVersion } from 'core-war-engine';
import {
  ENGINE_VERSION_STYLE,
  STATUS_ROW_STYLE,
  STATUS_STYLE,
  WARRIOR_HEX,
  resultBanner,
} from './styles';

type WarriorStatus = { name: string; alive: boolean; procs: number };

type Props = {
  ready: boolean;
  stepCount: number;
  warriors: WarriorStatus[];
  resultCode: number;
  resultWinner: number;
};

function ResultBanner({
  code,
  winnerId,
  names,
}: {
  code: number;
  winnerId: number;
  names: string[];
}) {
  const banner = resultBanner(code, winnerId, names);
  if (!banner) return null;

  const style: React.CSSProperties = {
    marginTop: '0.75rem',
    padding: '0.5rem 1.5rem',
    fontSize: '1.1rem',
    fontWeight: 600,
    letterSpacing: '0.05em',
    color: banner.color,
    border: `1px solid ${banner.color}44`,
    borderRadius: '6px',
    backgroundColor: `${banner.color}11`,
    textShadow: `0 0 12px ${banner.color}66`,
  };

  return <div style={style}>{banner.text}</div>;
}

export default function BattleStatus({
  ready,
  stepCount,
  warriors,
  resultCode,
  resultWinner,
}: Props) {
  if (!ready) {
    return (
      <div style={STATUS_STYLE}>
        <div>Loading engine...</div>
      </div>
    );
  }

  return (
    <div style={STATUS_STYLE}>
      <div>Steps: {stepCount.toLocaleString()} / 80,000</div>
      <div style={STATUS_ROW_STYLE}>
        {warriors.map((w, i) => (
          <span key={i} style={{ marginRight: '1.5rem', color: WARRIOR_HEX[i + 1] }}>
            {w.name}: {w.alive ? `alive (${w.procs} proc${w.procs !== 1 ? 's' : ''})` : 'dead'}
          </span>
        ))}
      </div>
      <ResultBanner code={resultCode} winnerId={resultWinner} names={warriors.map((w) => w.name)} />
      <div style={ENGINE_VERSION_STYLE}>Engine v{engineVersion()}</div>
    </div>
  );
}
