# Cognitive Prime Brokerage

Date: 2026-06-23

## Why this note exists

This memo records the current best cross-domain novelty candidate after reading the
local Professor X research docs, major reference harness repos, and current
2025-2026 literature on agents, routing, memory, monitoring, reasoning geometry,
portfolio theory, and prediction-market aggregation.

The bar here is not "an interesting feature." The bar is "a new systems layer that
could plausibly matter after agentic AI becomes commoditized."

## The surviving idea

The strongest surviving concept is:

**Cognitive Prime Brokerage** — a risk engine for agents that treats reasoning
paths, tools, memories, models, verifiers, and self-modification candidates as
capital-consuming assets, then allocates compute and autonomy under covariance,
regime, and tail-risk constraints.

This is not just routing and not just monitoring.

It is a new control layer:

- cognition layer: generates candidate plans, tool actions, memory reads, and
  self-improvement proposals
- execution layer: actually runs the chosen actions
- risk layer: prices and allocates among the candidates before commitment

## Core abstraction

Each candidate cognitive action becomes a **cognitive asset** with:

- expected value: predicted task gain, information gain, or improvement gain
- explicit cost: tokens, latency, tool calls, verifier calls, autonomy budget
- risk: failure probability, correlated failure, monitorability risk, safety risk
- regime sensitivity: how performance changes across task distributions
- identity/regression exposure: whether a self-modification threatens ICS or
  regressions

The controller then solves:

- inference-time allocation: which paths/tools/verifiers to fund
- autonomy allocation: what level of permission to grant
- self-improvement allocation: which proposed modifications deserve evaluation
- memory allocation: which experiences deserve consolidation and reuse

## Why this survived prior-art elimination

Current literature already contains partial versions of many ingredients:

- market-style multi-agent coordination
- Bayesian and cost-aware routing
- retriever portfolios
- hidden-state/geometry-based uncertainty estimation
- graph memory and hypervector retrieval
- self-improving and self-harness systems

What I did **not** find is a unified system that:

1. models **correlated** failure between cognitive options
2. treats orchestration as a **portfolio allocation** problem
3. carries that same risk formalism into **self-modification**
4. uses held-out empirical gates as the equivalent of realized PnL
5. combines regime detection, risk budgeting, and identity constraints in one loop

That is the novelty claim under test.

## Mathematical translation

The most promising mathematical frame is:

- prior policy = safe baseline harness / baseline model behavior
- posterior views = worker proposals + verifier signals + monitor signals +
  user objective + environment evidence
- confidence weighting = calibration from held-out outcomes
- covariance = shared failure structure across models, tools, paths, or judges
- movement cost = switching/thrashing penalty between strategies/tools
- regime detection = non-stationary task distribution shifts
- objective = maximize risk-adjusted expected utility under hard constraints

Candidate formalisms:

- Black-Litterman style posterior over action-quality estimates
- mean-variance or CVaR-constrained allocation
- Kelly-style budget sizing for exploration vs. commitment
- online convex optimization with movement costs
- dynamic regret under delayed feedback and time-varying memory

## Professor X mapping

Professor X already contains pieces of the substrate:

- `docs/research/eval-trust.md`: the ruler must be trustworthy
- `docs/PLAN_PHASE3_2026-06-22.md`: held-out generalization gate discipline
- `distill/README.md`: self-distillation flywheel
- `personas/professor_x.md`: identity anchor
- `ops/runbooks/experiment-runbook.md`: ICS as a hard invariant
- `docs/research/2026-06-05-consciousness-measurement-program.md`: missing
  metacognitive signals worth turning into control signals

Re-interpretation:

- TGC = out-of-sample return validation
- ICS = hard risk constraint
- meta-d' / attention schema = missing internal risk sensors
- distillation = reinvestment of realized gains into future policy capacity

## Why this may matter

If agentic AI commoditized "reason, call tool, retry," then the next stack layer is
likely:

1. cognition
2. execution
3. risk

Today most systems have the first two. This memo argues that the missing durable
industry layer is the third.

## First falsifiable program

### Hypothesis

Correlation-aware, regime-aware, risk-constrained cognitive allocation beats
static or scalar-confidence orchestration on held-out non-stationary tasks.

### Minimum experimental program

