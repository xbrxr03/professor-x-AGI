#!/usr/bin/env python3
"""Polished live TUI for the distillation flywheel — coding-agent styling.
Run:  bash distill/dash.sh   (or distill/.venv/bin/python distill/dash.py)

Features cherry-picked from good TUIs (btop/k9s/lazygit/coding agents):
  · rounded themed panels (Tokyo Night palette)      · live GPU gauges + history sparklines
  · animated spinner on the active phase             · a 6-step pipeline stepper with ✓/⟳/○
  · loss sparkline + delta                           · run elapsed timer + dimmed log tail footer
"""
import os, re, subprocess, time
from collections import deque
from datetime import datetime
from rich.live import Live
from rich.panel import Panel
from rich.table import Table
from rich.console import Group
from rich.columns import Columns
from rich.text import Text
from rich.align import Align
from rich import box

LOG = os.environ.get("PX_LOG", "/tmp/distill_flywheel.log")
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]")
BARS = "▁▂▃▄▅▆▇█"
SPIN = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
# Tokyo Night
BG, FG, DIM = "#1a1b26", "#c0caf5", "#565f89"
BLUE, GREEN, YEL, RED, MAG, CYAN = "#7aa2f7", "#9ece6a", "#e0af68", "#f7768e", "#bb9af7", "#7dcfff"

util_hist = deque(maxlen=48)
_t0 = time.time()


def sh(cmd):
    try:
        return subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=4).stdout.strip()
    except Exception:
        return ""


def heat(p):  # 0..100
    return GREEN if p < 50 else (YEL if p < 85 else RED)


def gauge(frac, width=22, color=GREEN):
    frac = max(0.0, min(1.0, frac)); fill = int(round(frac * width))
    t = Text(); t.append("━" * fill, style=color); t.append("━" * (width - fill), style=DIM)
    return t


def spark(vals, color=GREEN, width=40):
    vals = list(vals)[-width:]
    if not vals:
        return Text("", style=DIM)
    lo, hi = min(vals), max(vals)
    t = Text()
    for v in vals:
        i = 0 if hi == lo else int((v - lo) / (hi - lo) * (len(BARS) - 1))
        t.append(BARS[i], style=color)
    return t


def read_log():
    if not os.path.exists(LOG):
        return ""
    return ANSI.sub("", open(LOG, errors="ignore").read())


def parse(txt):
    st = {"losses": [], "epoch": None, "phase_idx": 0, "phase": "waiting for a run…",
          "sub": "", "started": None, "done_reason": None, "verdict": None}
    for m in re.finditer(r"'loss':\s*([0-9.]+).*?'epoch':\s*([0-9.]+)", txt):
        st["losses"].append(float(m.group(1))); st["epoch"] = float(m.group(2))
    # pipeline progress
    flags = {
        1: "[1/6]" in txt, 2: ("[2/6]" in txt or "curated trajectories" in txt),
        3: "[4/6]" in txt, 4: ("Converting to GGUF" in txt or "quantize time" in txt or "Merging" in txt),
        5: ("[5/6]" in txt or "[5b]" in txt), 6: ("[6/6]" in txt and "ICS-GATE" in txt),
    }
    st["phase_idx"] = max([k for k, v in flags.items() if v] or [0])
    names = {0: "waiting", 1: "install deps", 2: "data / curate", 3: "QLoRA fine-tune",
             4: "merge → GGUF → quantize", 5: "serve + stop-sanity", 6: "ICS gate"}
    st["phase"] = names[st["phase_idx"]]
    for key, lab in [("Merging model", "merging to 16-bit"), ("Converting to GGUF", "converting → GGUF"),
                     ("quantize time", "quantized → q4_K_M"), ("GPU free before train", "GPU cleared, loading model"),
                     ("stop-sanity", "checking the model halts")]:
        if key in txt:
            st["sub"] = lab
    dr = re.findall(r"done_reason=(\w+)", txt)
    if dr:
        st["done_reason"] = dr[-1]; st["sub"] = f"stop-sanity → done_reason={dr[-1]}"
    if "ACCEPT:" in txt: st["verdict"] = ("ACCEPT", GREEN)
    elif "REJECT:" in txt: st["verdict"] = ("REJECT", RED)
    m = re.search(r"flywheel \(turnkey\)\s+(.+)", txt)
    if m:
        for fmt in ("%a %b %d %I:%M:%S %p %Z %Y", "%a %b %d %H:%M:%S %Z %Y"):
            try:
                st["started"] = datetime.strptime(m.group(1).strip(), fmt); break
            except Exception:
                pass
    return st


