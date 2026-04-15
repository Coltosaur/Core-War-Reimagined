import { Link } from 'react-router-dom';

const PAGE_STYLE: React.CSSProperties = {
  maxWidth: '780px',
  margin: '0 auto',
  padding: '2.5rem 2rem 4rem',
  lineHeight: 1.65,
  color: '#d0d0d0',
};

const H1_STYLE: React.CSSProperties = {
  fontSize: '2rem',
  color: '#e94560',
  letterSpacing: '0.08em',
  margin: '0 0 0.25rem',
};

const H2_STYLE: React.CSSProperties = {
  fontSize: '1.25rem',
  color: '#4fc3f7',
  marginTop: '2.5rem',
  marginBottom: '0.75rem',
  paddingBottom: '0.3rem',
  borderBottom: '1px solid #222',
};

const H3_STYLE: React.CSSProperties = {
  fontSize: '1rem',
  color: '#f0c040',
  marginTop: '1.5rem',
  marginBottom: '0.4rem',
};

const LEDE_STYLE: React.CSSProperties = {
  fontSize: '0.95rem',
  color: '#bbb',
  marginBottom: '1.5rem',
};

const CODE_INLINE: React.CSSProperties = {
  fontFamily: '"JetBrains Mono", "Fira Code", monospace',
  fontSize: '0.88em',
  backgroundColor: '#1a1a1a',
  color: '#e94560',
  padding: '0.1rem 0.35rem',
  borderRadius: '3px',
  border: '1px solid #2a2a2a',
};

const CODE_BLOCK: React.CSSProperties = {
  fontFamily: '"JetBrains Mono", "Fira Code", monospace',
  fontSize: '0.85rem',
  backgroundColor: '#0f0f0f',
  color: '#e0e0e0',
  padding: '0.85rem 1rem',
  borderRadius: '4px',
  border: '1px solid #222',
  margin: '0.75rem 0',
  whiteSpace: 'pre',
  overflowX: 'auto',
  lineHeight: 1.5,
};

const CALLOUT_STYLE: React.CSSProperties = {
  backgroundColor: '#4fc3f711',
  border: '1px solid #4fc3f744',
  borderRadius: '4px',
  padding: '0.75rem 1rem',
  margin: '1rem 0',
  fontSize: '0.9rem',
};

const TABLE_STYLE: React.CSSProperties = {
  width: '100%',
  borderCollapse: 'collapse',
  fontSize: '0.88rem',
  margin: '0.75rem 0',
};

const TH_STYLE: React.CSSProperties = {
  textAlign: 'left',
  padding: '0.4rem 0.6rem',
  borderBottom: '1px solid #333',
  color: '#888',
  fontWeight: 600,
};

const TD_STYLE: React.CSSProperties = {
  padding: '0.35rem 0.6rem',
  borderBottom: '1px solid #1a1a1a',
  verticalAlign: 'top',
};

const C = ({ children }: { children: React.ReactNode }) => (
  <code style={CODE_INLINE}>{children}</code>
);

const LINK_STYLE: React.CSSProperties = {
  color: '#4fc3f7',
  textDecoration: 'none',
  borderBottom: '1px dashed #4fc3f766',
};