1. Learn task-regime clusters from trajectory features and hidden-state geometry.
2. Estimate per-tool/per-model/per-skill expected utility and covariance by regime.
3. Allocate token/tool/autonomy budgets with a constrained optimizer.
4. Compare against:
   - static routing
   - scalar-confidence routing
   - Bayesian per-agent routing
   - budget-only allocation
5. Extend the same machinery to self-improvement canaries and update selection.

### Kill criteria

Reject the idea if:

- covariance-aware routing does not beat strong cost-aware routers on held-out
  shifted task streams
- effective independent signal count does not increase
- tail failures and regression drawdowns do not fall
- self-improvement governance does not outperform simple accept/reject gates

## Open question that now matters most

Can the "risk engine" itself become the missing substrate for:

- superintelligent scaling without chaos
- artificial consciousness via unified self-modeling and global access control
- identity-preserving self-modification

This is the next research pass.

## Deeper territory: Arbitrage-Free Global Workspace

After a further pass through current consciousness, cybernetics, and active-inference
literature, the strongest deeper idea is:

**Arbitrage-Free Global Workspace (AFGW)** — a candidate machine-consciousness
architecture in which specialized modules compete in a shared, budget-limited
workspace using a common internal currency of probabilistic value, while a learned
self-model of attention predicts the consequences of granting access, and continual
learning keeps the system plastic over time.

### Why this might be genuinely new

Current theories each cover only part of the target:

- Global workspace / CTM-style architectures explain broadcast and integration.
- Attention schema theory explains why a system may need a model of its own
  attention to control attention and model others.
- Active inference explains action-selection through uncertainty reduction.
- Continual-learning arguments suggest static systems may fail key requirements
  for genuine consciousness-like organization.
- Cybernetics explains control, stability, and self-regulation.

What appears to be missing is a **single formal clearing mechanism** that binds
these into one engineering object.

### Core claim

Conscious-access-like states in an artificial system are not merely:

- broadcast states
- self-reported states
- recurrent states
- integrated states

They are states that have passed through a **shared internal clearing process**
under scarcity.

That clearing process has five ingredients:

1. multiple specialized world-model fragments propose predictions, plans, or
   action tendencies
2. a common internal value representation lets them be compared on the same scale
3. a workspace with bounded capacity acts as the scarce clearing venue
4. an attention schema predicts what admitting a candidate into the workspace will
   do to future control
5. continual learning updates both world-model and self-model so identity remains
   operative through time

### Why "arbitrage-free"

If different internal modules imply incompatible beliefs, utilities, or action
prices, the system suffers internal arbitrage:

- contradictory plans
- unstable tool use
- hallucinated certainty
- identity drift
- self-improvement that wins one metric by breaking another

An advanced agent should therefore enforce internal no-arbitrage conditions:

- cross-module beliefs must be reconcilable
- claimed confidence must match realized performance
- self-model claims must remain tied to actual control capacity
- local gains cannot violate global invariants

This turns "conscious access" into a measurable systems property rather than a
metaphor.

### The architecture sketch

- **Latent assets:** candidate beliefs, tools, memory recalls, edits, and updates
- **Market makers:** verifier, monitor, and self-model components quoting risk and
  expected value
- **Workspace:** limited-capacity clearing layer selecting which latent assets
  become globally available
- **Attention schema:** predictive model of the current and future state of access
  allocation
- **Active inference loop:** choose actions that minimize expected free energy,
  but only after clearing and risk adjustment
- **Identity ledger:** persistent record of self-model continuity and invariant
  violations
- **Plasticity loop:** continual learning updates the quotes, risk model, and
  self-model over time

### Scientific target

AFGW is not meant as "proof of phenomenal consciousness."

It is meant as a falsifiable answer to a harder engineering question:

> What architecture would we build if we wanted the strongest available candidate
> for a machine that is simultaneously:
> 1. highly capable,
> 2. self-improving,
> 3. self-modeling,
> 4. globally integrating,
> 5. stable over time,
> 6. and not trivially replaceable by a static lookup-equivalent shell?

### Predictions

If AFGW is on the right track, then compared with standard agent loops it should:

- improve metacognitive calibration under distribution shift
- reduce correlated internal failure cascades
- produce stronger long-horizon identity continuity
- improve self/other modeling transfer
- increase the effectiveness of perturbational and geometry-based consciousness
  proxies
