import { Link } from 'react-router-dom';
import styles from './LearnPage.module.css';

export default function LearnPage() {
  return (
    <div className={`prose ${styles.page}`}>
      <h1 className={styles.title}>LEARN REDCODE</h1>
      <p className={styles.lede}>
        A practical introduction to writing warriors for Core War. By the end of this page you
        should understand the machine, be able to read the classic warriors, and have the vocabulary
        to start writing your own.
      </p>

      <h2 className={styles.section}>1. The machine: MARS</h2>
      <p>
        MARS — the <strong>Memory Array Redcode Simulator</strong> — is a virtual computer with a{' '}
        <em>circular</em> memory of {8000} cells. Every cell is the same size and holds one Redcode
        instruction. Address 0 and address 7999 are adjacent; when you walk off the end, you wrap
        back around.
      </p>
      <p>
        A <strong>warrior</strong> is a Redcode program. Two warriors are loaded into different
        spots in the core, each with one
        <em> process</em> — essentially a program counter (PC) — pointing at its first instruction.
        The simulator ticks round-robin: one instruction from warrior A, then one from warrior B,
        then back to A.
      </p>
      <p>
        You <strong>win</strong> by being the last warrior with at least one live process. A process{' '}
        <strong>dies</strong> if it tries to execute a <code>DAT</code>, does an illegal
        divide-by-zero, or if the whole match hits the step limit without a winner (a tie).
      </p>

      <h2 className={styles.section}>2. Anatomy of an instruction</h2>
      <p>Every Redcode instruction looks like this:</p>
      <pre className={styles.code}>{`        MOV.I   #4, @bomb
        │   │   │   │
        │   │   │   └── B operand: address mode + value
        │   │   └────── A operand: address mode + value
        │   └────────── modifier (which fields to touch)
        └────────────── opcode (what to do)`}</pre>
      <p>
        Each of the four pieces is independent, which is why Redcode has so many variants:{' '}
        {'~16 opcodes × 7 modifiers × 8 addressing modes'} per operand adds up to a <em>lot</em> of
        possible instructions.
      </p>

      <h3 className={styles.subsection}>Labels, comments, and pseudo-ops</h3>
      <pre className={styles.code}>{`;name Hello            ; metadata: warrior name
;author you            ; metadata: author
        ORG    start   ; "start execution at label 'start'"
bomb    DAT.F  #0, #0  ; "bomb" is a label for this line
start   JMP    bomb    ; labels turn into relative offsets`}</pre>
      <p>
        Anything after a <code>;</code> is a comment. <code>;name</code> and <code>;author</code>{' '}
        are magic comments recognized as metadata. <code>ORG label</code> says "start executing at
        this label." Labels can also appear as the value of an operand — the assembler converts them
        into a relative offset from the line that uses them.
      </p>

      <h2 className={styles.section}>3. Addressing modes</h2>
      <p>
        Each operand carries an addressing mode that decides how its numeric value is interpreted.
        This is where Redcode gets weird and fun.
      </p>
      <div className={styles.tableWrap}>
        <table className={styles.table}>
          <thead>
            <tr>
              <th>Symbol</th>
              <th>Mode</th>
              <th>Meaning</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>
                <code>#</code>
              </td>
              <td>Immediate</td>
              <td>The literal number. No dereference.</td>
            </tr>
            <tr>
              <td>
                <code>$</code>
              </td>
              <td>Direct (default)</td>
              <td>
                Offset from current PC. <code>$5</code> = cell five ahead.
              </td>
            </tr>
            <tr>
              <td>
                <code>*</code>
              </td>
              <td className={styles.term}>A-indirect</td>
              <td>
                Use operand's value as an offset, go there, then follow <em>that</em> cell's
                A-field.
              </td>
            </tr>
            <tr>
              <td>
                <code>@</code>
              </td>
              <td className={styles.term}>B-indirect</td>
              <td>Same as above, but follow the target cell's B-field.</td>
            </tr>
            <tr>
              <td>
                <code>{'{'}</code>
              </td>
              <td className={styles.term}>A-predecrement</td>
              <td>
                Decrement the target cell's A-field <em>first</em>, then use it as the address.
              </td>
            </tr>
            <tr>
              <td>
                <code>{'<'}</code>
              </td>
              <td className={styles.term}>B-predecrement</td>
              <td>Same, but decrement the B-field. Used heavily in replicators.</td>
            </tr>
            <tr>
              <td>
                <code>{'}'}</code>
              </td>
              <td className={styles.term}>A-postincrement</td>
              <td>
                Use the A-field as the address, <em>then</em> increment it.
              </td>
            </tr>
            <tr>
              <td>
                <code>{'>'}</code>
              </td>
              <td className={styles.term}>B-postincrement</td>
              <td>Same, but increment the B-field.</td>
            </tr>
          </tbody>
        </table>
      </div>
      <div className={styles.callout}>
        <strong>Key idea:</strong> the predecrement/postincrement modes mutate the memory they look
        at. They're the backbone of warriors that need to advance a pointer with every copy (e.g.
        replicators).
      </div>

      <h2 className={styles.section}>4. Modifiers</h2>
      <p>
        The dot suffix on an opcode decides <em>which fields</em> the operation touches. An
        instruction has an A-field and a B-field (ignoring opcode/modifier/address-mode bits);
        modifiers control how data flows between them.
      </p>
      <div className={styles.tableWrap}>
        <table className={styles.table}>
          <thead>
            <tr>
              <th>Modifier</th>
              <th>Flow</th>
              <th>Typical use</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>
                <code>.A</code> / <code>.B</code>
              </td>
              <td>Field-to-field within same index</td>
              <td>Targeted arithmetic.</td>
            </tr>
            <tr>
              <td>
                <code>.AB</code> / <code>.BA</code>
              </td>
              <td>A→B or B→A across</td>
              <td>
                Move a single value from one field into the other of another cell (e.g.{' '}
                <code>MOV.AB #8, ptr</code>).
              </td>
            </tr>
            <tr>
              <td>
                <code>.F</code>
              </td>
              <td>Both fields in parallel</td>
              <td>Bulk operations on two numbers at once.</td>
            </tr>
            <tr>
              <td>
                <code>.X</code>
              </td>
              <td>Both fields, crossed</td>
              <td>Rare. Sometimes useful for scanners.</td>
            </tr>
            <tr>
              <td>
                <code>.I</code>
              </td>
              <td>Whole instruction</td>
              <td>
                Copy opcode + modifier + both operands. The default for <code>MOV</code>.
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <h2 className={styles.section}>5. Walkthrough: Imp</h2>
      <p>The shortest meaningful warrior in the book. One line:</p>
      <pre className={styles.code}>{`        MOV.I $0, $1`}</pre>
      <p>
        Let's dissect it. <code>MOV.I</code> copies a whole instruction. <code>$0</code> is "this
        cell." <code>$1</code> is "the next cell." So on each tick, the Imp copies itself forward by
        one. The process then advances its PC by one — which lands on a freshly-written Imp, and the
        whole thing repeats.
      </p>
      <p>
        The Imp is <em>hard to kill</em> because it's never standing still. By the time your bomb
        lands where it was, it has already moved on. It's also <em>hard to kill anything with</em> —
        it just leaves a trail of MOV.I everywhere.
      </p>

      <h2 className={styles.section}>6. Walkthrough: Dwarf</h2>
      <p>The canonical "stone" — the basic bomber.</p>
      <pre className={styles.code}>{`        ORG    start
start   ADD.AB #4, bomb
        MOV.I  bomb, @bomb
        JMP    start
bomb    DAT.F  #0, #0`}</pre>
      <p>Three instructions in a loop plus a payload. Each iteration:</p>
      <ol>
        <li>
          <code>ADD.AB #4, bomb</code> — add 4 into <em>bomb</em>'s B-field. So the B-field climbs:
          0, 4, 8, 12, … Each time we're aiming at a different cell.
        </li>
        <li>
          <code>MOV.I bomb, @bomb</code> — copy bomb (a DAT) to the address stored in bomb's
          B-field. The <code>@</code> prefix says "look at bomb, then follow its B-field." So the
          DAT lands 4, 8, 12, … cells past bomb itself, never touching the Dwarf's own code.
        </li>
        <li>
          <code>JMP start</code> — start over.
        </li>
      </ol>
      <p>
        Because 4 is coprime with 8000, the Dwarf eventually bombs every single cell. Any enemy
        process that steps on one of those DATs dies. Simple, slow, effective.
      </p>

      <h2 className={styles.section}>7. Three classic strategies</h2>
      <p>
        Once you're past the toy stage, warriors tend to fall into three broad families — the
        rock-paper-scissors of Core War.
      </p>

      <h3 className={styles.subsection}>Stones (bombers)</h3>
      <p>
        Sit in one place, lob DATs at a fixed stride. Dwarf is the archetype. They beat{' '}
        <strong>papers</strong> by bombing into the replicated copies.
      </p>

      <h3 className={styles.subsection}>Papers (replicators)</h3>
      <p>
        Copy themselves to another part of the core, spawn a new process there, then do it again.
        Mice is the archetype. They beat <strong>scanners</strong> by sheer numbers — the scanner
        can't find and bomb copies faster than the paper can make them.
      </p>

      <h3 className={styles.subsection}>Scanners</h3>
      <p>
        Walk the core looking for anything non-empty and bomb it. Much more efficient than blind
        bombing. Scanner is the archetype. They beat
        <strong> stones</strong> because they can find and kill the bomber's single location before
        it has bombed enough cells to matter.
      </p>

      <div className={styles.callout}>
        <strong>So:</strong> stones → papers → scanners → stones. Picking a strategy against an
        unknown opponent is essentially a bet.
      </div>

      <h2 className={styles.section}>8. Where to from here</h2>
      <p>
        Open the <Link to="/builder">Warrior Builder</Link> and pick a classic from the sidebar —
        every one is heavily commented now. Duplicate it, change a number (bomb stride, copy
        distance, split target), and hit <strong>Test in Battlefield</strong> to see what the change
        did.
      </p>
      <p>
        The builder's Redcode Cheat Sheet panel has every opcode, modifier, and addressing mode on
        one scrollable list. Keep it open while you read code.
      </p>
      <p className={styles.footer}>
        <Link to="/builder">→ Go to the Warrior Builder</Link>
      </p>
    </div>
  );
}
