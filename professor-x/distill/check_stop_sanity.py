#!/usr/bin/env python3
"""Run the raw-mode stop sanity check used before gating a distilled model."""

from __future__ import annotations

import argparse
import json
import sys
from urllib import request


DEFAULT_PROMPT = (
    "You are an agent. Respond in strict ReAct format.\n"
    "<task>\n"
    "List the files in the current directory.\n"
    "</task>\n\n"
    "Thought:"
)


def parse_args() -> argparse.Namespace:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--model", required=True, help="Ollama model name to probe")
    ap.add_argument("--url", default="http://localhost:11434/api/generate", help="Ollama generate endpoint")
    ap.add_argument("--prompt", default=DEFAULT_PROMPT, help="Prompt to use for the sanity check")
    return ap.parse_args()


def main() -> int:
    args = parse_args()
    payload = {
        "model": args.model,
        "prompt": args.prompt,
        "stream": False,
        "think": False,
        "options": {"num_predict": 512, "stop": ["Observation:"]},
    }
    req = request.Request(
        args.url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    with request.urlopen(req) as resp:
        data = json.loads(resp.read().decode("utf-8"))
    done_reason = data.get("done_reason")
    response = data.get("response", "")
    has_action = "Action:" in response
    print(f"done_reason={done_reason} has_action={has_action}")
    if done_reason != "stop" or not has_action:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
