# The GitHub plan

The project's plan on GitHub (crowdwave/neoteletext-engine) is created from these files, so it can be reviewed, versioned and re-run.

- `plan_data_1.py` to `plan_data_3.py`: the parent issues (one per part of the project) with their goal, how other browsers do it, security lessons, constraints and sources, and their tasks with done-when and security criteria; then the decision and gap issues.
- `create_issues.py`: creates the labels, issues, milestones and sub-issue links with `gh`. It is idempotent: `state.json` records every issue it created, so running it again creates only what is missing.

```sh
python3 -I scripts/github-plan/create_issues.py --dry-run   # count, change nothing
python3 -I scripts/github-plan/create_issues.py             # create what is missing
```

The research behind the "how other browsers do it" sections examined Chromium, Firefox, WebKit/Safari, Ladybird, Servo/servoshell, Verso, NetSurf, Dillo, Lagrange, Kristall and Amfora (11 October 2026). Security is the overriding priority of every part.
