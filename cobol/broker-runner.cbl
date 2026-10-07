       IDENTIFICATION DIVISION.
       PROGRAM-ID. SD-NATIVE-RUN.
      * EXACT FIXED RECORD INPUT AND OUTPUT. NO JSON OR SHELL GLUE.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT NATIVE-INPUT ASSIGN TO NATIVEIN
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-INPUT-FS.
           SELECT NATIVE-OUTPUT ASSIGN TO NATIVEOUT
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-OUTPUT-FS.
       DATA DIVISION.
       FILE SECTION.
       FD NATIVE-INPUT RECORDING MODE IS F.
       01 NATIVE-INPUT-REC         PIC X(8412).
       FD NATIVE-OUTPUT RECORDING MODE IS F.
       01 NATIVE-OUTPUT-REC        PIC X(2579).
       WORKING-STORAGE SECTION.
       01 WS-INPUT-FS             PIC XX.
       01 WS-OUTPUT-FS            PIC XX.
       01 WS-FAILED               PIC 9 VALUE ZERO.
           COPY 'request.cpy'.
           COPY 'result.cpy'.
       PROCEDURE DIVISION.
       MAIN-CONTROL.
           INITIALIZE SD-REQUEST SD-RESULT
           MOVE 'SDABI001' TO SD-RESULT-ABI
           OPEN INPUT NATIVE-INPUT
           IF WS-INPUT-FS NOT = '00'
               MOVE 'INPUT-OPEN-FAILED' TO SD-ERROR-MESSAGE
               PERFORM INPUT-FAILED
           ELSE
               READ NATIVE-INPUT
               IF WS-INPUT-FS NOT = '00'
                   MOVE 'INPUT-RECORD-INVALID' TO SD-ERROR-MESSAGE
                   PERFORM INPUT-FAILED
               ELSE
                   MOVE NATIVE-INPUT-REC TO SD-REQUEST
                   READ NATIVE-INPUT
                   IF WS-INPUT-FS NOT = '10'
                       MOVE 'INPUT-FRAMING-INVALID'
                         TO SD-ERROR-MESSAGE
                       PERFORM INPUT-FAILED
                   END-IF
               END-IF
               CLOSE NATIVE-INPUT
           END-IF
           IF WS-FAILED = ZERO
               CALL 'SYNTHETIC-DAVID-CORP'
                   USING SD-REQUEST SD-RESULT
           END-IF
           MOVE SD-RESULT TO NATIVE-OUTPUT-REC
           OPEN OUTPUT NATIVE-OUTPUT
           IF WS-OUTPUT-FS = '00'
               WRITE NATIVE-OUTPUT-REC
               IF WS-OUTPUT-FS NOT = '00'
                   DISPLAY 'FAIL: NATIVE OUTPUT WRITE'
               END-IF
               CLOSE NATIVE-OUTPUT
           ELSE
               DISPLAY 'FAIL: NATIVE OUTPUT OPEN'
           END-IF
           STOP RUN.
       INPUT-FAILED.
           MOVE 1 TO WS-FAILED
           MOVE 'HALTED' TO SD-RESULT-STATUS
           MOVE 16 TO SD-ERROR-CODE.
       END PROGRAM SD-NATIVE-RUN.
