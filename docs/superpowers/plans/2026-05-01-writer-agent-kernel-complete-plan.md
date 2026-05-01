# Cursor-Style Writer Agent Kernel — Complete Foundation Plan

## 0. Product Reframe

Forge is not a writing tool with AI features.

Forge is a persistent writing agent for long-form fiction: the writer's second brain, second novelist, and creative partner. The editor is only one surface. The core product is the agentic substrate that observes the novel project, maintains canon, understands writing intent, proposes changes, remembers decisions, and improves through author feedback.

The current codebase already contains useful pieces: `agent-harness-core`, `AgentLoop`, ambient agents, Project Brain RAG, Hermes memory, TipTap integration, multi-ghost FIM, lorebook, outline, chapter generation, and diagnostic panels. The missing foundation is not another panel. The missing foundation is a durable Writer Agent Kernel that turns these pieces into one long-lived collaborator.

## 1. Current-State Diagnosis

### 1.1 What Already Exists

- A Tauri + React + TipTap desktop shell.
- A Rust workspace split between product-specific `src-tauri` and reusable `agent-harness-core`.
- Provider abstraction, OpenAI-compatible streaming, tool registry, tool executor, compaction, permission policy, PTC, memory, vector DB, and ambient event bus.
- Editor events, ghost text, semantic lint, entity anchors, parallel drafts, Project Brain RAG, lorebook, outline, storyboard, Script Doctor, and chapter generation.
- OS keychain API key storage and CSP hardening.
- Basic tests across Rust runtime and frontend build/lint.

### 1.2 What Is Still Wrong

The current product still behaves too much like an AI writing application:

- The README still frames the app as a "local AI writing desktop app".
- `src-tauri/src/lib.rs` remains a large orchestration file.
- `ask_agent` creates a fresh `AgentLoop` per request instead of routing through a persistent project agent.
- Frontend operation handling still carries XML action-tag legacy.
- Attention and intent are mostly heuristic keyword/threshold logic.
- Memory exists, but it is not yet a structured creative ledger.
- RAG exists, but it is not yet a canon-aware reasoning substrate.
- Ambient agents exist, but they do not yet operate under a unified project-level agenda.

The result: Forge can assist writing, but it is not yet reliably a creative partner.

## 2. North Star

The user should feel:

> "I am not operating an AI writing tool. I am writing with a partner who understands this book, remembers our decisions, protects continuity, challenges weak scenes, and helps me continue without taking control away."

This implies five product laws:

1. The agent is persistent, not summoned.
2. The agent has project memory, not just chat history.
3. The agent proposes typed operations, not hidden text commands.
4. The agent explains evidence and tradeoffs, not just suggestions.
5. The agent improves from author feedback, not just prompt tweaks.

## 3. Foundation Architecture

### 3.1 Target Layers

```text
Frontend Surfaces
  Editor / Ghost Text / Companion Panel / Lore Cards / Draft Canvas
        |
Writing Agent Protocol
  Observations -> Proposals -> Operations -> Feedback -> Reflections
        |
Writer Agent Kernel
  Intent Engine / Context Engine / Canon Engine / Proposal Engine / Memory Engine
        |
Project Substrate
  Chapters / Outline / Lorebook / Canon Ledger / Promise Ledger / Style Ledger / Decision Ledger
        |
LLM + Local Runtime
  Provider / Tool Registry / Compaction / RAG / Tests / Traces / Permissions
```

### 3.2 Core Rule

The frontend must stop owning agent intelligence.

The frontend sends observations and renders proposals. The kernel decides what matters, which context is needed, whether to act, and which operation to propose.

## 4. Writer Agent Kernel

Create a new product-level module:

```text
src-tauri/src/writer_agent/
├── mod.rs
├── kernel.rs
├── observation.rs
├── intent.rs
├── context.rs
├── canon.rs
├── memory.rs
├── proposal.rs
├── operation.rs
├── feedback.rs
├── trace.rs
└── evaluation.rs
```

Longer-term, generic pieces should move into `agent-harness-core`, but the first implementation should stay product-specific enough to avoid premature abstraction.

### 4.1 Kernel Responsibilities

`WriterAgentKernel` owns:

- Current project session.
- Active chapter and editor state.
- Rolling observation stream.
- Current author intent estimate.
- Canon and memory snapshots.
- Proposal queue.
- Operation execution and approval.
- Feedback and learning events.
- Trace records for debugging and evaluation.