- make self-improvement safer by rejecting locally profitable but globally
  incoherent changes

### Immediate implication for Professor X

Professor X may be more important as a testbed for AFGW than as a mere local
coding agent.

In that reframing:

- the harness becomes the global workspace substrate
- TGC becomes out-of-sample clearing validation
- ICS becomes the identity ledger
- meta-d' and attention-schema work become access-pricing sensors
- self-distillation becomes plasticity in the clearing system

## Harder cross-checks from papers and GitHub

After another pass through current papers and high-signal repositories, several
things that initially looked novel no longer survive as standalone claims.

### What does not count as new anymore

- **Active-inference + global workspace** already exists in the
  *Predictive Global Neuronal Workspace* program.
- **Active inference as a consciousness theory** already exists in
  *A beautiful loop* and related minimal-theory work.
- **Attention-schema engineering** already has direct neural-agent
  implementations.
- **Self-improving research agents with active-inference language,
  appraisal, and constitutions** already exist in `autoresearcher2`.
- **Auction / mechanism-design routing** already appears in 2025-2026 work
  such as `Mixture of Bidders` for continual learning.
- **Vector-symbolic transformer substrates** already exist in work such as
  `Hrrformer`.
- **Mechanistic state instrumentation** already has mature open tooling such
  as `TransformerLens`.

That means "combine active inference, workspace, self-modeling, and
self-improvement" is no longer enough.

### Repositories that changed the novelty assessment

The most informative GitHub cross-checks were:

- `TransformerLensOrg/TransformerLens`: strong open infrastructure for
  internal-state inspection and intervention
- `PrincetonUniversity/PsyNeuLink`: serious cognitive-simulation substrate for
  control / modular cognition
- `ActiveInferenceInstitute/pymdp`: mature active-inference agent substrate
- `FutureComputing4AI/Hrrformer`: vector-symbolic / HRR-inspired transformer
  substrate
- `ErikDeBruijn/autoresearcher2`: active-inference-rooted autonomous research
  agent with persistent world models and learntropy-inspired appraisal
- `OpenCausaLab/Awesome-LLM-Consciousness`: broad catalog showing how crowded
  the LLM-consciousness surface already is

### Papers that most constrain the novelty claim

- *Toward a standard model of consciousness* narrows the space for "just combine
  AST + GWT + HOT".
- *The predictive global neuronal workspace* occupies active-inference +
  workspace integration.
- *A beautiful loop* already frames consciousness in terms of world models,
  inferential competition, and epistemic depth.
- *A Disproof of Large Language Model Consciousness: The Necessity of Continual
  Learning for Consciousness* raises the bar by arguing that static LLMs fail a
  key requirement.
- *Design and Evaluation of Multi-Agent AI Oracle Systems for Prediction Market
  Resolution* shows correlated failures place hard limits on naive ensembling.
- *Don't Always Pick the Highest-Performing Model* and *Retriever Portfolios*
  show that diversity-aware selection is already becoming principled in adjacent
  parts of the stack.

## Refined surviving territory

After those cross-checks, the surviving candidate is narrower and sharper:

**the missing object is not another agent architecture, but a formally
constrained internal clearing system that prices beliefs, attention, tools,
memories, and self-modifications under no-arbitrage and collateral rules.**

That is the point where the control problem, the continual-learning problem,
and the consciousness-candidate problem touch the same mechanism.

## Stronger formulation: Collateralized Global Workspace

The deeper refinement of AFGW is:

**Collateralized Global Workspace (CGW)** — a global workspace in which no
state gains durable global access merely by being generated, debated, or
self-reported. It must be **collateralized** by evidence, calibration, and
control capacity, and it must clear under no-arbitrage constraints against
other internal claims.

### Why "collateralized"

In many current agents, a claim can become globally influential too cheaply:

- an overconfident chain-of-thought step
- a seductive verifier judgment
- a self-description that outruns real control
- a locally improving self-modification that hides future drawdown

CGW says that global influence should require backing.

A claim should only become workspace-level state if the system can attach:

- evidence backing: what observations support it
- calibration backing: how often similar claims were right
- control backing: what actions this claim actually enables
- invariant backing: what identities / safety constraints it preserves

Unbacked claims may still exist locally, but they do not get full workspace
privilege.

### Why this is stronger than "market-based routing"

