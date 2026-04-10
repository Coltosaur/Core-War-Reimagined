;name Linear Scanner
;strategy Walks core forward looking for a non-blank cell. When found,
;strategy bombs that location with a recognizable DAT and dies in a
;strategy landing pad. The bomb's B-field is set to 99 so the test can
;strategy verify the kill came from this scanner.

        ORG    loop
ptr     DAT.F  #0, #9
blank   DAT.F  #0, #0
bomb    DAT.F  #0, #99
loop    ADD.AB #1, ptr
        SEQ.I  @ptr, blank
        JMP    found
        JMP    loop
found   MOV.I  bomb, @ptr
landing DAT.F  #0, #0
        END