### 4.2 Kernel API

```rust
pub struct WriterAgentKernel {
    project_id: String,
    session_id: String,
    memory: WriterMemory,
    canon: CanonEngine,
    context: ContextEngine,
    intent: IntentEngine,
    proposal: ProposalEngine,
    trace: AgentTraceStore,
}

impl WriterAgentKernel {
    pub async fn observe(&mut self, observation: WriterObservation) -> Result<Vec<AgentProposal>, AgentError>;
    pub async fn apply_feedback(&mut self, feedback: ProposalFeedback) -> Result<(), AgentError>;
    pub async fn execute_operation(&mut self, operation: WriterOperation) -> Result<OperationResult, AgentError>;
    pub async fn refresh_project_model(&mut self, reason: RefreshReason) -> Result<(), AgentError>;
    pub fn status(&self) -> WriterAgentStatus;
}
```

## 5. Writing Agent Protocol

Replace XML-style actions as the primary agent operation mechanism.

### 5.1 Observation

An observation is not just text. It is the agent's sensory input.

```ts
interface WriterObservation {
  id: string;
  createdAt: number;
  source: "editor" | "outline" | "lorebook" | "chapter_save" | "manual_request";
  reason: "typed" | "idle" | "selection" | "chapter_switch" | "save" | "explicit";
  projectId: string;
  chapterTitle?: string;
  chapterRevision?: string;
  cursor?: { from: number; to: number };
  selection?: { from: number; to: number; text: string };
  prefix: string;
  suffix: string;
  paragraph: string;
  fullTextDigest?: string;
  editorDirty: boolean;
}
```

### 5.2 Proposal

A proposal is the agent saying: "I noticed X; here is what I suggest; here is why; here is the evidence."

```ts
interface AgentProposal {
  id: string;
  observationId: string;
  kind:
    | "ghost"
    | "parallel_draft"
    | "continuity_warning"
    | "canon_update"
    | "style_note"
    | "plot_promise"
    | "chapter_structure"
    | "question";
  priority: "ambient" | "normal" | "urgent";
  target?: TextRange;
  preview: string;
  operations: WriterOperation[];
  rationale: string;
  evidence: EvidenceRef[];
  risks: string[];
  confidence: number;
  expiresAt?: number;
}
```

### 5.3 Operation

Operations are typed, inspectable, and reversible.

```ts
type WriterOperation =
  | { kind: "text.insert"; chapter: string; at: number; text: string }
  | { kind: "text.replace"; chapter: string; from: number; to: number; text: string }
  | { kind: "text.annotate"; chapter: string; from: number; to: number; message: string; severity: string }
  | { kind: "canon.upsert_entity"; entity: CanonEntity }
  | { kind: "canon.upsert_rule"; rule: CanonRule }
  | { kind: "promise.add"; promise: PlotPromise }
  | { kind: "promise.resolve"; promiseId: string; chapter: string }
  | { kind: "style.update_preference"; preference: StylePreference }
  | { kind: "outline.update"; nodeId: string; patch: unknown };
```

### 5.4 Feedback

Feedback is what turns the app into a partner that learns.

```ts
interface ProposalFeedback {
  proposalId: string;
  action: "accepted" | "rejected" | "edited" | "snoozed" | "explained";
  finalText?: string;
  reason?: string;
  createdAt: number;
}
```

## 6. Creative Ledgers

The current memory/RAG is not enough. The agent needs structured ledgers.

### 6.1 Canon Ledger

Purpose: protect world truth.

Entities:

- Character
- Location
- Item
- Organization
- WorldRule
- TimelineEvent

Each entry stores:

- canonical name
- aliases
- attributes
- source chapter/location
- confidence
- last verified time
- contradiction history

Minimum SQLite tables:

```sql
canon_entities(id, kind, name, aliases_json, summary, attributes_json, confidence, created_at, updated_at)
canon_facts(id, entity_id, key, value, source_ref, confidence, status, created_at, updated_at)
canon_relations(id, from_entity, relation, to_entity, source_ref, confidence)
canon_conflicts(id, fact_a, fact_b, message, severity, status, created_at)
```

### 6.2 Promise Ledger

Purpose: track伏笔, unresolved questions, pending emotional beats, and delayed payoffs.