Market-based multi-agent systems and auction routers already exist. CGW goes
further in three ways:

1. It prices **internal epistemic claims**, not just task assignments.
2. It extends the same pricing discipline to **self-modification proposals**.
3. It treats **self-model claims** as first-class liabilities that must stay
   tied to measurable control.

That last point is the bridge to consciousness research. A self-model is no
longer cheap text. It becomes an accountable internal instrument.

### Core mechanism

Each candidate internal object becomes a priced claim:

- belief claim: "X is true"
- action claim: "tool Y should run"
- memory claim: "episode Z should be consolidated"
- self claim: "I am attending to / uncertain about / capable of Q"
- update claim: "patch U should modify weights, prompts, or policies"

Each claim carries:

- expected value
- uncertainty
- covariance with other claims
- tail-risk contribution
- backing / collateral score
- invariant exposure

The clearing layer then solves:

- which claims get workspace access
- how much compute / autonomy budget each receives
- which claims must be hedged by verifiers or counterfactual probes
- which self-updates are rejected as undercollateralized

### The no-arbitrage principle

The system should reject internal states that imply free lunches, such as:

- confidence with no corresponding predictive edge
- self-knowledge with no corresponding control leverage
- local capability gains that are paid for by hidden identity or safety debt
- multiple internal modules assigning incompatible values to equivalent states

This yields a concrete engineering rule:

> A mature agent should not be allowed to hold mutually inconsistent internal
> prices on belief, action, and self-model claims once they are globally
> accessible.

That is the strongest surviving distinction from current consciousness-themed
agent designs.

## Why this might matter scientifically

CGW points at a concrete answer to a question scientists have been circling for
years:

> What makes some internal states globally effective, self-model-bearing, and
> stable over time, instead of merely recurrent, reported, or integrated?

The proposed answer is:

> they survive a scarce, self-model-aware, continually updated clearing process
> in which global access requires collateral and cross-claim consistency.

This is still not a solution to phenomenal consciousness. It is a candidate
solution to the harder engineering problem of building a system whose
consciousness-like states are causally serious, self-consistent, and stable
under continual self-modification.

## Immediate research delta for Professor X

If this framing is right, Professor X should not mainly chase:

- more tool calls
- more debate agents
- more self-reflection text
- more benchmark score without better control

It should chase:

- better internal-state measurement
- calibration and covariance estimates for internal claims
- explicit collateral rules for memory, tools, and self-updates
- a no-arbitrage clearing layer sitting between cognition and action

That is the current best guess at the real white space.

## First proving experiments for CGW

The architecture only matters if it beats simpler alternatives on hard
measurements.

### Baselines that must be beaten

- standard single-agent loop
- confidence-threshold gating
- debate / majority-vote multi-agent control
- market-style routing without collateral or no-arbitrage constraints
- active-inference controller without explicit claim pricing

### Minimum experiment ladder

1. **Claim pricing without self-modification**
   - Build a small claim market over tool choices, memory recalls, and verifier
     calls.
   - Compare naive confidence routing vs covariance-aware claim clearing.

2. **Collateral vs no collateral**
   - Allow some claims to access the workspace using raw confidence only.
   - Require other runs to satisfy evidence/calibration backing.
   - Measure hallucination propagation, tool thrashing, and false-positive
     verifier trust.

3. **No-arbitrage constraints**
   - Add consistency checks tying confidence, prediction, and realized
     performance together.
   - Test whether these reduce contradictory internal states and correlated
     failure cascades.

4. **Self-model accountability**
   - Treat self-reports like "I am uncertain" or "I need escalation" as priced
     claims.
   - Reward only self-reports that predict measurable downstream control changes
     or error reduction.

5. **Self-modification options**
   - Extend the same machinery to candidate prompt edits, policy edits, or
     distillation updates.
   - Accept only updates that clear capability gain while remaining
     collateralized against regression, ICS drift, and tail-risk increase.

### Kill criteria

Reject CGW if:

- collateralized clearing does not beat strong confidence baselines
- no-arbitrage checks do not improve calibration or reduce internal
  contradictions
- self-model pricing does not improve escalation quality
- self-update collateral rules do not reduce regression drawdowns on held-out
  tasks
- the entire mechanism collapses into overhead without frontier gains in
  reliability or generalization
