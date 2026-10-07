       IDENTIFICATION DIVISION.
       PROGRAM-ID. SYNTHETIC-DAVID-CORP.
       AUTHOR. SYNTHETIC-DAVID.
      * SDABI001 ASCII DISPLAY FIELDS. GNUCOBOL C ABI TARGET.
      * ONE ATOMIC READ-ONLY VALIDATION AND EVIDENCE COMMIT.
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-COMMAND              PIC X(64).
       01 WS-BROKER-RC            PIC 9(4).
       01 WS-FAILED               PIC 9.
       LINKAGE SECTION.
           COPY 'request.cpy'.
           COPY 'result.cpy'.
       PROCEDURE DIVISION USING SD-REQUEST SD-RESULT.
       MAIN-CONTROL.
           INITIALIZE SD-RESULT
           MOVE 'SDABI001' TO SD-RESULT-ABI
           MOVE ZERO TO WS-FAILED
           IF SD-ABI-VERSION NOT = 'SDABI001'
               MOVE 'ABI-VERSION-UNSUPPORTED' TO SD-ERROR-MESSAGE
               PERFORM HALT-REQUEST
           END-IF
           IF WS-FAILED = ZERO
               IF SD-REQUEST-ID = SPACES OR SD-TRACE-ID = SPACES
                   OR SD-GRAPH-ID = SPACES OR SD-REQUESTOR = SPACES
                   MOVE 'INVALID-IDENTITY' TO SD-ERROR-MESSAGE
                   PERFORM HALT-REQUEST
               END-IF
           END-IF
           IF WS-FAILED = ZERO
               IF SD-PAYLOAD-LENGTH IS NOT NUMERIC
                   MOVE 'INVALID-PAYLOAD-LENGTH'
                     TO SD-ERROR-MESSAGE
                   PERFORM HALT-REQUEST
               ELSE
                   IF SD-PAYLOAD-LENGTH < 1
                       OR SD-PAYLOAD-LENGTH > 8192
                       MOVE 'INVALID-PAYLOAD-LENGTH'
                         TO SD-ERROR-MESSAGE
                       PERFORM HALT-REQUEST
                   END-IF
               END-IF
           END-IF
           IF WS-FAILED = ZERO
               EVALUATE SD-REQUEST-TYPE
                 WHEN 'LEDGER-VALIDATE'
                 WHEN 'MIGRATION-VALIDATE'
                 WHEN 'DOCUMENT-INSPECT'
                 WHEN 'COBOL-INSPECT'
                   CONTINUE
                 WHEN OTHER
                   MOVE 'UNKNOWN-REQUEST-TYPE'
                     TO SD-ERROR-MESSAGE
                   PERFORM HALT-REQUEST
               END-EVALUATE
           END-IF
           IF WS-FAILED = ZERO
               MOVE 'EXECUTE-AND-COMMIT' TO WS-COMMAND
               MOVE ZERO TO WS-BROKER-RC
               CALL 'SD_BROKER' USING WS-COMMAND SD-REQUEST
                   SD-RESULT WS-BROKER-RC
                   ON EXCEPTION
                       MOVE 16 TO WS-BROKER-RC
                       MOVE 'BROKER-UNAVAILABLE'
                         TO SD-ERROR-MESSAGE
               END-CALL
               IF WS-BROKER-RC IS NOT NUMERIC
                   MOVE 'BROKER-RC-INVALID' TO SD-ERROR-MESSAGE
                   PERFORM HALT-REQUEST
               ELSE
                   IF WS-BROKER-RC NOT = ZERO
                       IF SD-ERROR-MESSAGE = SPACES
                           MOVE 'BROKER-FAILED'
                             TO SD-ERROR-MESSAGE
                       END-IF
                       PERFORM HALT-REQUEST
                   END-IF
               END-IF
           END-IF
           IF WS-FAILED = ZERO
               IF SD-RESULT-ABI NOT = 'SDABI001'
                   MOVE 'RESULT-ABI-INVALID' TO SD-ERROR-MESSAGE
                   PERFORM HALT-REQUEST
               ELSE
                   EVALUATE SD-RESULT-STATUS
                     WHEN 'COMPLETED'
                       PERFORM VALIDATE-RESULT
                     WHEN 'ESCALATED'
                       IF SD-RISK-SIGNAL IS NOT NUMERIC
                           MOVE 'RESULT-RISK-INVALID'
                             TO SD-ERROR-MESSAGE
                           PERFORM HALT-REQUEST
                       ELSE
                           IF SD-RISK-SIGNAL = ZERO
                               OR SD-EVIDENCE = SPACES
                               MOVE 'ESCALATION-CONTRACT-INVALID'
                                 TO SD-ERROR-MESSAGE
                               PERFORM HALT-REQUEST
                           END-IF
                       END-IF
                     WHEN OTHER
                       MOVE 'BROKER-STATUS-INVALID'
                         TO SD-ERROR-MESSAGE
                       PERFORM HALT-REQUEST
                   END-EVALUATE
               END-IF
           END-IF
           EXIT PROGRAM.
       VALIDATE-RESULT.
           IF SD-EVIDENCE = SPACES OR SD-PROVENANCE-ID = SPACES
               OR SD-AGENT-ID = SPACES OR SD-TOOL-ID = SPACES
               OR SD-INPUT-HASH = SPACES
               OR SD-OUTPUT-HASH = SPACES
               OR SD-TIMESTAMP = SPACES
               MOVE 'RESULT-CONTRACT-INVALID' TO SD-ERROR-MESSAGE
               PERFORM HALT-REQUEST
           END-IF
           IF WS-FAILED = ZERO
               IF SD-ERROR-CODE IS NOT NUMERIC
                   OR SD-RISK-SIGNAL IS NOT NUMERIC
                   OR SD-UNCERTAINTY IS NOT NUMERIC
                   MOVE 'RESULT-NUMERIC-INVALID'
                     TO SD-ERROR-MESSAGE
                   PERFORM HALT-REQUEST
               ELSE
                   IF SD-ERROR-CODE NOT = ZERO
                       OR SD-RISK-SIGNAL NOT = ZERO
                       MOVE 'RESULT-CONTRACT-INVALID'
                         TO SD-ERROR-MESSAGE
                       PERFORM HALT-REQUEST
                   END-IF
               END-IF
           END-IF.
       HALT-REQUEST.
           MOVE 1 TO WS-FAILED
           MOVE 'HALTED' TO SD-RESULT-STATUS
           MOVE 16 TO SD-ERROR-CODE.
       END PROGRAM SYNTHETIC-DAVID-CORP.