```sql
plot_promises(id, kind, title, description, introduced_chapter, introduced_ref, expected_payoff, status, priority)
plot_promise_events(id, promise_id, chapter, event_type, note, source_ref, created_at)
```

Kinds:

- clue
- mystery
- emotional_debt
- relationship_tension
- object_in_motion
- threat
- prophecy_or_rule

### 6.3 Style Ledger

Purpose: learn the author's taste.

```sql
style_preferences(id, key, value, evidence_ref, confidence, accepted_count, rejected_count, updated_at)
style_examples(id, kind, text, chapter, note, score)
```

Examples:

- prefers terse action beats
- rejects exposition-heavy lore dumps
- likes dialogue with subtext
- avoids modern slang
- prefers third-person limited

### 6.4 Decision Ledger

Purpose: remember why the book took a path.

```sql
creative_decisions(id, scope, title, decision, alternatives_json, rationale, source_refs_json, created_at)
proposal_feedback(id, proposal_id, action, final_text, reason, created_at)
```

This is the difference between memory and partnership.

## 7. Intent Engine

The current keyword classifier is acceptable as a prototype but too weak for a Cursor-style agent.

### 7.1 Target Intent Model

The engine should classify:

- dialogue
- action
- description
- transition
- exposition
- emotional beat
- conflict escalation
- reveal
- setup/payoff
- revision
- structural planning
- canon maintenance

Output:

```rust
pub struct WritingIntentEstimate {
    pub primary: WritingIntent,
    pub secondary: Vec<WritingIntent>,
    pub confidence: f32,
    pub cues: Vec<String>,
    pub desired_agent_behavior: AgentBehavior,
}
```

### 7.2 Implementation Stages

Stage 1:

- Keep local rules.
- Expand test corpus.
- Add confusion tests.

Stage 2:

- Add tiny local classifier or embedding nearest-neighbor intent classifier.
- Store labeled author feedback.

Stage 3:

- Hybrid local classifier + LLM verifier for ambiguous cases.
- Enforce latency budget: local first, cloud only if idle window allows.

## 8. Context Engine

The context engine decides what the agent knows at each moment.

### 8.1 Context Pack Contract

```rust
pub struct WritingContextPack {
    pub system_contract: String,
    pub project_brief: String,
    pub author_style: String,
    pub canon_slice: String,
    pub promise_slice: String,
    pub outline_slice: String,
    pub rag_excerpts: Vec<ContextExcerpt>,
    pub cursor_prefix: String,
    pub cursor_suffix: String,
    pub selected_text: String,
    pub budget_report: ContextBudgetReport,
}
```

### 8.2 Priority Order

For ghost writing:

1. Cursor prefix and current paragraph
2. Cursor suffix
3. relevant canon facts
4. relevant active promises
5. outline node
6. author style preferences
7. RAG excerpts

For continuity diagnostics:

1. current sentence/paragraph
2. canon facts for mentioned entities
3. timeline facts
4. recent chapter summaries
5. source references

For chapter generation:

1. target chapter outline
2. previous chapter summaries
3. active promises
4. canon facts
5. style ledger
6. RAG excerpts
7. neighboring full text slices

## 9. Proposal Engine

The proposal engine decides what to show and how intrusive it should be.

### 9.1 Proposal Priority

Ambient:

- ghost text
- entity hover card
- weak style note

Normal:

- parallel draft
- pacing note
- unresolved promise reminder

Urgent:

- hard canon contradiction
- save conflict
- destructive operation requiring approval

### 9.2 Suppression Rules

The agent must know when to shut up.

Suppress if:

- user is actively typing
- same proposal rejected recently
- confidence is below threshold
- proposal repeats known advice
- target text changed
- model evidence is stale

### 9.3 Acceptance Loop

Every proposal must produce feedback:

- accepted
- rejected
- ignored until expired
- edited after insertion
- snoozed

This feedback updates style ledger and attention policy.

## 10. Canon Engine

The canon engine maintains story truth.

### 10.1 Sources

- explicit lorebook entries
- accepted canon update proposals
- chapter text extractions
- outline facts
- user decisions

### 10.2 Fact Confidence

Not all facts are equal.

Priority:

1. user-confirmed lorebook/canon entry
2. accepted agent proposal
3. repeated chapter evidence
4. single inferred chapter evidence
5. low-confidence model extraction

### 10.3 Conflict Flow

