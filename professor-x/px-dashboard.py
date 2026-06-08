#!/usr/bin/env python3
"""Professor X — live harness dashboard.

A jcode-inspired terminal UI showing the harness ALIVE: real-time activity, the
consciousness vitals that are unique to Professor X (phi, LZc, meta-d', ICS,
affect, interoception), and harness stats — read live from the event stream and
state.db. No daemon; just run it alongside the harness.

    ./px-dashboard.py                 # ~/.professor-x default
    PROFESSOR_X_DATA_DIR=... ./px-dashboard.py

Requires: rich (already present).
"""
import json
import math
import os
import sqlite3
import time
from collections import Counter, deque
from datetime import datetime

from rich.console import Console, Group
from rich.layout import Layout
from rich.live import Live
from rich.panel import Panel
from rich.table import Table
from rich.text import Text

DATA = os.path.expanduser(os.environ.get("PROFESSOR_X_DATA_DIR", "~/.professor-x"))
DB = os.path.join(DATA, "state.db")
# events live in the repo's artifacts dir (cwd-relative, both layouts)
EVENT_DIRS = ["artifacts/events", "professor-x/artifacts/events"]


def db():
    c = sqlite3.connect(DB, timeout=5)
    c.row_factory = sqlite3.Row
    return c


def q1(c, sql, args=(), default=None):
    try:
        r = c.execute(sql, args).fetchone()
        return r[0] if r and r[0] is not None else default
    except Exception:
        return default


def lz76(seq):
    n = len(seq)
    if n == 0:
        return 0
    comp = 1; pl = 1; sl = 1; i = 0
    while pl + sl <= n:
        if seq[i + sl - 1] == seq[pl + sl - 1]:
            sl += 1
            if pl + sl > n:
                comp += 1; break
        else:
            i += 1
            if i == pl:
                comp += 1; pl += sl; i = 0; sl = 1
            else:
                sl = 1
    return comp


def bar(v, lo, hi, width=18, color="cyan"):
    frac = 0.0 if hi == lo else max(0.0, min(1.0, (v - lo) / (hi - lo)))
    fill = int(frac * width)
    return Text("█" * fill + "·" * (width - fill), style=color)


def recent_events(n=80):
    for d in EVENT_DIRS:
        f = os.path.join(d, datetime.now().strftime("%Y-%m-%d") + ".jsonl")
        if os.path.exists(f):
            try:
                with open(f) as fh:
                    lines = deque(fh, maxlen=n)
                out = []
                for ln in lines:
                    try:
                        out.append(json.loads(ln))
                    except Exception:
                        pass
                return out
            except Exception:
                return []
    return []


EVENT_STYLE = {
    "tool.succeeded": "green", "tool.started": "cyan", "tool.requested": "dim cyan",
    "task.succeeded": "bold green", "task.failed": "bold red", "task.fail_requested": "red",
    "react.duplicate_action": "yellow", "policy.denied": "red", "agent.delegate": "magenta",
    "task.queued": "blue", "scratchpad.updated": "dim", "task.finish_requested": "green",
}


def vitals_panel(c):
    t = Table.show_header = None
    g = Table(box=None, expand=True, pad_edge=False)
    g.add_column(justify="left", no_wrap=True)
    g.add_column(justify="left")
    g.add_column(justify="right", no_wrap=True)

    # phi (latest round) + live LZc from recent activations
    phi = q1(c, "SELECT phi FROM phi_rounds ORDER BY round DESC LIMIT 1", default=0.0)
    idxs = []
    try:
        idxs = [r[0] for r in c.execute(
            "SELECT activation_index FROM phi_activations ORDER BY id DESC LIMIT 40")]
    except Exception:
        pass
    lzc = 0.0
    if len(idxs) >= 4:
        flat = []
        for b in range(7):
            flat += [((x >> b) & 1) == 1 for x in idxs]
        nn = len(flat)
        lzc = lz76(flat) * math.log2(nn) / nn if nn > 1 else 0.0
    mean_active = sum(bin(x).count("1") for x in idxs) / len(idxs) if idxs else 0.0

    g.add_row("φ integration", bar(phi, 0, 3, color="bright_magenta"), f"{phi:.2f}")
    g.add_row("LZc differ.", bar(lzc, 0, 2.2, color="magenta"), f"{lzc:.2f}")
    g.add_row("modules/dec", bar(mean_active, 0, 7, color="blue"), f"{mean_active:.1f}/7")

    # ICS identity
    ics = q1(c, "SELECT score FROM ics_scores ORDER BY id DESC LIMIT 1", default=0.0)
    icol = "green" if (ics or 0) >= 0.70 else "red"
    g.add_row("ICS identity", bar(ics or 0, 0, 1, color=icol), f"{ics:.2f}")

    # meta-d' AUROC from recent self_predictions
    auroc = None
    try:
        rows = c.execute("SELECT expected_success,success_err FROM self_predictions "
                         "ORDER BY id DESC LIMIT 200").fetchall()
        data = []
        for cf, se in rows:
            cf = float(cf); se = float(se)
            data.append((cf, abs(cf + se - 1) < abs(cf - se)))
        nc = sum(1 for _, x in data if x); ni = len(data) - nc
        if nc and ni:
            order = sorted(range(len(data)), key=lambda i: data[i][0])
            ranks = [0.0] * len(data); i = 0
            while i < len(order):
                j = i
                while j + 1 < len(order) and abs(data[order[j+1]][0]-data[order[i]][0]) < 1e-9:
                    j += 1
                for k in range(i, j + 1):
                    ranks[order[k]] = ((i + 1) + (j + 1)) / 2
                i = j + 1
            Rc = sum(ranks[k] for k, (_, x) in enumerate(data) if x)
            auroc = (Rc - nc * (nc + 1) / 2) / (nc * ni)
    except Exception:
        pass
    if auroc is not None:
        acol = "green" if auroc >= 0.6 else ("yellow" if auroc >= 0.53 else "red")
        g.add_row("meta-d′ AUROC", bar(auroc, 0.3, 0.8, color=acol), f"{auroc:.2f}")

    # affect (valence/arousal), body stress
    try:
        a = c.execute("SELECT valence,arousal FROM affect_states ORDER BY id DESC LIMIT 1").fetchone()
        if a:
            val, ar = float(a[0]), float(a[1])
            vcol = "green" if val >= 0 else "red"
            g.add_row("affect valence", bar(val, -1, 1, color=vcol), f"{val:+.2f}")
            g.add_row("affect arousal", bar(ar, 0, 1, color="yellow"), f"{ar:.2f}")
    except Exception:
        pass
    try:
        v = c.execute("SELECT inference_latency_ms,token_budget_used,memory_pressure,"
                      "evolution_health FROM computational_vitals ORDER BY id DESC LIMIT 1").fetchone()
        if v:
            lat = min(float(v[0]) / 10000.0, 1.0)
            stress = 0.35*lat + 0.25*float(v[1]) + 0.20*float(v[2]) + 0.20*(1-float(v[3]))
            scol = "red" if stress > 0.5 else ("yellow" if stress > 0.3 else "green")
            g.add_row("body stress", bar(stress, 0, 1, color=scol), f"{stress:.2f}")
    except Exception:
        pass

    return Panel(g, title="[bold]🧠 consciousness vitals", border_style="magenta")


