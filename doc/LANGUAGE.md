# Designing a Programming Language for AI Agents

A thought experiment on what a programming language would look like if it were designed for LLM-based coding agents rather than human programmers.

---

## Table of Contents

- [The Problem](#the-problem)
- [What Human-Oriented Design Gets Wrong for LLMs](#what-human-oriented-design-gets-wrong-for-llms)
  - [Safeguards That Become Redundant](#safeguards-that-become-redundant)
  - [What Would Actually Help an LLM](#what-would-actually-help-an-llm)
  - [The Counterintuitive Insight](#the-counterintuitive-insight)
  - [The Real Question](#the-real-question)
- [A Prototype: What It Would Look Like](#a-prototype-what-it-would-look-like)
  - [Layer 1: The AI-Native Format](#layer-1-the-ai-native-format)
  - [Layer 2: The Human Projection](#layer-2-the-human-projection)
  - [Layer 3: How an LLM Would Edit This](#layer-3-how-an-llm-would-edit-this)
  - [Design Decision Rationale](#design-decision-rationale)
  - [Key Takeaway](#key-takeaway)
- [Existing Languages: How Close Are We?](#existing-languages-how-close-are-we)
  - [Tier 1: Closest to the Vision](#tier-1-closest-to-the-vision)
  - [Tier 2: Gets Several Things Right](#tier-2-gets-several-things-right)
  - [Tier 3: Gets One or Two Things Right](#tier-3-gets-one-or-two-things-right)
  - [The Uncomfortable Pattern](#the-uncomfortable-pattern)

---

## The Problem

We design programming languages to be fast, secure, and primarily **user-friendly to human programmers** — modularity, isolation of scope, easy syntax, etc. But when the primary "user" of a language is an LLM-based coding agent, the constraints shift dramatically. Humans have limited working memory, read linearly, and parse visually. LLMs don't share those constraints — but have entirely different ones that current languages ignore.

---

## What Human-Oriented Design Gets Wrong for LLMs

### Safeguards That Become Redundant

- **Syntactic sugar** — An LLM doesn't need `for x in list` to be prettier than `loop(list, fn(x) -> ...)`. Both parse equally well. The entire concept of "readable syntax" is a human concern. Lisp's s-expressions or even raw AST notation would be fine.

- **Naming conventions** — camelCase vs snake_case, short vs descriptive names — these exist to help humans scan and remember. An LLM could work with UUIDs as identifiers if the *semantic graph* were available.

- **Indentation/formatting** — Entirely visual. Explicit block delimiters (or just an AST) are unambiguous. Python's significant whitespace is actually *harder* for LLMs because whitespace is tokenizer-hostile.

- **Boilerplate reduction** — DRY exists because humans hate repetitive typing and lose track of duplicated logic. An LLM doesn't get bored or lose track. Explicit repetition with guaranteed consistency checks could actually be *better* than implicit magic (think: Rails conventions that hide behavior).

- **Progressive disclosure of complexity** — Languages hide things (default parameters, implicit conversions, operator overloading) so humans aren't overwhelmed. This actively hurts LLMs — hidden behavior means the model needs to simulate a mental model of what the runtime *actually does* versus what the code *appears to do*.

### What Would Actually Help an LLM

1. **Rich, inline semantic metadata** — Not comments (natural language is ambiguous), but machine-readable *intent annotations*. "This function is pure." "This block must execute in < 10ms." "This invariant must hold: x > 0 at exit." Current type systems are a weak version of this. The ideal is **contracts, effects, and provenance** as first-class citizens, not bolted-on.

2. **Bidirectional traceability** — Every line of code should know *why it exists*. Link to the requirement, the test, the commit rationale. When editing a function, the language itself should say "this exists because of requirement R-1234, is tested by test T-56, and is depended on by modules A, B, C." Current languages have zero support for this — it lives in external tools (Jira, git blame, grep).

3. **Formal, queryable dependency graphs as a language primitive** — Not import statements to chase. The ability to ask the language runtime: "what is the full transitive impact of changing this type?" and get a precise answer. LSP is a hack approximation of this.

4. **Deterministic, total effect tracking** — Every function should declare *everything* it can do: I/O, mutation, allocation, panic, non-termination. Haskell's IO monad is directionally right but too coarse. The ideal: "this function reads from network, writes to this specific database table, and can throw these 3 error types." This lets an LLM reason about changes *without running code*.

5. **Native diff/patch semantics** — Current languages represent *state* (the current source). LLMs work in *deltas* (edits). A language designed for LLMs might represent programs as a *history of transformations* with semantic meaning, not flat text files. Think: a program is a chain of refactoring operations, not a pile of characters.

6. **Constraint-based specifications alongside implementation** — Instead of just writing code and hoping tests catch errors, every function would carry a formal spec. LLMs are much better at verifying "does this implementation satisfy this formal constraint?" than "does this code do what the vague comment says?"

7. **Elimination of ambiguous overloading** — Every operation should have exactly one meaning in context. The fact that `+` means integer addition, float addition, string concatenation, and list concatenation depending on types is a human convenience that creates an inference burden for LLMs.

### The Counterintuitive Insight

The language an LLM would *actually* want looks less like Python and more like a **typed, total, effect-tracked AST format with embedded formal specifications and full provenance metadata** — basically a rich IR (intermediate representation) with the semantic density of something like Lean 4 or Idris, but without any concern for how it looks on screen.

The irony: **this already exists in pieces.** LLVM IR, WebAssembly, Typed Racket, Dafny, F\*. Nobody uses them directly because they're hostile to humans. An LLM-native language would essentially be a *very* rich IR that no human would want to write, paired with a human-facing projection (like a view layer) for when people need to read it.

### The Real Question

The deeper issue isn't "what language should LLMs use" — it's **should LLMs use textual programming languages at all?** Text is a serialization format for human cognition. The ideal LLM programming interface might be direct manipulation of a semantic graph with formal verification at every step, where "source code" as flat text simply doesn't exist.

---

## A Prototype: What It Would Look Like

The same concept — a simple HTTP user service — shown in three layers: the actual AI-native format, a human projection of it, and the semantic editing interface.

### Layer 1: The AI-Native Format

This is what the LLM actually works with:

```scheme
(module user-service
  :provenance {req: "SPEC-2024-0042", author: "agent:claude-v4", created: "2026-02-09T14:00:00Z"}
  :version 7
  :parent-version 6
  :delta (added-fn get-user-by-id "support single-user lookup endpoint")

  (type User
    :invariants [(> (strlen name) 0) (matches email #/.+@.+\..+/)]
    (field id   UUID   :immutable :generated)
    (field name String :min-len 1 :max-len 200)
    (field email String :format :email :unique-within user-store))

  (effect-set db-read    [:reads  user-store])
  (effect-set db-write   [:writes user-store :reads user-store])
  (effect-set http-respond [:sends http-response])

  (fn get-user-by-id
    :provenance {req: "SPEC-2024-0042#section-3", test: ["T-101" "T-102" "T-103"]}
    :effects    [db-read http-respond]
    :total      true
    :latency-budget 50ms
    :called-by  [api-router/handle-request admin-panel/user-detail]

    (param id UUID
      :source http-path-param
      :validated-at boundary)

    (returns (union
      (ok   User   :http 200 :serialize :json)
      (err  :not-found {:id id} :http 404)
      (err  :invalid-id {:id id} :http 400)))

    ;; the actual logic — note how small it is relative to the metadata
    (let [validated-id (validate-uuid id)]
      (match validated-id
        (err _)    (err :invalid-id {:id id})
        (ok  uuid) (match (query user-store {:id uuid})
                     (none)   (err :not-found {:id uuid})
                     (some u) (ok u)))))

  (fn create-user
    :provenance {req: "SPEC-2024-0041", test: ["T-090" "T-091"]}
    :effects    [db-write http-respond]
    :total      true
    :idempotency-key (hash (. input email))
    :latency-budget 200ms

    (param input {:name String :email String}
      :source http-body
      :content-type :json
      :validated-at boundary)

    (returns (union
      (ok   User   :http 201 :serialize :json)
      (err  :duplicate-email {:email (. input email)} :http 409)
      (err  :validation-failed (list ValidationError) :http 422)))

    (let [errors (validate-against User input)]
      (if (non-empty? errors)
        (err :validation-failed errors)
        (match (insert! user-store (build User input))
          (err :unique-violation) (err :duplicate-email {:email (. input email)})
          (ok user)               (ok user))))))
```

### Layer 2: The Human Projection

This is auto-generated. Not source code — a read-only rendering, like how a database has a storage format and a query output. No human writes this; it's projected from Layer 1 whenever someone needs to review.

```python
# --- Auto-projected from user-service v7 ---
# Spec: SPEC-2024-0042 | Tests: T-101, T-102, T-103

@effects(reads="user_store")
@budget(latency="50ms")
@total
def get_user_by_id(id: UUID) -> User | NotFound | InvalidId:
    match validate_uuid(id):
        case Err(_):
            return InvalidId(id=id)         # → 400
        case Ok(uuid):
            match user_store.get(id=uuid):
                case None:
                    return NotFound(id=uuid) # → 404
                case user:
                    return user              # → 200
```

### Layer 3: How an LLM Would Edit This

An LLM wouldn't submit a text diff. It would submit a **semantic operation**:

```scheme
(edit user-service
  :operation add-field
  :target-type User
  :field (field role (enum :admin :member :guest) :default :member)
  :reason "SPEC-2024-0055: role-based access control"

  :cascading-impacts
    ;; the language ITSELF computes these and asks the LLM to confirm:
    ;; - create-user needs to accept optional 'role' param
    ;; - get-user-by-id return type now includes 'role'
    ;; - 3 tests need updating: T-090, T-091, T-101
    ;; - user-store schema migration required
    ;; - admin-panel/user-detail consumes User — verify compatibility

  :confirm true)
```

The system responds with:

```scheme
(edit-result
  :version 8
  :parent 7
  :changes-applied 4
  :changes-pending-review 2
  :migration-generated "M-0008-add-role-to-user"
  :tests-invalidated ["T-090" "T-091" "T-101"]
  :tests-auto-updated ["T-101"]    ;; trivial: return shape changed
  :tests-need-manual  ["T-090" "T-091"]  ;; behavioral: creation logic changed
  :downstream-verified ["admin-panel/user-detail: compatible"]
  :downstream-warning  ["api-router: new field not yet exposed in list endpoint"])
```

### Design Decision Rationale

| Design Decision | Human Language Equivalent | Why It Helps LLMs |
|---|---|---|
| S-expression AST | `if/else` blocks, braces | Zero parsing ambiguity, trivial to manipulate programmatically |
| `:provenance` on every node | Git blame + Jira links | Never have to ask "why does this exist?" — it's inline |
| `:effects` declarations | Implicit side effects | Know *exactly* what a function touches without reading its body |
| `:total` annotation | Hope + tests | Compiler-verified "no crashes, no infinite loops" — can be trusted |
| `:called-by` graph | Grep for usages | Impact analysis is instant, not a search problem |
| `:latency-budget` | SLA docs somewhere | Performance constraints are code, not tribal knowledge |
| Semantic edits vs text diffs | `sed` / find-replace | Declare *intent*, the system computes *consequences* |
| Cascading impact analysis | "Did you remember to update X?" | The language tells what else broke — no guessing |
| Explicit return union types | Exceptions thrown from anywhere | Every possible outcome is enumerated — no surprise runtime paths |

### Key Takeaway

The ratio of **metadata to logic** is roughly 3:1. In human languages it's the inverse. That's the fundamental shift — an AI-native language is *mostly specification, provenance, and constraints* with a thin layer of actual computation. The logic is the easy part. Knowing **why**, **what else is affected**, and **what guarantees must hold** is where LLMs actually spend their reasoning budget.

This isn't a language for writing code. It's a language for **maintaining systems**.

---

## Existing Languages: How Close Are We?

An honest ranking of existing popular languages, based on how much of the ideal each one already delivers natively.

### Tier 1: Closest to the Vision

#### Lean 4

The single closest thing that exists today. Dependent types mean specifications *are* the code — a function's type can literally state "returns a sorted list whose length equals the input's length." The proof obligations force total, verified logic. Its macro system operates on the AST directly, which is close to the "semantic edit" concept. The metaprogramming framework (Lean's `Elab` monad) lets you query and manipulate the proof environment programmatically.

**Missing pieces:** no effect tracking, no provenance, no built-in dependency graph queries.

#### F\* (F-Star)

Microsoft Research's language. Has *exactly* the effect system described above — you declare `ST` for state, `IO` for I/O, `Pure` for pure computation, and the compiler enforces it. Refinement types let you encode invariants like `:min-len 1` directly in the type. It can extract verified code to OCaml, F#, or C. It's the closest thing to "mostly specification with thin logic."

Almost nobody uses it outside of research, which is telling about the human-friendliness tradeoff.

#### Idris 2

Similar to Lean but with first-class **quantitative type theory** — the type system tracks *how many times* a value is used. This is a primitive form of resource/effect tracking. Its elaborator reflection lets programs inspect and modify their own type-checking process, which rhymes with the semantic editing concept.

### Tier 2: Gets Several Things Right

#### Rust

Not for the reasons people usually cite. The borrow checker is essentially a *compiler-verified effect system* for memory — it tracks aliasing, mutation, and lifetime at the type level. The trait system with `Send`, `Sync`, `Unpin` is effect tracking by another name. The `Result<T, E>` convention with exhaustive `match` gives explicit return unions. `cargo` gives a real dependency graph that can be queried.

**Missing:** no formal specs, no provenance, no totality checking, and the syntax is complex enough that LLMs burn tokens on lifetime annotation gymnastics.

#### Haskell

The OG for several of these ideas. Purity by default means effects are *always* explicit (the `IO` monad). The type system is powerful enough to encode many invariants. `hlint` and typed holes give structured feedback.

**Missing:** the effect story is coarse (just `IO` vs pure — no granularity on *what kind* of I/O), no provenance, no built-in specification language, and the lazy evaluation model means reasoning about performance is genuinely hard even for LLMs.

#### Dafny

Microsoft's verification-aware language. Has `requires`, `ensures`, `invariant` as first-class syntax — those are exactly the `:invariants` annotations described above. The verifier checks them at compile time. Loop termination is checked (`decreases` clauses = totality). It's basically "specification-heavy programming" incarnate.

**Weakness:** small ecosystem, no effect system, and it's oriented toward algorithm verification rather than system building.

### Tier 3: Gets One or Two Things Right

#### Elixir/Erlang (BEAM)

Surprising pick, but: OTP supervision trees are essentially a **declarative dependency and failure graph**. The process model gives natural effect isolation — each process is a boundary. Pattern matching on tagged tuples (`{:ok, result}` / `{:error, reason}`) is explicit union returns. `@spec` and `@doc` are inline metadata. Hot code reloading is primitive "semantic patching."

**Missing:** no formal verification, no type enforcement at compile time (Dialyzer is optional and incomplete), no provenance.

#### Scala 3

The effect system work (Caprese/capture checking) is heading in the right direction. Union types, match types, and opaque types give expressive return specifications. Inline metaprogramming via `scala.quoted` operates on typed ASTs. But it carries enormous JVM complexity baggage.

#### Ada/SPARK

SPARK subset is formally verifiable with contracts (`Pre`, `Post`, `Contract_Cases`). Used in aerospace and defense where "prove it can't crash" is a real requirement. Very close to the `:total` + `:invariants` concept. But the language is verbose, the ecosystem is small, and there's no effect tracking beyond what contracts express.

### The Uncomfortable Pattern

| What LLMs Want | Who Has It | Why It's Not Mainstream |
|---|---|---|
| Formal specs as code | Lean, F\*, Dafny | Steep learning curve for humans |
| Effect tracking | F\*, Haskell, Rust (partial) | Adds annotation burden |
| Totality checking | Lean, Idris, Agda | Rejects many "useful" programs |
| Rich AST manipulation | Lean, Lisp/Racket, Elixir macros | Humans find macros confusing |
| Exhaustive return types | Rust, Haskell, OCaml | Humans find `match` tedious vs exceptions |

Every feature exists somewhere. The reason no single language combines them all is that **each one adds cognitive overhead for humans**. The entire history of mainstream language design is about *removing* the things LLMs find most useful, because humans experience them as friction.

That's the core tension: **the ideal AI language is the one humans keep rejecting.**

---

## Addendum: Additional Thoughts

### Extra primitives that seem uniquely useful for LLMs

- **Canonical, lossless AST serialization** — A single, stable representation that is deterministic, diffable, and hashable. This removes formatter drift and makes semantic caching trivial.
- **Proof-carrying artifacts** — The compiler emits machine-checkable certificates tied to specs and effects, allowing incremental trust without rerunning the world.
- **Capability-scoped effects** — Effects should require explicit capability grants at the module boundary (not just declared in function signatures). This gives the language a built-in permission model.
- **Spec/test duality** — Specs should be executable, and tests should be derivable from specs. The boundary between the two should be thin and programmatic.

### Modularization for LLMs

LLMs can handle monoliths better than humans, but **modules still matter** as *semantic boundaries*:

- **Effect scoping**: capabilities are granted at module boundaries.
- **Ownership and invariants**: modules define which data invariants they own and enforce.
- **Dependency graphs**: the language can cache and re-verify smaller units.
- **Partial recompilation**: change impact is cheaper when units are isolated.

In an AI-native language, modularization should be **constraint- and capability-scoped**, not file-based. You can store everything in one file, but the *units* should still be explicit.

### Runtime observability as a language primitive

A language for LLMs should treat **structured tracing** as first-class:

- Every effect produces machine-readable trace events with causal links.
- Edits can be validated against *behavioral deltas*, not just type diffs.
- Provenance can extend into runtime, allowing “why” to be attached to “what happened.”

### Alternative framing

Instead of a “language,” think: **graph-native IR + semantic edit protocol + human projection DSL**. Text becomes a view, not the source. The primary interface is a constraint-checked, capability-secured semantic graph with proof-carrying metadata.
