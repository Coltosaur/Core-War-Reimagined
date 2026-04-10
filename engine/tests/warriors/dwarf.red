;name Dwarf
;author A.K. Dewdney
;strategy A "stone" warrior. Bombs core at intervals of 4, advancing the bomb
;strategy pointer each iteration. The first canonical Core War warrior.

        ORG    start
start   ADD.AB #4, bomb
        MOV.I  bomb, @bomb
        JMP    start
bomb    DAT.F  #0, #0
        END