Example: user writes "林墨拔出一把长剑", canon says "林墨惯用寒影刀".

Flow:

1. Observation detects entity + weapon mention.
2. Canon engine retrieves facts about 林墨.weapon.
3. Conflict rule compares "长剑" vs "寒影刀".
4. Proposal engine emits `continuity_warning`.
5. UI renders weak underline.
6. User can accept fix, ignore, or update canon.

## 11. Agent Surfaces

### 11.1 Editor Surface

Must support:

- multi-ghost candidates
- semantic lint
- entity anchors
- inline typed operations
- proposal accept/reject/edit

### 11.2 Companion Panel

Not a chat box.

It should show:

- active scene goal
- unresolved promises
- current emotional arc
- canon risks
- recent decisions
- agent's current observation status

### 11.3 Parallel Draft Canvas

Purpose: co-writing, not generation dumping.

Requirements:

- three draft branches
- branch rationale
- sentence-level drag/insert
- diff against user's current text
- feedback capture

### 11.4 Project Brain

Must evolve from RAG search to associative trails:

- "this line relates to these promises"
- "this character state changed here"
- "this object was last seen there"
- "this rejected idea should not be reintroduced"

## 12. Evaluation Harness

No evaluation means guaranteed self-deception.

Create:

```text
agent-evals/
├── fixtures/
│   ├── canon_conflict_weapon/
│   ├── dialogue_voice/
│   ├── unresolved_promise/
│   ├── timeline_conflict/
│   ├── pacing_flat_scene/
│   ├── style_preference/
│   └── chapter_generation_context/
├── expected/
├── run_eval.rs
└── reports/
```

### 12.1 Required Eval Cases

1. Weapon contradiction: detects wrong weapon.
2. Character voice: flags dialogue inconsistent with profile.
3. Promise tracking: notices unresolved object introduced earlier.
4. Timeline contradiction: flags impossible time/order.
5. Pacing: identifies flat scene with no conflict movement.
6. Style preference: avoids rejected style pattern.
7. Parallel draft: generates three meaningfully different branches.
8. Context budget: includes required canon facts under tight budget.
9. Suppression: does not suggest during active typing.
10. Feedback learning: reduces repeated rejected suggestion.

### 12.2 Metrics

- detection precision
- detection recall
- suggestion acceptance rate
- repeated rejection rate
- average latency
- context budget waste
- false interruption rate
- proposal evidence coverage

## 13. Engineering Roadmap

### Phase A: Kill Legacy Protocol Risk

Goal: typed agent operations replace XML action tags.

Tasks:

- Add `WriterObservation`, `AgentProposal`, `WriterOperation`, `ProposalFeedback` to protocol.
- Add Rust mirror structs.
- Add operation executor with revision checks.
- Convert inline insert/replace flows to typed operations.
- Keep XML tags only as backward-compatible fallback.
- Remove `harness_echo` command.

Acceptance:

- No new feature depends on XML action parsing.
- All write operations carry target chapter, range, revision, and rationale.
- Failed operations return typed errors.

### Phase B: Writer Agent Kernel Skeleton

Goal: one persistent project agent owns observations and proposals.

Tasks:

- Create `src-tauri/src/writer_agent`.
- Add `WriterAgentKernel` managed in Tauri state.
- Route `agent_observe`, `report_editor_state`, `report_semantic_lint_state`, and `ask_agent` through kernel.
- Add trace logging per observation/proposal.
- Add kernel status command.

Acceptance:

- `ask_agent` no longer creates an isolated ad hoc agent without session state.
- Kernel can report current project state, active proposal count, and recent observations.

### Phase C: Creative Ledgers

Goal: move from memory blobs to structured project mind.

Tasks:

- Add SQLite tables for canon, promises, style preferences, creative decisions.
- Add migration/versioning.
- Add CRUD APIs and tests.
- Seed canon ledger from existing lorebook.
- Extract promises from outline/chapter save events.
- Record proposal feedback.

Acceptance:

- Agent can answer: "what unresolved promises exist in this chapter?"
- Agent can answer: "what canon facts are relevant to this sentence?"
- Feedback updates style preferences.

### Phase D: Context Engine

Goal: deterministic context packs for every agent action.

Tasks:

- Implement `WritingContextPack`.
- Define context budgets by task type.
- Add source reports and truncation warnings.
- Replace ad hoc prompt assembly in CoWriter and chapter generation.
- Add eval for required-source inclusion.

