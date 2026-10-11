"""Creates the GitHub plan: labels, epics, sub-issues, decisions. Idempotent through state.json."""
import json, os, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import plan_data_1  # noqa: E402
import plan_data_2  # noqa: E402,F401
import plan_data_3  # noqa: E402

REPO = "crowdwave/neoteletext"
DRY = "--dry-run" in sys.argv
STATE = os.path.join(HERE, "state.json")
state = json.load(open(STATE)) if os.path.exists(STATE) else {}

EXTRA_LABELS = {
    "epic": "5319e7", "area/requirements": "c2e0c6", "area/process": "c2e0c6", "area/network": "c2e0c6",
    "area/navigation": "c2e0c6", "area/address": "c2e0c6", "area/trusted-ui": "c2e0c6", "area/input": "c2e0c6",
    "area/ipc": "c2e0c6", "area/sandbox": "c2e0c6", "area/ui": "c2e0c6", "area/rendering": "c2e0c6",
    "area/a11y": "c2e0c6", "area/storage": "c2e0c6", "area/packaging": "c2e0c6", "area/testing": "c2e0c6",
    "os/macos": "ededed", "os/windows": "ededed", "os/linux": "ededed",
}


def gh(*args, input_text=None):
    if DRY:
        return ""
    for attempt in range(5):
        r = subprocess.run(["gh", *args], capture_output=True, text=True, input=input_text)
        if r.returncode == 0:
            return r.stdout
        if "rate limit" in r.stderr.lower() or "secondary" in r.stderr.lower():
            time.sleep(60 * (attempt + 1))
            continue
        raise SystemExit(f"gh {' '.join(args[:3])} failed: {r.stderr}")
    raise SystemExit("gave up after rate limits")


def save():
    if not DRY:
        json.dump(state, open(STATE, "w"), indent=1)


def milestones():
    if DRY:
        return {}
    out = json.loads(gh("api", f"repos/{REPO}/milestones?state=all&per_page=100"))
    return {m["title"]: m["number"] for m in out}


def create_issue(key, title, body, labels, milestone, ms):
    if key in state:
        return state[key]
    args = ["issue", "create", "--repo", REPO, "--title", title, "--body-file", "-"]
    for label in labels:
        args += ["--label", label]
    if milestone:
        args += ["--milestone", milestone]
    url = gh(*args, input_text=body).strip()
    number = int(url.rsplit("/", 1)[-1]) if url else 0
    issue_id = json.loads(gh("api", f"repos/{REPO}/issues/{number}"))["id"] if number else 0
    state[key] = {"number": number, "id": issue_id}
    save()
    if not DRY:
        time.sleep(1)
    return state[key]


def link(parent, child, key):
    if key in state.get("_links", {}) or DRY:
        return
    gh("api", "-X", "POST", f"repos/{REPO}/issues/{parent['number']}/sub_issues", "-F", f"sub_issue_id={child['id']}")
    state.setdefault("_links", {})[key] = True
    save()


def bullets(items):
    return "\n".join(f"- {i}" for i in items) if items else "- None."


def main():
    for name, colour in EXTRA_LABELS.items():
        gh("label", "create", name, "--repo", REPO, "--color", colour, "--force")
    ms = milestones()
    count = 0
    for e in plan_data_1.EPICS:
        body = (f"## Goal\n\n{e['goal']}\n\n"
                f"## How other browsers do it\n\nExamined: {plan_data_1.BROWSERS}.\n\n{bullets(e['prior_art'])}\n\n"
                f"## Security\n\nSecurity is the overriding priority of this project.\n\n{bullets(e['security'])}\n\n"
                f"## Our design and constraints\n\n{bullets(e['constraints'])}\n\n"
                f"## Tasks\n\nThe sub-issues of this issue. Every task must meet its security criterion in the gate before it is closed.\n\n"
                f"## Sources\n\n{bullets(e['sources'])}\n")
        parent = create_issue(f"epic:{e['key']}", f"[{e['key']}] {e['title']}", body,
                              ["epic", *e["labels"]], e["milestone"], ms)
        count += 1
        for i, task in enumerate(e["tasks"], 1):
            labels = sorted(set([*task["labels"], *[l for l in e["labels"] if l.startswith(("area/", "desktop-browser", "web-native", "glyphs"))]]
                                + (["security"] if task["sec"] else [])))
            tbody = (f"Part of #{parent['number']} ([{e['key']}] {e['title']}).\n\n"
                     f"## What\n\n{task['desc']}\n\n"
                     f"## Done when\n\n{task['done']}\n\n"
                     f"## Security\n\n{task['sec'] or 'No criterion beyond the parent issue security section; review it before closing.'}\n\n"
                     f"## Context\n\nThe parent issue records how {plan_data_1.BROWSERS} approach this.\n\n{bullets(e['constraints'])}\n")
            child = create_issue(f"task:{e['key']}:{i}", f"[{e['key']}-{i}] {task['title']}", tbody, labels,
                                 task["milestone"] or e["milestone"], ms)
            link(parent, child, f"{e['key']}:{i}")
            count += 1
    for i, (title, desc, labels) in enumerate(plan_data_3.DECISIONS, 1):
        body = f"{desc}\n\nFrom docs/open-questions.md.\n"
        create_issue(f"other:{i}", title, body, labels, None, ms)
        count += 1
    print(f"{'would create' if DRY else 'created or found'} {count} issues")


main()
