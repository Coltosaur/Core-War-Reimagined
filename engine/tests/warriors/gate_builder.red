;name Gate Builder
;author Core War Reimagined
;strategy Uses FOR/ROF to build a wall of DAT bombs (a "gate")
;strategy at a fixed offset, then loops to maintain it.

        ORG start
count   EQU 4

        ; Build a gate of `count` DAT bombs at the target location.
        FOR count
gate    DAT.F #0, #0
        ROF

start   JMP   start
        END