Acceptance:

- Every LLM call has a budget report.
- Tests prove canon facts outrank low-value RAG under tight budgets.

### Phase E: Intent Engine

Goal: stable author-intent detection.

Tasks:

- Expand writing intent taxonomy.
- Add fixture corpus.
- Add deterministic rule classifier with test coverage.
- Add optional small local classifier layer.
- Add feedback labels from accepted/rejected proposals.

Acceptance:

- Intent classifier has measurable precision on fixtures.
- Ghost prompts and proposal priorities derive from intent estimate.

### Phase F: Canon + Promise Diagnostics

Goal: ambient agent protects story continuity.

Tasks:

- Detect mentioned entities in current paragraph.
- Retrieve canon facts and active promises.
- Emit typed continuity proposals.
- Render weak underlines and hover evidence.
- Add "update canon instead" action.

Acceptance:

- Weapon contradiction eval passes.
- Timeline contradiction eval passes.
- User can resolve by editing text or updating canon.

### Phase G: Companion Panel as Second Brain

Goal: replace chat-first side panel with agent state.

Tasks:

- Add active scene goal.
- Add unresolved promises.
- Add canon risks.
- Add emotional arc/pacing note.
- Add recent accepted decisions.
- Move chat to secondary tab or command input.

Acceptance:

- Without opening chat, writer can see what the agent believes matters now.

### Phase H: Evaluation Gate

Goal: no agent change ships without eval.

Tasks:

- Create `agent-evals`.
- Add golden fixtures.
- Add eval runner.
- Add report JSON.
- Add CI-friendly command.

Acceptance:

- `cargo test` covers unit behavior.
- `cargo run -p agent-evals` covers product behavior.
- Regressions are visible before manual testing.

## 14. File-Level Refactor Targets

### 14.1 `src-tauri/src/lib.rs`

Target: reduce to app setup, state wiring, and command routing.

Move out:

- API key helpers -> `auth.rs`
- diagnostics export -> `diagnostics.rs`
- agent commands -> `commands/agent.rs`
- project commands -> `commands/project.rs`
- lore commands -> `commands/lore.rs`
- chapter generation commands -> `commands/chapter.rs`
- ambient output mapping -> `writer_agent/surface.rs`

### 14.2 `src/protocol.ts`

Target: split into domain protocols.

```text
src/protocol/
├── commands.ts
├── events.ts
├── writer-agent.ts
├── chapters.ts
├── lore.ts
└── index.ts
```

### 14.3 `src/components/AgentPanel.tsx`

Target: stop being the primary agent interface.

Split:

- `CompanionPanel`
- `ChatConsole`
- `ProposalFeed`
- `PromiseLedgerPanel`
- `CanonRiskPanel`

## 15. Product Acceptance Criteria

Forge becomes a Cursor-style writing agent only when these are true:

- The agent notices relevant project facts without being asked.
- The agent can explain proposal evidence.
- The agent can propose reversible typed edits.
- The agent remembers accepted/rejected creative decisions.
- The agent maintains canon and unresolved promises.
- The agent can run continuity/pacing checks automatically.
- The agent's behavior improves through author feedback.
- The agent has measurable evals, not demo-only confidence.

## 16. Immediate Next Sprint

Do not build more visual panels first.

Build this sprint:

1. `writer_agent` module skeleton.
2. typed `WriterObservation`, `AgentProposal`, `WriterOperation`, `ProposalFeedback`.
3. operation executor with revision checks.
4. creative ledger SQLite migrations.
5. first 5 eval fixtures.
6. route existing `agent_observe` through kernel.
7. convert one flow: continuity warning -> typed proposal -> UI underline -> feedback record.

Definition of done:

- One end-to-end typed proposal exists.
- It has evidence.
- It can be accepted/rejected.
- Feedback is stored.
- Eval covers it.

## 17. Non-Goals

Avoid these until the foundation is stable:

- More decorative UI.
- More generic chat abilities.
- More one-off prompt commands.
- More untyped XML action tags.
- More isolated panels that do not feed the kernel.
- More memory features without feedback/evaluation.

## 18. The Hard Truth

The product does not win by writing prettier paragraphs.

It wins by becoming the only system that can hold an entire novel in working memory, protect its truth, understand the writer's current intent, and collaborate without stealing authorship.

