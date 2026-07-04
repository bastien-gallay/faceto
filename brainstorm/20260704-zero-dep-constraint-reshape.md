# Brainstorm: Reshape/replace faceto's "zero external dependencies" constraint

| Field | Value |
| --- | --- |
| **Date** | 2026-07-04 |
| **Duration** | 13 min (15:48 – 16:01) |
| **Participants** | User + AI Facilitator |
| **Problem shape** | Decision under constraints |

## Session Plan

| # | Phase | Technique | Duration | Status |
| --- | --- | --- | --- | --- |
| 0 | Intake | Seed + extend (grounded in Cargo.toml + CI job) | 2 min | Done |
| 1 | Diverge/Analyze | Constraint Mapping | 5 min | Done |
| 2 | Converge | MoSCoW (formalize new policy) | 4 min | Done |
| 3 | Crystallize | Action Items | 2 min | Done |

Impact/Effort was folded into action ordering — the session converged at Step 1
(Constraint Mapping surfaced the core/serve seam), so a separate scoring pass would have
manufactured noise.

## Ideas — Starting Point

- Goal (real): fast, light, local-first, trivial to install & use — for devs and teams.
- Proxy (current): "zero external crates, ever," enforced by counting `Cargo.lock` packages.
- Named cracks: runtime deps invisible (all in binary) so *zero* is arbitrary; dev-deps
  (a PBT lib) shouldn't count; the binary *will grow* — which package-count doesn't measure.
- `[AI]` candidate rules: zero **runtime** deps · binary-size budget · offline-installable ·
  no runtime network · curated allowlist · no build.rs/proc-macro · MSRV supply-chain audit.

## Step 1: Constraint Mapping (15:48 – 15:56)

### Output

Decomposed the goal; graded "zero deps" as a proxy for each facet:

| Goal facet | Really requires | "Zero deps" serves it? | Sharper proxy |
|---|---|---|---|
| Trivial install | first-try `cargo install`/binary, offline | Partly — dev-deps don't touch install | offline-installable + no build.rs/proc-macro |
| Light | small executable | **Yes — its one strong job** | **binary-size budget** |
| Fast (runtime) | low startup, snappy | Barely — that's the code | non-issue |
| Local-**first** | zero-setup/zero-network *default*; remote additive | "no network" would ban the roadmap | zero-setup default; remote opt-in |
| Easy build/contribute | fast clean build | dev-deps slow tests only | dev-deps out of scope |

**Insight:** one blunt rule proxied ~4 properties, poorly for most, and measured *count*
when the real fear is *size*.

### User Feedback

> "In Local-first, don't forget the 'first'. Some small collaboration features [on the
> roadmap] would break local-only but not local-first. Maybe the remote version, or all the
> serve part, will have to become a pluggable tool… reuse faceto as core lib/specialised
> service. BUT this decision isn't made yet, so local-FIRST."
>
> "pure-std core + dev-deps freed + size budget, let's go"

### Facilitator Notes

The core/serve seam was the convergence point. Once `serve`/remote/collab can spin out as a
separate tool reusing core, core has no reason to compromise — the pivotal fork
(pure-std-core vs curated-allowlist) collapsed to pure-std immediately. New-dimension signal
(user raised an architectural split not in the seed ideas) → allowed re-divergence, which
paid off.

## Step 2: MoSCoW — new policy (15:56 – 16:00)

Reshaped constraint: *"faceto-core is a pure-std, offline-installable, local-first library
and CLI. What ships in the binary stays zero-runtime-dependency and size-bounded; how we
test it does not."*

- **Must** — Core = zero **runtime** deps (normal graph only: `cargo tree -e normal --prefix none | sort -u` → just `faceto`).
- **Must** — dev-dependencies free (not built by `cargo install`, never in the binary).
- **Must** — binary-size budget in CI (anchor 905K → ceiling ~2 MB, tunable).
- **Should** — local-**first** phrasing (zero-setup/zero-network default; remote additive, never banned).
- **Could** — offline-install smoke check (`cargo install --offline`).
- **Won't (now)** — curated runtime allowlist (parked); governing serve/remote/collab with
  this rule (**named seam** — decide if/when it splits into a pluggable tool).

---

## Outcome

### Selected Ideas / Decisions

1. **Pure-std core, dev-deps freed, size budget** — the constraint binds what ships, not how
   we test; growth is measured directly (size) instead of by a proxy (package count).
2. **Scope the rule to faceto-core** — a future serve/remote/collab layer is a named seam,
   exempt, reusing core as a lib.
3. **Local-first, not local-only** — collaboration is allowed to exist; the local,
   zero-setup, offline default is what's protected.

### Action Items

- [ ] CI: replace `zero dependencies` job (Cargo.lock count) with normal-edge check (`cargo tree -e normal`).
- [ ] CI: add binary-size budget job (fail if release `faceto` > ~2 MB).
- [ ] Docs/copy: `Cargo.toml` comment, `CLAUDE.md` "no crates ever", README claim, and PR #36 badge (`dependencies: 0` → `runtime deps: 0`).
- [ ] Add the PBT crate under `[dev-dependencies]` (enabling change).
- [ ] Park: revisit the constraint when serve/remote/collab work starts.

---

## Session Meta-Analysis

- **Duration:** 13 min
- **Techniques used:** Constraint Mapping (8 min), MoSCoW (4 min)
- **Techniques skipped:** Impact/Effort (folded into action ordering — converged before it was needed), Six Hats (opt-in only, not requested)
- **Adaptations made:** Inserted the core/serve scoping axis mid-Step-1 on a new-dimension signal from the user; collapsed the pure-std-vs-allowlist fork once the seam appeared.
- **Problem shape:** Decision under constraints → held (the schema/lock-in-adjacent framing stayed a constraint decision, not an atom inventory).
- **Convergence point:** Step 1 (Constraint Mapping) — the core/serve seam.
- **What worked well:** Grounding in the actual CI job first meant the "proxy vs goal" split was concrete, not hand-wavy. Separating goal from proxy is the whole game for "reshape a rule" topics.
- **What could improve:** Could have offered the size-budget number earlier; measuring the 905K anchor upfront would have sharpened Step 1.
- **Session energy:** high — user came with the goal/proxy distinction already half-formed and made the call decisively.
- **Recommendation for similar sessions:** For "reshape/replace an existing rule" topics, always grep the rule's *enforcement* first, then run Constraint Mapping with an explicit "is the current rule a good proxy?" column. Watch for a scoping seam (this rule binds X, not Y) — it often collapses the decision.