def gpu_panel(frame):
    q = sh("nvidia-smi --query-gpu=name,utilization.gpu,memory.used,memory.total,"
           "temperature.gpu,power.draw,power.limit --format=csv,noheader,nounits")
    if not q:
        return Panel(Text("nvidia-smi unavailable", style=RED), title="GPU", box=box.ROUNDED, border_style=RED)
    name, util, mu, mt, temp, pw, pl = [x.strip() for x in q.split(",")]
    util, mu, mt, temp, pw = float(util), float(mu), float(mt), float(temp), float(pw)
    pl = float(pl) if pl and pl != "[N/A]" else max(pw, 1)
    util_hist.append(util)
    g = Table.grid(padding=(0, 1)); g.add_column(justify="right", style=DIM, no_wrap=True); g.add_column()
    g.add_row("card", Text(name, style=f"bold {BLUE}"))
    g.add_row("util", Text.assemble(gauge(util / 100, color=heat(util)), Text(f" {util:4.0f}%", style=heat(util))))
    g.add_row("", Text.assemble(Text("hist ", style=DIM), spark(util_hist, color=CYAN)))
    g.add_row("vram", Text.assemble(gauge(mu / mt, color=heat(mu / mt * 100)),
              Text(f" {mu/1024:4.1f}/{mt/1024:.0f} GB", style=FG)))
    g.add_row("power", Text.assemble(gauge(pw / pl, color=MAG), Text(f" {pw:3.0f}/{pl:.0f} W", style=FG)))
    g.add_row("temp", Text(f"{temp:.0f}°C", style=heat(temp)))
    ps = sh("ollama ps"); rows = [l for l in ps.splitlines()[1:] if l.strip()]
    g.add_row("model", Text(rows[0].split()[0] if rows else "(none loaded)",
              style=MAG if rows else DIM))
    return Panel(g, title=f"[{CYAN}]󰢮 GPU[/]", title_align="left", box=box.ROUNDED, border_style=BLUE, padding=(0, 1))


def stepper(st, frame):
    steps = ["deps", "data", "train", "convert", "serve", "gate"]
    cur = st["phase_idx"]
    line = Text()
    for i, name in enumerate(steps, 1):
        if st["verdict"] and i == 6:
            ic, sty = ("✓" if st["verdict"][0] == "ACCEPT" else "✗"), st["verdict"][1]
        elif i < cur:
            ic, sty = "✓", GREEN
        elif i == cur:
            ic, sty = SPIN[frame % len(SPIN)], YEL
        else:
            ic, sty = "○", DIM
        line.append(f" {ic} ", style=sty)
        line.append(name + ("  ›" if i < len(steps) else ""), style=(FG if i == cur else DIM))
    return line


def train_panel(st, frame):
    g = Table.grid(padding=(0, 1)); g.add_column(justify="right", style=DIM, no_wrap=True); g.add_column()
    spin = SPIN[frame % len(SPIN)] if cur_active(st) else "●"
    g.add_row("phase", Text.assemble(Text(f"{spin} ", style=YEL), Text(st["phase"], style=f"bold {FG}")))
    if st["sub"]:
        g.add_row("", Text(st["sub"], style=f"italic {CYAN}"))
    if st["epoch"] is not None:
        g.add_row("epoch", Text.assemble(gauge(min(st["epoch"] / 2.0, 1.0), color=BLUE),
                  Text(f" {st['epoch']:.2f}/2", style=FG)))
    if st["losses"]:
        first, last = st["losses"][0], st["losses"][-1]
        d = last - first
        g.add_row("loss", Text.assemble(Text(f"{last:.3f} ", style=f"bold {GREEN}"),
                  Text(f"({d:+.2f})", style=GREEN if d < 0 else RED)))
        g.add_row("", spark(st["losses"], color=GREEN))
    if st["verdict"]:
        v, c = st["verdict"]
        g.add_row("gate", Text(f"{'✅' if v=='ACCEPT' else '⛔'} {v}", style=f"bold {c}"))
    return Panel(g, title=f"[{CYAN}]󱙺 Training[/]", title_align="left", box=box.ROUNDED, border_style=BLUE, padding=(0, 1))


def cur_active(st):
    # consider it "running" if the log changed in the last 30s
    try:
        return (time.time() - os.path.getmtime(LOG)) < 30
    except Exception:
        return False


def log_panel():
    body = "(no flywheel log yet — start a run)"
    if os.path.exists(LOG):
        lines = [l for l in read_log().splitlines() if l.strip()][-6:]
        body = "\n".join(l[:100] for l in lines)
    return Panel(Text(body, style=DIM), title=f"[{CYAN}] log[/]", title_align="left",
                 box=box.ROUNDED, border_style=DIM, padding=(0, 1))


def render():
    frame = int((time.time() - _t0) * 8)
    txt = read_log(); st = parse(txt)
    elapsed = ""
    if st["started"]:
        secs = int((datetime.now() - st["started"]).total_seconds())
        if secs >= 0:
            elapsed = f"  ·  elapsed {secs//3600:d}:{(secs%3600)//60:02d}:{secs%60:02d}"
    running = cur_active(st)
    dot = Text("● live" if running else "○ idle", style=GREEN if running else DIM)
    header = Table.grid(expand=True); header.add_column(justify="left"); header.add_column(justify="right")
    header.add_row(Text.assemble(Text("  PROFESSOR X ", style=f"bold {BLUE}"),
                   Text("· distillation flywheel", style=DIM), Text(elapsed, style=DIM)),
                   Text.assemble(dot, Text(f"   {time.strftime('%H:%M:%S')}  ", style=DIM)))
    head = Panel(header, box=box.ROUNDED, border_style=MAG, padding=(0, 0))
    steps = Panel(Align.center(stepper(st, frame)), box=box.ROUNDED, border_style=DIM, padding=(0, 1))
    body = Columns([gpu_panel(frame), train_panel(st, frame)], expand=True, equal=True)
    foot = Align.center(Text("q / Ctrl-C quit   ·   refresh 0.5s   ·   reads /tmp/distill_flywheel.log", style=DIM))
    return Group(head, steps, body, log_panel(), foot)


if __name__ == "__main__":
    try:
        with Live(render(), refresh_per_second=8, screen=True) as live:
            while True:
                time.sleep(0.5)
                live.update(render())
    except KeyboardInterrupt:
        pass
