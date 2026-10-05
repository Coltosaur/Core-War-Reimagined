import { engineVersion } from 'core-war-engine';
import { warriorText } from '../../core/warriorColors';
import { resultBanner } from './styles';
import styles from './BattleStatus.module.css';

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

  // The tint follows the outcome (winner's color, tie, no winner), so it's
  // computed here rather than enumerated as classes.
  return (
    <div
      role="status"
      className={styles.banner}
      style={{
        color: banner.color,
        borderColor: `color-mix(in srgb, ${banner.color} 30%, transparent)`,
        backgroundColor: `color-mix(in srgb, ${banner.color} 7%, transparent)`,
        textShadow: `0 0 12px color-mix(in srgb, ${banner.color} 40%, transparent)`,
      }}
    >
      {banner.text}
    </div>
  );
}

export default function BattleStatus({
  ready,
  stepCount,
  warriors,
  resultCode,
  resultWinner,
}: Props) {
  if (!ready) {
    return <div className={styles.status}>Loading engine...</div>;
  }

  return (
    <div className={styles.status}>
      <div>Steps: {stepCount.toLocaleString()} / 80,000</div>
      <div className={styles.warriors}>
        {warriors.map((w, i) => (
          <span key={i} style={{ color: warriorText(i) }}>
            {w.name}: {w.alive ? `alive (${w.procs} proc${w.procs !== 1 ? 's' : ''})` : 'dead'}
          </span>
        ))}
      </div>
      <ResultBanner code={resultCode} winnerId={resultWinner} names={warriors.map((w) => w.name)} />
      <div className={styles.version}>Engine v{engineVersion()}</div>
    </div>
  );
}
