       IDENTIFICATION DIVISION.
       PROGRAM-ID. SYNTHETIC-DAVID-CORP.
       AUTHOR. SYNTHETIC-DAVID.
      * COBOL-85 STYLE CONTROL GATE. FOREIGN BROKER IS REQUIRED.
      * NO AUTOMATIC BANKING OR MIGRATION AUTHORITY.
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATE                PIC X(16).
       01 WS-COMMAND              PIC X(64).
       01 WS-BROKER-RC            PIC S9(4) COMP.
       01 WS-FAILED               PIC 9.
       LINKAGE SECTION.
           COPY 'request.cpy'.
           COPY 'result.cpy'.
       PROCEDURE DIVISION USING SD-REQUEST SD-RESULT.
       MAIN-CONTROL.
           INITIALIZE SD-RESULT
           MOVE ZERO TO WS-FAILED
           MOVE 'RUNNING' TO WS-STATE
           IF SD-REQUEST-ID = SPACES OR SD-TRACE-ID = SPACES
               OR SD-GRAPH-ID = SPACES OR SD-REQUESTOR = SPACES
               MOVE 'INVALID-IDENTITY' TO SD-ERROR-MESSAGE
               PERFORM HALT-REQUEST
           END-IF
           IF WS-FAILED = ZERO
               IF SD-PAYLOAD-LENGTH < 1
                   OR SD-PAYLOAD-LENGTH > 8192
                   MOVE 'INVALID-PAYLOAD-LENGTH'
                     TO SD-ERROR-MESSAGE
                   PERFORM HALT-REQUEST
               END-IF
           END-IF
           IF WS-FAILED = ZERO
               MOVE 'AUTHZ-AND-GRAPH-CHECK' TO WS-COMMAND
               PERFORM CALL-BROKER
           END-IF
           IF WS-FAILED = ZERO
               EVALUATE SD-REQUEST-TYPE
                 WHEN 'LEDGER-VALIDATE'
                   MOVE 'DOUBLE-ENTRY-VALIDATE' TO WS-COMMAND
                 WHEN 'MIGRATION-VALIDATE'
                   MOVE 'LEGACY-MODERN-DIFF' TO WS-COMMAND
                 WHEN 'DOCUMENT-INSPECT'
                   MOVE 'DOCUMENT-PARSE' TO WS-COMMAND
                 WHEN OTHER
                   MOVE 'UNKNOWN-REQUEST-TYPE'
                     TO SD-ERROR-MESSAGE
                   PERFORM HALT-REQUEST
               END-EVALUATE
           END-IF
           IF WS-FAILED = ZERO
               PERFORM CALL-BROKER
               IF WS-FAILED = ZERO
                   PERFORM VALIDATE-RESULT
               END-IF
           END-IF
           IF WS-FAILED = ZERO AND SD-RISK-SIGNAL > ZERO
               MOVE 'ESCALATED' TO WS-STATE
               MOVE 'FREEZE-AND-PERSIST' TO WS-COMMAND
               PERFORM CALL-BROKER
           END-IF
           IF WS-FAILED = ZERO AND WS-STATE = 'RUNNING'
               MOVE 'PROVENANCE-AUDIT-COMMIT' TO WS-COMMAND
               PERFORM CALL-BROKER
               IF WS-FAILED = ZERO
                   PERFORM VALIDATE-RESULT
               END-IF
               IF WS-FAILED = ZERO
                   MOVE 'COMPLETED' TO WS-STATE
               END-IF
           END-IF
           MOVE WS-STATE TO SD-RESULT-STATUS
           EXIT PROGRAM.
       CALL-BROKER.
           MOVE ZERO TO WS-BROKER-RC
           CALL 'SD-BROKER' USING WS-COMMAND SD-REQUEST
               SD-RESULT WS-BROKER-RC
               ON EXCEPTION
                   MOVE 16 TO WS-BROKER-RC
           END-CALL
           IF WS-BROKER-RC NOT = ZERO
               OR SD-RESULT-STATUS NOT = 'OK'
               MOVE 'BROKER-FAILED' TO SD-ERROR-MESSAGE
               PERFORM HALT-REQUEST
           END-IF.
       VALIDATE-RESULT.
           IF SD-EVIDENCE = SPACES OR SD-PROVENANCE-ID = SPACES
               OR SD-AGENT-ID = SPACES OR SD-TOOL-ID = SPACES
               OR SD-INPUT-HASH = SPACES
               OR SD-OUTPUT-HASH = SPACES
               OR SD-TIMESTAMP = SPACES
               OR SD-ERROR-CODE NOT = ZERO
               MOVE 'RESULT-CONTRACT-INVALID' TO SD-ERROR-MESSAGE
               PERFORM HALT-REQUEST
           END-IF.
       HALT-REQUEST.
           MOVE 1 TO WS-FAILED
           MOVE 'HALTED' TO WS-STATE
           MOVE 16 TO SD-ERROR-CODE.
       END PROGRAM SYNTHETIC-DAVID-CORP.
