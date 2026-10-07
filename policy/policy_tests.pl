:- begin_tests(david_policy).
:- use_module(policy).
test(minimum) :- minimum_set(['ledger.validate'], ['DOCUMENT','LEDGER'],
    ['ledger:read','document:read'], ['LEDGER']).
test(permission, [fail]) :- minimum_set(['ledger.validate'], ['LEDGER'], [], _).
test(cobol_minimum) :- minimum_set(['legacy.cobol.inspect'], ['DOCUMENT','COBOL-ANALYZER'],
    ['document:read','legacy:cobol:read'], ['COBOL-ANALYZER']).
test(cobol_permission, [fail]) :- minimum_set(['legacy.cobol.inspect'], ['COBOL-ANALYZER'],
    ['document:read'], _).
test(unknown_agent, [fail]) :- minimum_set(['ledger.validate'], ['UNKNOWN'], ['ledger:read'], _).
test(exact_balance) :- double_entry_ok(100000, 100000).
test(no_tolerance, [fail]) :- double_entry_ok(100000, 100001).
test(no_float, [fail]) :- double_entry_ok(1.0, 1.0).
test(divergence, [fail]) :- migration_ok([balance(1)], [balance(2)], 'EXACT-CANONICAL-TERM-V1').
:- end_tests(david_policy).
