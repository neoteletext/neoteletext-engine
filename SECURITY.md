# Security policy

Security overrides every other requirement of this project (REQUIREMENTS.md decision E.32). This file is the reporting process REQ-SEC-023 requires.

## Reporting a vulnerability

Please report vulnerabilities privately to the project owner by email, not in a public issue. Include:

- what an attacker can do, and against which asset of `THREAT-MODEL.md`;
- the input or steps that show it (a short file, page or captured exchange is ideal);
- the version or commit you tested.

## What happens next

1. The report is acknowledged within seven days.
2. The fault is reproduced, and a regression test is written that fails on the faulty code (REQ-SEC-023).
3. The fix and the test are released together, with a note crediting the reporter unless they prefer otherwise.
4. `THREAT-MODEL.md` is updated if the fault shows a gap in the model.

## Release gate

A release requires (REQ-SEC-023):

- every requirement of area SEC passing its tests;
- no open security advisories against any dependency;
- clean fuzzing for a stated minimum time;
- the software bill of materials published.
