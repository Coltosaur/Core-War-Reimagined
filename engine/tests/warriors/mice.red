;name Mice
;author Chip Wendell
;strategy Self-replicator. Copies its own code to a remote location, forks a
;strategy process there via SPL, then advances the copy pointer and repeats.
;strategy Each remote copy independently does the same, producing exponential
;strategy replication across core. Won the first ICWS tournament in 1986.

        ORG    start
ptr     DAT.F  #0, #0
start   MOV.AB #8, ptr
loop    MOV.I  @ptr, <copy
        DJN.B  loop, ptr
        SPL    @copy, #0
        ADD.AB #653, copy
        JMZ.B  start, ptr
copy    DAT.F  #0, #833
