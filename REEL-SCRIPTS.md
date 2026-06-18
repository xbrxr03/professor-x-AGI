# 🎬 Professor X — Reel Scripts (7-Day Launch)

**Format:** 60-90 seconds each. General audience — no jargon, explain everything in plain language.
**Goal:** Record all 7 in one day. Post 1 per day.
**Tone:** Confident, direct, a bit rebellious. You're the underdog with the real insight.

---

## Day 1: "Everyone's Evolving AI Wrong"

*[OPEN: You sitting at your desk, RTX 3060 visible]*

**HOOK (0-5s):**
Every AI company trying to make agents smarter is doing the same thing — they make the brain bigger. More parameters. More training. More compute.

**CORE (5-55s):**
But there's three ways to make an AI agent better. Most people only know about one.

Way one: change the brain itself. Bigger model, more training data. That's what everyone does. That's the expensive way.

Way two: give it better notes in the moment. Like an open-book test — if you put the right reference material in front of the model, it performs way better. Even without changing the brain.

Way three — and nobody talks about this — change the system *around* the brain. The tools, the memory, how it plans, how it decides what to do next. This is called the harness.

Here's what blew my mind: a study ran 21,000 tests and found that swapping the *harness* on Claude gave a bigger performance jump than upgrading to a whole new model. The harness matters more than the model. Nobody's optimizing it.

**CTA (55-75s):**
I'm building an AI called Professor X. It runs on a $400 GPU. And it doesn't upgrade its brain — it upgrades its harness. I'm gonna show you what happens.

Follow along. This is gonna get weird.

---

## Day 2: "A Gaming PC vs a Research Lab"

*[OPEN: Split visual — SJTU research lab / your setup]*

**HOOK (0-5s):**
Shanghai Jiao Tong University built a self-evolving AI agent. They used H800 clusters — the most expensive GPUs money can buy. I'm building the same thing on a single RTX 3060.

**CORE (5-55s):**
Their system is called ASI-Evolve. It's impressive. They gave the agent the ability to improve itself over time. Run a task, see what failed, try to fix it. Repeat.

But here's the thing — they're spending thousands of dollars on compute to test whether an AI can get better at *itself*. They evolve the model weights. The expensive part.

My thesis is: if the harness is the dominant lever — and evidence says it is — then you don't need the expensive part. You can evolve the harness on cheap hardware and get comparable gains.

The harness is code. It's config files. It's prompt templates and tool descriptions. None of that costs compute. It costs *design*. And design can be automated.

So I'm running the experiment. Same question, one GPU. Can a self-evolving harness make a small model act like a big one?

**CTA (55-75s):**
Day 2. The build is live on GitHub. If it works on a 3060, it works everywhere.

---

## Day 3: "The Paper That Almost Killed My Project"

*[OPEN: You looking at a screen, slight concern]*

**HOOK (0-5s):**
Day 3 of building Professor X. I found a research paper that does exactly what I'm doing. For a moment I thought the project was dead.

**CORE (5-60s):**
It's called MOSS. A research team built a system that rewrites its own code — its own harness — to get better at tasks. Same core idea as Professor X.

But here's where they stop and I keep going.

MOSS treats the harness like a black box. It says "this task failed, let me try a random change and see if it works." Two out of three changes don't actually help. That's a 33% success rate. It's basically guessing.

Professor X doesn't guess. Before it tries to fix anything, it runs a diagnostic. Five layers deep. It asks: did the memory retrieval fail? Did the context builder put important info in the wrong spot? Did it pick the wrong tool? Did the tool run but return garbage? Or did the model just not think it through?

Each failure gets a label. Then the fix targets that specific layer. No more random changes. Targeted fixes.

**CTA (60-75s):**
Competition validates the space. Differentiation wins. Professor X doesn't throw spaghetti at the wall.

---

## Day 4: "Your AI Agent Doesn't Know Who It Is"

*[OPEN: You, close-up, serious tone]*

**HOOK (0-5s):**
Here's a problem nobody's talking about. Every self-improving AI system gets better — but it also changes. After 30 rounds of self-modification, is your agent still the same agent?

**CORE (5-60s):**
Think about it. If an AI rewrites its own code 30 times — changes how it remembers things, how it uses tools, how it plans — at what point is it even the same system anymore?

This isn't philosophy. It's an engineering problem. If your AI's behavior drifts too far, you can't trust it. You can't reproduce results. You can't compare round 30 to round 1.

