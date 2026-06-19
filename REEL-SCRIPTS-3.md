# 🎬 Professor X — Reel Scripts, Batch 3 (from the real research)

**Format:** 60-90s each. Plain language, underdog tone, build-in-public — the *real* messy research,
including the nulls and the noise. Every one of these is something that actually happened this week.

---

## "Your AI Benchmark Is Probably Lying to You"

*[OPEN: you at the desk, a score on screen]*

**HOOK (0-5s):**
I tried to prove my AI was getting smarter. I couldn't — because my test was broken. Not wrong. *Too easy.*

**CORE (5-60s):**
Here's the trap nobody warns you about. I had a coding test my AI scored 95% on. Great, right? So I improved the AI and ran it again. Still 95%. Improved it more. Still 95%.

The problem wasn't the AI. When a test is so easy the baseline already aces it, there's no room left to show improvement. A near-perfect score *can't go up.* The test was saturated — it had no headroom.

So before I could measure *any* progress, I had to build a *harder* test — one where the AI fails enough that getting better actually shows on the scoreboard. Building the honest ruler came before everything else.

**CTA (60-75s):**
If your AI always scores great, be suspicious. A test that can't show failure can't show improvement either. Build the hard ruler first.

---

## "I Almost Rebuilt a Feature I Already Had"

*[OPEN: you, half-laughing]*

**HOOK (0-5s):**
I spent a while designing the perfect fix for my AI. Then I found out I'd already built it. Months ago.

**CORE (5-55s):**
My agent kept failing coding tasks by "fixing" them, declaring victory, and being wrong. So I designed a gate: before it's allowed to say "done," run the test — if it still fails, make it keep working.

I was about to build it. But I checked the code first. It was *already there.* Fully wired. I'd written it and forgotten.

That checking step saved me a wasted day — and it taught me something bigger: half of working on a complex system is *knowing what you already have.* The codebase had more capability than I remembered. The bottleneck was somewhere else entirely.

**CTA (55-70s):**
Before you build the clever thing, check if past-you already did. Read before you write. Always.

---

## "It Looked Like a 3x Win. It Was Just Noise."

*[OPEN: you, serious]*

**HOOK (0-5s):**
My AI change tripled the score. I almost posted it as a breakthrough. Good thing I didn't.

**CORE (5-65s):**
I made a tweak, ran the test once: the score jumped from 12% to 37%. Three times better. Easy win, right?

But AI results are *noisy* — run the exact same thing twice, get different numbers. So before celebrating, I ran the *old* version three times too. Its real average? Twenty-nine percent. My "tripling" was actually a tiny bump — small enough that it could just be luck.

That's how most AI hype happens: someone runs it once, gets a lucky number, posts it. The honest version is boring — run it many times, compare averages, and admit when the difference is just noise. Which, this time, it was.

**CTA (65-80s):**
One good run isn't a result. If you didn't measure the noise, you don't have a finding — you have a coincidence.

---

## "The Bug Isn't Always in the File You're Looking At"

*[OPEN: you, screen with a few code files]*

**HOOK (0-5s):**
Here's why small AI agents fail at real codebases — and it's not what you'd think.

**CORE (5-60s):**
I gave my local AI a broken program and said "fix it." It opened the obvious file, found something that looked wrong, edited it, declared done. Test still failed.

Why? The bug wasn't in that file. The file it was told about called a *second* file, which called a *third* — and the real bug was three hops away, in a little helper function nobody points at.

Humans do this instinctively — follow the trail across files. A small model gets tunnel vision: it fixes the first plausible thing it sees. Real coding isn't fixing one line; it's *finding* the line, across a whole project. That localization is the hard part, and it's exactly the skill I'm trying to teach it.

**CTA (60-75s):**
Fixing code is easy. *Finding* the bug across a real codebase is the actual skill — for AI and humans both.

---

## "I Test My AI on Problems It Has Never Seen"

*[OPEN: you, two columns on screen: "trained" / "held out"]*

**HOOK (0-5s):**
There's one trick that separates an AI that *learned* something from one that just *memorized*. Here it is.

**CORE (5-65s):**
I'm teaching my small AI a new skill by showing it examples solved by a bigger one. But here's the danger: it might just memorize those specific examples and look smart without actually learning anything.

So I split the problems in two. I train it on one set — and then I test it on a *completely separate set it has never seen.* Same skill, brand-new problems.

If it only does well on what it studied, it memorized. If it does well on the problems it's never seen — *that's* real learning. Transfer. That's the only result I'd actually trust, and it's the one I'm running right now.

**CTA (65-80s):**
Never grade an AI on its homework. Grade it on the surprise exam. Held-out testing is how you catch memorization pretending to be intelligence.

---

## "Teaching an AI a Skill It Doesn't Have"

*[OPEN: you, two model names: 8B and 14B]*

**HOOK (0-5s):**
You can take a skill from a big AI and pour it into a small one — but only if you check one thing first.

**CORE (5-65s):**
My small local model couldn't find bugs across multiple files. I wanted to teach it. The method: let a bigger, smarter model solve those problems, record exactly how it did it, and train the small one to imitate.

But before spending hours on that — I checked the precondition that everyone skips: *can the big model actually do it?* If the teacher can't solve the problem either, there's nothing to teach, and you'd waste a whole day distilling failure.

So I measured. The big model solved about 60% of the hard cases; the small one, half that. A real, teachable gap. *Now* it's worth doing — collect the teacher's solutions, train the student, and test if the skill actually transferred.

**CTA (65-85s):**
Before you copy a skill from a big model to a small one, prove the big one even has it. Measure the gap first — or you're just distilling noise.

---

## Production Notes (Batch 3)
- ~60-85s each. Captions on. You + camera carries it; drop in terminal/score/code-file B-roll.
- These are the *honest* arc — the nulls (noise, saturation) are the point. That authenticity is the brand.
- Hashtags: #ProfessorX #AI #LocalAI #BuildInPublic #MachineLearning #OpenSource
