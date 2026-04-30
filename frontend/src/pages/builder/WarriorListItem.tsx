import type { Warrior } from '../../warriors/library';
import { CLASSIC_BADGE_STYLE, LIST_ITEM_LABEL_STYLE, listItemStyle } from './styles';

type Props = {
  warrior: Warrior;
  active: boolean;
  onSelect: (id: string) => void;
};

export default function WarriorListItem({ warrior, active, onSelect }: Props) {
  return (
    <div style={listItemStyle(active, warrior.isPreset)} onClick={() => onSelect(warrior.id)}>
      <span style={LIST_ITEM_LABEL_STYLE}>{warrior.label}</span>
      {warrior.isPreset && <span style={CLASSIC_BADGE_STYLE}>CLASSIC</span>}
    </div>
  );
}
