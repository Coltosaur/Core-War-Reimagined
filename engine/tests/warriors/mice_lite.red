;name Mice-Lite
;strategy Simplified Mice-style replicator. Copies an imp template into three
;strategy consecutive cells using a DJN counter and predecrement-B addressing,
;strategy then dies in a DAT landing pad. A pedagogical fragment of the real
;strategy Mice (Chip Wendell, 1986) without the SPL/JMZ tail.

        ORG    loop
counter DAT.F  #0, #3
dest    DAT.F  #0, #8
imp     MOV.I  $0, $1
loop    MOV.I  imp, <dest
        DJN.B  loop, counter
landing DAT.F  #0, #0
        END