Professor X has a solution. After every modification, it takes a behavioral fingerprint — a score across different types of tasks. Think of it like a personality test, but for code. "Good at tool use, bad at planning. Good at reasoning, bad at memory retrieval."

This fingerprint is tracked over time. So you can watch the agent evolve and still verify: yes, it's getting better, and yes, it's still fundamentally the same system, just improved.

It's called identity-preserving evolution. The agent gets better without becoming someone else.

**CTA (60-80s):**
What if your AI could improve without losing its identity? That's what we're testing.

---

## Day 5: "The Diagnostic That Fixes Itself"

*[OPEN: You, animated — this is the cool one]*

**HOOK (0-5s):**
When your AI agent fails a task, what do you do? Most systems just try again. Maybe with a different prompt. Maybe they just retry. That's like taking your car to a mechanic who just replaces random parts until it works.

**CORE (5-65s):**
Professor X runs a five-layer diagnostic every time something fails.

Layer 1: Did it even remember the right information? If the memory system didn't retrieve what was needed, no amount of reasoning will fix it.

Layer 2: Was the right information in the right place? There's research showing that if you put important stuff in the middle of the prompt, the model straight-up ignores it. Position matters.

Layer 3: Did it pick the right tool? A hammer isn't useful if you needed a screwdriver.

Layer 4: Did the tool work but return useless output? The tool ran fine, the result was garbage.

Layer 5: Did everything work — the memory, the tools, the info — but the model just didn't put it together right? That's a reasoning failure.

Only then does it propose a fix. And the fix targets the specific layer that failed. No guessing. Current systems guess right 33% of the time. We're targeting 60%.

**CTA (65-80s):**
Most agents retry. Professor X diagnoses. Day 5 — see you tomorrow.

---

## Day 6: "The Benchmark That Doesn't Exist"

*[OPEN: You, slightly frustrated]*

**HOOK (0-5s):**
I went looking for a benchmark to test whether Professor X's harness actually improves over time. There isn't one.

**CORE (5-60s):**
Every AI benchmark measures one thing: can the model do the task? Accuracy scores. Pass rates. None of them measure whether the *system around the model* is getting better.

If I run 30 rounds of self-evolution and the agent goes from 40% to 60% — did the harness improve, or did I just get lucky with task selection? Nobody can tell, because nobody built the test for it.

So we built one. It's called HIRO — Harness Improvement Rate Over iterations.

The idea is simple: freeze the model. Don't change the brain at all. Only change the harness. Run the same 60 tasks every round. Measure how much the harness alone improved performance. That's your HIRO score.

Sixty tasks. Three categories — tool use, planning, and self-correction. Four baselines including a static harness and a human-expert harness. Every round, you get a clean number: did the harness actually get better, or not?

**CTA (60-75s):**
If you can't measure it, you can't claim it. We're measuring it.

---

## Day 7: "The Experiment"

*[OPEN: You, confident, looking at camera directly]*

**HOOK (0-5s):**
Week 1. Here's the full picture. One student, one GPU, one question.

**CORE (5-65s):**
Can a self-evolving AI harness make a small model act like a big one?

Here's what we're running. Professor X starts with a basic harness around a small model running locally. Every round, it runs tasks, diagnoses failures at five layers, proposes targeted fixes, and evolves.

Four baselines to compare against. A static harness that never changes — that's the floor. A human-expert harness designed by hand. The same harness on a frontier model. And a random-evolution harness that just guesses.

Thirty rounds. The same 60 tasks each time. After every round, we measure: did the harness actually get better? And did the agent stay the same agent — not drift into something unrecognizable?

The three inventions make this possible. The diagnostic tells you *why* something failed. The fingerprint tracks *what* you're good at over time. And adaptive context allocation fixes the biggest silent killer — shoving too much information at a small model and hoping it sorts it out.

If the harness is the dominant lever — and all the evidence says it is — then a $400 GPU should be able to produce measurable, reproducible improvement. That's the bet.

**CTA (65-90s):**
Thirty rounds. One GPU. We'll see if the harness is the thing everyone's been ignoring. Follow the build. It's all open source.

---

## Production Notes

- **Total runtime:** ~75-90 seconds each (Day 7 may run slightly longer at ~90s)
- **Pacing:** Fast hook → slow down for core → steady CTA. No rushing.
- **Visuals:** Screen recordings of Professor X running, code diffs, architecture diagrams where relevant. Keep it simple — you + camera is enough for most of it.
- **Captions:** Always on. General audience won't catch every word.
- **Hashtags:** #ProfessorX #AI #SelfEvolving #OpenSource #BuildInPublic