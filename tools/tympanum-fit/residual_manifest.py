"""Put a hybrid pack's entry (residual_zip.py's `<id>.manifest.json`) into the content
manifest: replaced where the pack is, appended when it is new. The rest of the file is left
byte for byte (its line endings too).

    python -I residual_manifest.py <crates/cantus-content/manifest/content.json> <entry.json>
"""
import json
import sys


def main():
    path, entry_path = sys.argv[1:3]
    raw = open(path, "rb").read().decode("utf-8")
    nl = "\r\n" if "\r\n" in raw else "\n"
    entry = json.load(open(entry_path, encoding="utf-8"))
    text = nl.join("  " + l for l in json.dumps(entry, ensure_ascii=False, indent=1).split("\n"))
    key = '"id": "%s"' % entry["id"]
    at = raw.find(key)
    if at >= 0:
        # The entry's own braces: from the `{` before its id to its matching `}`.
        start = raw.rfind("{", 0, at)
        start = raw.rfind(nl, 0, start) + len(nl)
        depth, i = 0, raw.index("{", start)
        in_str = False
        while True:
            c = raw[i]
            if in_str:
                if c == "\\":
                    i += 1
                elif c == '"':
                    in_str = False
            elif c == '"':
                in_str = True
            elif c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        raw = raw[:start] + text + raw[i + 1:]
    else:
        tail = nl + " ]" + nl + "}" + nl
        assert raw.endswith(tail), "the manifest's end is not where expected"
        raw = raw[:-len(tail)] + "," + nl + text + tail
    json.loads(raw)
    open(path, "wb").write(raw.encode("utf-8"))
    print("%s: %s" % (entry["id"], "replaced" if at >= 0 else "added"))


if __name__ == "__main__":
    main()