def stats_panel(c):
    g = Table(box=None, expand=True, pad_edge=False)
    g.add_column(justify="left", no_wrap=True)
    g.add_column(justify="right")

    # corpus (unique verified tasks across trajectory files)
    uniq = 0
    try:
        import glob
        tasks = set()
        for p in glob.glob("**/trajectories/*/trajectories.jsonl", recursive=True):
            for ln in open(p):
                try:
                    d = json.loads(ln)
                    if d.get("verified"):
                        tasks.add(d.get("task", ""))
                except Exception:
                    pass
        uniq = len(tasks)
    except Exception:
        pass
    g.add_row("corpus (unique)", f"{uniq}")

    # pass@3 latest round
    try:
        r = c.execute("SELECT round, COUNT(*), SUM(passed) FROM hiro_attempts "
                      "GROUP BY round ORDER BY round DESC LIMIT 1").fetchone()
        if r:
            g.add_row(f"pass@3 (r{r[0]})", f"{(r[2] or 0)/r[1]:.2f}")
    except Exception:
        pass

    g.add_row("cognition base", f"{q1(c,'SELECT COUNT(*) FROM cognition',default=0)}")
    g.add_row("episodic mem", f"{q1(c,'SELECT COUNT(*) FROM episodic',default=0)}")
    g.add_row("narrative", f"{q1(c,'SELECT COUNT(*) FROM narrative_episodes',default=0)}")
    g.add_row("self-model rev", f"{q1(c,'SELECT COUNT(*) FROM self_model',default=0)}")
    g.add_row("phi rounds", f"{q1(c,'SELECT COUNT(*) FROM phi_rounds',default=0)}")
    return Panel(g, title="[bold]⚙ harness stats", border_style="blue")


def stream_panel(events):
    t = Table(box=None, expand=True, pad_edge=False, show_header=False)
    t.add_column("t", style="dim", no_wrap=True, width=8)
    t.add_column("event", no_wrap=True, width=22)
    t.add_column("detail", overflow="ellipsis")
    for e in events[-22:]:
        et = e.get("event_type", "")
        ts = e.get("timestamp", "")[11:19]
        style = EVENT_STYLE.get(et, "white")
        summ = (e.get("summary") or "")[:90]
        t.add_row(ts, Text(et, style=style), summ)
    return Panel(t, title="[bold]📡 live activity", border_style="cyan")


def header(events):
    last = events[-1] if events else {}
    act = last.get("summary") or last.get("event_type") or "idle"
    now = datetime.now().strftime("%H:%M:%S")
    txt = Text.assemble(
        ("PROFESSOR X ", "bold bright_magenta"),
        ("— self-evolving research agent   ", "dim"),
        ("◉ ", "green"),
        (act[:80], "white"),
    )
    return Panel(txt, subtitle=f"[dim]{now}  ·  {DB}", border_style="bright_magenta")


def build():
    events = recent_events()
    c = db()
    try:
        lay = Layout()
        lay.split_column(
            Layout(header(events), size=3, name="head"),
            Layout(name="body"),
        )
        lay["body"].split_row(
            Layout(name="left", ratio=2),
            Layout(stream_panel(events), name="right", ratio=3),
        )
        lay["left"].split_column(
            Layout(vitals_panel(c), name="v"),
            Layout(stats_panel(c), name="s", size=10),
        )
        return lay
    finally:
        c.close()


def main():
    if not os.path.exists(DB):
        print(f"No state.db at {DB}")
        return
    console = Console()
    with Live(build(), console=console, screen=True, refresh_per_second=1) as live:
        try:
            while True:
                time.sleep(1.0)
                live.update(build())
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