export default function LearnPage() {
  return (
    <div style={PAGE_STYLE}>
      <h1 style={H1_STYLE}>LEARN REDCODE</h1>
      <p style={LEDE_STYLE}>
        A practical introduction to writing warriors for Core War. By the end
        of this page you should understand the machine, be able to read the
        classic warriors, and have the vocabulary to start writing your own.
      </p>

      <h2 style={H2_STYLE}>1. The machine: MARS</h2>
      <p>
        MARS — the <strong>Memory Array Redcode Simulator</strong> — is a
        virtual computer with a <em>circular</em> memory of {8000} cells. Every
        cell is the same size and holds one Redcode instruction. Address 0 and
        address 7999 are adjacent; when you walk off the end, you wrap back
        around.
      </p>
      <p>
        A <strong>warrior</strong> is a Redcode program. Two warriors are
        loaded into different spots in the core, each with one
        <em> process</em> — essentially a program counter (PC) — pointing at
        its first instruction. The simulator ticks round-robin: one
        instruction from warrior A, then one from warrior B, then back to A.
      </p>
      <p>
        You <strong>win</strong> by being the last warrior with at least one
        live process. A process <strong>dies</strong> if it tries to execute a{' '}
        <C>DAT</C>, does an illegal divide-by-zero, or if the whole match
        hits the step limit without a winner (a tie).
      </p>

      <h2 style={H2_STYLE}>2. Anatomy of an instruction</h2>
      <p>Every Redcode instruction looks like this:</p>
      <pre style={CODE_BLOCK}>{`        MOV.I   #4, @bomb
        │   │   │   │
        │   │   │   └── B operand: address mode + value
        │   │   └────── A operand: address mode + value
        │   └────────── modifier (which fields to touch)
        └────────────── opcode (what to do)`}</pre>
      <p>
        Each of the four pieces is independent, which is why Redcode has so
        many variants: {'~16 opcodes × 7 modifiers × 8 addressing modes'} per
        operand adds up to a <em>lot</em> of possible instructions.
      </p>

      <h3 style={H3_STYLE}>Labels, comments, and pseudo-ops</h3>
      <pre style={CODE_BLOCK}>{`;name Hello            ; metadata: warrior name
;author you            ; metadata: author
        ORG    start   ; "start execution at label 'start'"
bomb    DAT.F  #0, #0  ; "bomb" is a label for this line
start   JMP    bomb    ; labels turn into relative offsets`}</pre>
      <p>
        Anything after a <C>;</C> is a comment. <C>;name</C> and <C>;author</C>{' '}
        are magic comments recognized as metadata. <C>ORG label</C> says
        "start executing at this label." Labels can also appear as the value
        of an operand — the assembler converts them into a relative offset
        from the line that uses them.
      </p>

      <h2 style={H2_STYLE}>3. Addressing modes</h2>
      <p>
        Each operand carries an addressing mode that decides how its numeric
        value is interpreted. This is where Redcode gets weird and fun.
      </p>
      <table style={TABLE_STYLE}>
        <thead>
          <tr>
            <th style={TH_STYLE}>Symbol</th>
            <th style={TH_STYLE}>Mode</th>
            <th style={TH_STYLE}>Meaning</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td style={TD_STYLE}>
              <C>#</C>
            </td>
            <td style={TD_STYLE}>Immediate</td>
            <td style={TD_STYLE}>The literal number. No dereference.</td>
          </tr>
          <tr>
            <td style={TD_STYLE}>
              <C>$</C>
            </td>
            <td style={TD_STYLE}>Direct (default)</td>
            <td style={TD_STYLE}>
              Offset from current PC. <C>$5</C> = cell five ahead.
            </td>
          </tr>
          <tr>
            <td style={TD_STYLE}>
              <C>*</C>
            </td>
            <td style={TD_STYLE}>A-indirect</td>
            <td style={TD_STYLE}>
              Use operand's value as an offset, go there, then follow{' '}
              <em>that</em> cell's A-field.
            </td>
          </tr>
          <tr>
            <td style={TD_STYLE}>
              <C>@</C>
            </td>
            <td style={TD_STYLE}>B-indirect</td>
            <td style={TD_STYLE}>
              Same as above, but follow the target cell's B-field.
            </td>
          </tr>
          <tr>
            <td style={TD_STYLE}>
              <C>{'{'}</C>
            </td>
            <td style={TD_STYLE}>A-predecrement</td>
            <td style={TD_STYLE}>
              Decrement the target cell's A-field <em>first</em>, then use it
              as the address.
            </td>
          </tr>
          <tr>
            <td style={TD_STYLE}>
              <C>{'<'}</C>
            </td>
            <td style={TD_STYLE}>B-predecrement</td>
            <td style={TD_STYLE}>
              Same, but decrement the B-field. Used heavily in replicators.
            </td>
          </tr>
          <tr>
            <td style={TD_STYLE}>
              <C>{'}'}</C>
            </td>
            <td style={TD_STYLE}>A-postincrement</td>
            <td style={TD_STYLE}>
              Use the A-field as the address, <em>then</em> increment it.
            </td>
          </tr>
          <tr>
            <td style={TD_STYLE}>
              <C>{'>'}</C>
            </td>
            <td style={TD_STYLE}>B-postincrement</td>
            <td style={TD_STYLE}>Same, but increment the B-field.</td>
          </tr>
        </tbody>
      </table>
      <div style={CALLOUT_STYLE}>
        <strong>Key idea:</strong> the predecrement/postincrement modes
        mutate the memory they look at. They're the backbone of warriors that
        need to advance a pointer with every copy (e.g. replicators).
      </div>

      <h2 style={H2_STYLE}>4. Modifiers</h2>
      <p>
        The dot suffix on an opcode decides <em>which fields</em> the
        operation touches. An instruction has an A-field and a B-field
        (ignoring opcode/modifier/address-mode bits); modifiers control how
        data flows between them.
      </p>
      <table style={TABLE_STYLE}>
        <thead>
          <tr>
            <th style={TH_STYLE}>Modifier</th>
            <th style={TH_STYLE}>Flow</th>
            <th style={TH_STYLE}>Typical use</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td style={TD_STYLE}>
              <C>.A</C> / <C>.B</C>
            </td>
            <td style={TD_STYLE}>Field-to-field within same index</td>
            <td style={TD_STYLE}>Targeted arithmetic.</td>
          </tr>
          <tr>
            <td style={TD_STYLE}>
              <C>.AB</C> / <C>.BA</C>
            </td>
            <td style={TD_STYLE}>A→B or B→A across</td>
            <td style={TD_STYLE}>
              Move a single value from one field into the other of another
              cell (e.g. <C>MOV.AB #8, ptr</C>).
            </td>
          </tr>
          <tr>
            <td style={TD_STYLE}>
              <C>.F</C>
            </td>
            <td style={TD_STYLE}>Both fields in parallel</td>
            <td style={TD_STYLE}>Bulk operations on two numbers at once.</td>
          </tr>
          <tr>
            <td style={TD_STYLE}>
              <C>.X</C>
            </td>
            <td style={TD_STYLE}>Both fields, crossed</td>
            <td style={TD_STYLE}>Rare. Sometimes useful for scanners.</td>
          </tr>
          <tr>
            <td style={TD_STYLE}>
              <C>.I</C>
            </td>
            <td style={TD_STYLE}>Whole instruction</td>
            <td style={TD_STYLE}>
              Copy opcode + modifier + both operands. The default for{' '}
              <C>MOV</C>.
            </td>
          </tr>
        </tbody>
      </table>

      <h2 style={H2_STYLE}>5. Walkthrough: Imp</h2>
      <p>
        The shortest meaningful warrior in the book. One line:
      </p>
      <pre style={CODE_BLOCK}>{`        MOV.I $0, $1`}</pre>
      <p>
        Let's dissect it. <C>MOV.I</C> copies a whole instruction. <C>$0</C>{' '}
        is "this cell." <C>$1</C> is "the next cell." So on each tick, the
        Imp copies itself forward by one. The process then advances its PC by
        one — which lands on a freshly-written Imp, and the whole thing
        repeats.
      </p>
      <p>
        The Imp is <em>hard to kill</em> because it's never standing still.
        By the time your bomb lands where it was, it has already moved on.
        It's also <em>hard to kill anything with</em> — it just leaves a
        trail of MOV.I everywhere.
      </p>

      <h2 style={H2_STYLE}>6. Walkthrough: Dwarf</h2>
      <p>The canonical "stone" — the basic bomber.</p>
      <pre style={CODE_BLOCK}>{`        ORG    start
start   ADD.AB #4, bomb
        MOV.I  bomb, @bomb
        JMP    start
bomb    DAT.F  #0, #0`}</pre>
      <p>
        Three instructions in a loop plus a payload. Each iteration:
      </p>
      <ol>
        <li>
          <C>ADD.AB #4, bomb</C> — add 4 into <em>bomb</em>'s B-field. So the
          B-field climbs: 0, 4, 8, 12, … Each time we're aiming at a
          different cell.
        </li>
        <li>
          <C>MOV.I bomb, @bomb</C> — copy bomb (a DAT) to the address stored
          in bomb's B-field. The <C>@</C> prefix says "look at bomb, then
          follow its B-field." So the DAT lands 4, 8, 12, … cells past bomb
          itself, never touching the Dwarf's own code.
        </li>
        <li>
          <C>JMP start</C> — start over.
        </li>
      </ol>
      <p>
        Because 4 is coprime with 8000, the Dwarf eventually bombs every
        single cell. Any enemy process that steps on one of those DATs dies.
        Simple, slow, effective.
      </p>

      <h2 style={H2_STYLE}>7. Three classic strategies</h2>
      <p>
        Once you're past the toy stage, warriors tend to fall into three
        broad families — the rock-paper-scissors of Core War.
      </p>

      <h3 style={H3_STYLE}>Stones (bombers)</h3>
      <p>
        Sit in one place, lob DATs at a fixed stride. Dwarf is the
        archetype. They beat <strong>papers</strong> by bombing into the
        replicated copies.
      </p>

      <h3 style={H3_STYLE}>Papers (replicators)</h3>
      <p>
        Copy themselves to another part of the core, spawn a new process
        there, then do it again. Mice is the archetype. They beat{' '}
        <strong>scanners</strong> by sheer numbers — the scanner can't find
        and bomb copies faster than the paper can make them.
      </p>

      <h3 style={H3_STYLE}>Scanners</h3>
      <p>
        Walk the core looking for anything non-empty and bomb it. Much more
        efficient than blind bombing. Scanner is the archetype. They beat
        <strong> stones</strong> because they can find and kill the bomber's
        single location before it has bombed enough cells to matter.
      </p>

      <div style={CALLOUT_STYLE}>
        <strong>So:</strong> stones → papers → scanners → stones. Picking a
        strategy against an unknown opponent is essentially a bet.
      </div>

      <h2 style={H2_STYLE}>8. Where to from here</h2>
      <p>
        Open the <Link to="/builder" style={LINK_STYLE}>Warrior Builder</Link>{' '}
        and pick a classic from the sidebar — every one is heavily commented
        now. Duplicate it, change a number (bomb stride, copy distance,
        split target), and hit <strong>Test in Battlefield</strong> to see
        what the change did.
      </p>
      <p>
        The cheat sheet on the right of the builder has every opcode,
        modifier, and addressing mode on one scrollable list. Keep it open
        while you read code.
      </p>
      <p style={{ marginTop: '2rem' }}>
        <Link to="/builder" style={LINK_STYLE}>
          → Go to the Warrior Builder
        </Link>
      </p>
    </div>
  );
}
