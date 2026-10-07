:- module(david_policy, [minimum_set/4, double_entry_ok/2,
                         migration_ok/3, risk_eval/2]).
:- use_module(library(lists)).
% GPU/vector/LLM scores may propose candidates, never permissions.
agent('LEDGER', 'ledger.validate', 'ledger:read', []).
agent('MIG-VALID', 'migration.validate', 'migration:read', []).
agent('DOCUMENT', 'document.inspect', 'document:read', []).

minimum_set(Required, Candidates, Permissions, Selected) :-
    ground(Required-Candidates-Permissions),
    is_list(Required), Required \= [], is_list(Candidates),
    is_list(Permissions), maplist(atom, Required),
    maplist(atom, Candidates), maplist(atom, Permissions),
    length(Candidates, Count), Count =< 64,
    sort(Candidates, Unique), length(Unique, Count),
    maplist(known_agent, Unique),
    findall(Size-Set,
        (subset_of(Unique, Set), Set \= [],
         maplist(permitted(Permissions), Set),
         maplist(covered(Set), Required),
         maplist(dependencies_met(Set), Set), length(Set, Size)),
        Solutions),
    sort(Solutions, [_-Selected|_]).
known_agent(A) :- agent(A, _, _, _).
permitted(P, A) :- agent(A, _, Permission, _), memberchk(Permission, P).
covered(Set, Capability) :- member(A, Set), agent(A, Capability, _, _), !.
dependencies_met(Set, A) :- agent(A, _, _, Dependencies),
    forall(member(D, Dependencies), memberchk(D, Set)).
subset_of([], []).
subset_of([H|T], [H|S]) :- subset_of(T, S).
subset_of([_|T], S) :- subset_of(T, S).
% Values are integer units of 1/10000, not binary floating point.
double_entry_ok(Debits, Credits) :- integer(Debits), integer(Credits),
    Debits >= 0, Credits >= 0, Debits =:= Credits.
migration_ok(Legacy, Modern, 'EXACT-CANONICAL-TERM-V1') :-
    ground(Legacy), ground(Modern), Legacy == Modern.
risk_eval(Flags, Total) :- is_list(Flags),
    maplist(valid_risk, Flags), sum_list(Flags, Total).
valid_risk(R) :- integer(R), R >= 0, R =< 9999.
