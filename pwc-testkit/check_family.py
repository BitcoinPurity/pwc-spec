"""Editorial consistency check across the PWC document family.

Usage: python3 check_family.py <directory containing the PWC-*.md files, normally specs/>
Checks section references between documents, method, notification and event names, error codes, test identifiers
cited by PWC-CONFORMANCE-001, table shapes, and requirement keywords written in lower case in normative documents.
"""
import json
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
HERE = pathlib.Path(__file__).resolve().parent
docs = {p.name: p.read_text(encoding="utf-8") for p in sorted(root.glob("PWC-*.md"))}
ALIAS = {
    "Core": "PWC-Core-v0.2.1-EN.md", "PWC-CORE-001": "PWC-Core-v0.2.1-EN.md",
    "LOCAL": "PWC-Transport-Local-v0.1.0-EN.md", "PWC-TRANSPORT-LOCAL-001": "PWC-Transport-Local-v0.1.0-EN.md",
    "BROWSER": "PWC-Transport-Browser-v0.1.0-EN.md", "PWC-TRANSPORT-BROWSER-001": "PWC-Transport-Browser-v0.1.0-EN.md",
    "REMOTE": "PWC-Transport-Remote-v0.1.0-EN.md", "PWC-TRANSPORT-REMOTE-001": "PWC-Transport-Remote-v0.1.0-EN.md",
    "MOBILE": "PWC-Transport-Mobile-v0.1.0-EN.md", "PWC-TRANSPORT-MOBILE-001": "PWC-Transport-Mobile-v0.1.0-EN.md",
    "PWC-CONFORMANCE-001": "PWC-Conformance-v0.1.0-EN.md", "PWC-REGISTRY-001": "PWC-Registry-v0.1.0-EN.md",
    "PWC-GOVERNANCE-001": "PWC-Governance-v0.1.0-EN.md", "PWC-SECURITY-001": "PWC-Security-v0.1.0-EN.md",
    "PWC-UX-001": "PWC-UX-Guidelines-v0.1.0-EN.md", "PWC-CLIENT-GUIDE-001": "PWC-Client-Guide-v0.1.0-EN.md",
    "PWC-OVERVIEW-001": "PWC-Overview-v0.1.0-EN.md",
}
INFORMATIVE = {"PWC-Security-v0.1.0-EN.md", "PWC-UX-Guidelines-v0.1.0-EN.md", "PWC-Client-Guide-v0.1.0-EN.md",
               "PWC-Overview-v0.1.0-EN.md"}
problems = []
missing_docs = set(ALIAS.values()) - set(docs)
if missing_docs:
    problems.append(f"missing documents: {sorted(missing_docs)}")


def sections(text):
    return set(re.findall(r"^#+ (\d+(?:\.\d+)*)[\. ]", text, re.M))


def strip_code(text):
    return re.sub(r"```.*?```", "", text, flags=re.S)


heads = {name: sections(text) for name, text in docs.items()}
core_items = len(re.findall(r"^\d+\. \*\*", docs["PWC-Core-v0.2.1-EN.md"].split("## 12.")[1].split("## 13.")[0], re.M))

ref_re = re.compile(r"(?:(" + "|".join(sorted(map(re.escape, ALIAS), key=len, reverse=True)) +
                    r")(?: v[0-9.]+)? )?§(\d+(?:\.\d+)*)((?:, item (\d+))?)")
for name, text in docs.items():
    body = strip_code(text)
    for m in ref_re.finditer(body):
        target = ALIAS.get(m.group(1), name) if m.group(1) else name
        sec = m.group(2)
        # "§x" right after a cross-document reference in a list ("LOCAL §8; REMOTE §5.3", "§4.2 and §4.3")
        before = body[max(0, m.start() - 40):m.start()]
        before = body[max(0, m.start() - 120):m.start()]
        prev = re.search(r"(" + "|".join(map(re.escape, ALIAS)) + r")(?: v[0-9.]+)? §[\d.]+"
                         r"(?:(?:,| and| or| to) §[\d.]+)*(?:,| and| or| to) $", before)
        if not m.group(1) and prev:
            target = ALIAS[prev.group(1)]
        if target not in heads:
            continue
        if sec not in heads[target]:
            problems.append(f"{name}: §{sec} not found in {target}")
        if m.group(4) and target == "PWC-Core-v0.2.1-EN.md" and not 1 <= int(m.group(4)) <= core_items:
            problems.append(f"{name}: Core §12 item {m.group(4)} does not exist")

registry = json.loads((HERE.parent / "pwc-schema" / "pwc-registry-0.1.0.json").read_text(encoding="utf-8"))
known = ({m["name"] for m in registry["methods"]} | {n["name"] for n in registry["notifications"]}
         | {e["event_type"] for e in registry["events"]})
codes = {e["code"] for e in registry["errors"]}
CATEGORIES = r"(?:pwc|session|wallet|account|address|utxo|transaction|psbt|message|operation|local|remote|grant)"
for name, text in docs.items():
    for m in re.findall(rf"`({CATEGORIES}\.[a-zA-Z]+)`", text):
        if m not in known and m not in ("psbt.json", "transaction.send"):
            problems.append(f"{name}: unknown method/notification/event `{m}`")
    for m in re.findall(r"`([A-Z][A-Z_]{5,})`", text):
        if m.endswith(("_SATS",)) or m.startswith(("PSBT_GLOBAL", "PSBT_IN_", "SIGHASH", "FILE_FLAG", "PIPE_",
                                                     "SO_", "SOCK_", "AF_", "XDG_", "MAX_")):
            continue
        if m not in codes and name != "PWC-Core-v0.2.1-EN.md":
            problems.append(f"{name}: unknown error code `{m}`")

tests = {}
for name, text in docs.items():
    for tid in re.findall(r"^\| ([CLBRM]-\d{2}) \|", text, re.M):
        tests.setdefault(tid, name)
conf = docs["PWC-Conformance-v0.1.0-EN.md"]
for a, b in re.findall(r"\b([CLBRM]-\d{2}) to \1?([CLBRM]-\d{2})", conf):
    pass
for tid in set(re.findall(r"\b([CLBRM]-\d{2})\b", conf)):
    if tid not in tests:
        problems.append(f"Conformance cites {tid}, which no document defines")
for tid in set(re.findall(r"\b([CLBRM]-\d{2})\b", docs["PWC-Security-v0.1.0-EN.md"])):
    if tid not in tests:
        problems.append(f"Security cites {tid}, which no document defines")

for name, text in docs.items():
    fence, width = False, None
    for i, line in enumerate(text.split("\n"), 1):
        if line.startswith("```"):
            fence = not fence
            continue
        if fence:
            continue
        if line.startswith("|"):
            n = len(re.findall(r"(?<!\\)\|", line))
            width = width or n
            if n != width:
                problems.append(f"{name}:{i}: table row has {n - 1} columns, expected {width - 1}")
        else:
            width = None
    if name not in INFORMATIVE:
        for i, line in enumerate(strip_code(text).split("\n"), 1):
            for w in re.findall(r"\b(must|shall|should not|must not)\b", line):
                if not re.search(r"(User|the User|Users|a person)\s+(must|should)", line):
                    problems.append(f"{name}: lower-case '{w}': {line.strip()[:90]}")

if problems:
    print("\n".join(problems))
    sys.exit(1)
print(f"family consistent: {len(docs)} documents, {sum(len(h) for h in heads.values())} sections, "
      f"{len(tests)} test identifiers")
