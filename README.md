# pact-lang

A compiler for **Pact** — a programming language designed for AI agents, not humans.

Pact inverts the usual ratio: programs are *mostly specification, provenance, and constraints* with a thin layer of computation. The logic is the easy part. Knowing **why** something exists, **what else is affected**, and **what guarantees must hold** is where the language spends its budget.

## What is Pact?

Pact is an S-expression based language where every function carries rich, machine-readable metadata:

```scheme
(fn get-user-by-id
  :provenance {req: "SPEC-2024-0042#section-3", test: ["T-101" "T-102" "T-103"]}
  :effects    [db-read http-respond]
  :total      true
  :latency-budget 50ms
  :called-by  [api-router/handle-request admin-panel/user-detail]

  (param id UUID :source http-path-param :validated-at boundary)

  (returns (union
    (ok   User   :http 200 :serialize :json)
    (err  :not-found {:id id} :http 404)
    (err  :invalid-id {:id id} :http 400)))

  (let [validated-id (validate-uuid id)]
    (match validated-id
      (err _)    (err :invalid-id {:id id})
      (ok  uuid) (match (query user-store {:id uuid})
                   (none)   (err :not-found {:id uuid})
                   (some u) (ok u)))))
```

Key language features:

- **Provenance** — every function and type knows *why* it exists (spec reference, author, tests)
- **Effect tracking** — functions declare exactly what I/O they perform (reads, writes, sends)
- **Totality** — functions marked `:total true` must handle all cases exhaustively
- **Latency budgets** — performance constraints are part of the code, not tribal knowledge
- **Dependency graphs** — `:called-by` makes impact analysis instant
- **Union return types** — every possible outcome is enumerated with HTTP status mappings
- **Type invariants** — constraints like `:min-len`, `:max-len`, `:format` are first-class

## Building

```bash
cargo build --release
```

No external dependencies. Just Rust's standard library.

## Usage

```bash
# Generate a .pct file from a YAML spec (human intent → machine format)
pact generate examples/user-service.spec.yaml -o user-service.pct

# Compile a Pact file to Rust source code
pact compile examples/user-service.pct -o output/

# Check for errors without generating code
pact check examples/user-service.pct

# Parse only (show the concrete syntax tree)
pact parse examples/minimal.pct
```

## Spec-to-Pct Generator

The `generate` command translates human-readable YAML specs (Layer 0 — human intent) into `.pct` files (Layer 1 — AI-native format) that feed into the compiler pipeline:

```
Spec (.yaml) → YamlParser → SpecAST → PctEmitter → .pct file → [compiler]
```

### YAML Spec Format

Write requirements in plain English:

```yaml
spec: SPEC-2024-0042
title: "User service"
owner: platform-team
domain:
  User:
    fields:
      - name: required, string, 1-200 chars
      - email: required, email format, unique
      - id: auto-generated, immutable
endpoints:
  get-user:
    description: "Returns a user by ID"
    input: user id (from URL)
    outputs:
      - success: the user found (200)
      - not found: when the ID doesn't exist (404)
    constraints:
      - max response time: 50ms
      - read-only
  create-user:
    description: "Creates a new user"
    input: user data (from body)
    outputs:
      - created: the new user (201)
      - duplicate email: email already exists (409)
      - validation failed: invalid input (422)
    constraints:
      - idempotent by: email
      - max response time: 200ms
quality:
  - all functions must be total
traceability:
  known dependencies: api-router, admin-panel
```

### What Gets Mapped

| Spec descriptor | Generated .pct |
|----------------|---------------|
| `required, string, 1-200 chars` | `(field name String :min-len 1 :max-len 200)` + invariant |
| `email format, unique` | `(field email String :format :email :unique-within <store>)` |
| `auto-generated, immutable` | `(field id UUID :immutable :generated)` |
| `read-only` constraint | effect set `db-read [:reads <store>]` |
| `max response time: 50ms` | `:latency-budget 50ms` |
| `idempotent by: email` | `:idempotency-key (hash (. input email))` |
| output `success (200)` | `(ok Type :http 200 :serialize :json)` |
| output `not found (404)` | `(err :not-found {:id id} :http 404)` |
| `all functions must be total` | `:total true` on every function |

The generator also scaffolds function bodies: read endpoints get validate-query-match logic, write endpoints get validate-insert-match logic.

The generated `.pct` is validated by round-tripping through lexer, parser, and lowerer before writing to disk.

### Scope and Limitations

The generator is designed for **service contract specifications** — CRUD endpoints, API contracts, input validation, error variants. It handles the domain well:

- Domain types with field constraints
- Read/write endpoints with HTTP status mappings
- Effect tracking, latency budgets, idempotency keys
- Traceability and provenance metadata

It is **not** designed for algorithmic specifications (data structures, sorting algorithms, state machines). Those require language features Pact doesn't yet have: generic types, recursive types, trait bounds, and algorithmic body templates.

## Compiler Pipeline

The compiler has 6 phases:

```
Source (.pct) → Lexer → Parser (CST) → Lowering (AST) → Semantic Analysis → Codegen (Rust)
```

| Phase | What it does |
|-------|-------------|
| **Lexer** | Tokenizes source into symbols, keywords, strings, integers, durations, regex literals |
| **Parser** | Builds a generic S-expression tree (lists, vectors, maps, atoms) — no semantic knowledge |
| **Lowering** | Converts CST to typed AST (Module, TypeDef, FnDef, Expr, Pattern, etc.) |
| **Semantic analysis** | Name resolution, effect checking, match exhaustiveness |
| **Codegen** | Emits Rust source: structs, traits, enums, functions with doc comments |

## What Gets Generated

Given a Pact module, the compiler produces Rust code with:

| Pact construct | Rust output |
|---------------|-------------|
| `(type User ...)` | `pub struct User` with `validate()` method |
| `(effect-set db-read ...)` | `pub trait DbRead` with typed methods |
| `(fn get-user ...)` | `pub fn get_user<Ctx: DbRead + ...>()` with trait-bounded context |
| `(returns (union ...))` | `pub enum GetUserResult` with `http_status()` and `Display` |
| `:provenance`, `:called-by`, etc. | Doc comments preserving all metadata |
| `:invariants`, `:min-len`, `:max-len` | Validation logic in `validate()` |

## Examples

The `examples/` directory contains several Pact modules:

### `minimal.pct` — Starting point
The smallest valid module. One type, one effect set, one function.

```bash
pact compile examples/minimal.pct -o output/
```

### `user-service.pct` — Canonical example
The reference example from the language spec. A user CRUD service with two functions (`get-user-by-id`, `create-user`), full provenance, effect tracking, and union return types.

```bash
pact compile examples/user-service.pct -o output/
```

### `auth-service.pct` — Authentication
Token-based authentication with session management. Demonstrates multiple effect sets (session reads/writes, user lookup, audit logging), expiration handling, and password verification flows.

```bash
pact compile examples/auth-service.pct -o output/
```

### `inventory.pct` — Inventory management
Stock tracking with reservations. Multiple types (`Product`, `StockEntry`, `Reservation`), cross-type queries, quantity constraints, and write-heavy operations.

```bash
pact compile examples/inventory.pct -o output/
```

### `notification.pct` — Notifications
Multi-channel delivery with template rendering. Shows send effects (`email-gateway`, `sms-gateway`), long latency budgets (2000ms), and chained operations (render → deliver → persist).

```bash
pact compile examples/notification.pct -o output/
```

## Language Reference

### Module

Every `.pct` file contains a single module:

```scheme
(module module-name
  :provenance {req: "SPEC-ID", author: "agent:name", created: "ISO-8601"}
  :version 7
  :parent-version 6
  :delta (operation target "description")

  ;; declarations: types, effect-sets, functions
  ...)
```

### Types

Types have named fields with constraints:

```scheme
(type User
  :invariants [(> (strlen name) 0) (matches email #/.+@.+/)]
  (field id    UUID   :immutable :generated)
  (field name  String :min-len 1 :max-len 200)
  (field email String :format :email :unique-within user-store))
```

Supported field annotations: `:immutable`, `:generated`, `:min-len`, `:max-len`, `:format`, `:unique-within`.

### Effect Sets

Effect sets declare what I/O operations a group of capabilities performs:

```scheme
(effect-set db-read  [:reads  user-store])
(effect-set db-write [:writes user-store :reads user-store])
(effect-set notify   [:sends  email-gateway])
```

Effect kinds: `:reads`, `:writes`, `:sends`.

### Functions

Functions carry metadata, parameters, return types, and a body:

```scheme
(fn function-name
  :provenance {req: "SPEC-ID", test: ["T-001" "T-002"]}
  :effects    [effect-set-1 effect-set-2]
  :total      true
  :latency-budget 50ms
  :called-by  [caller/function-name]
  :idempotency-key (hash (. input email))

  (param name Type :source http-path-param :validated-at boundary)

  (returns (union
    (ok  Type    :http 200 :serialize :json)
    (err :tag    payload-type :http 404)))

  body-expression)
```

### Expressions

```scheme
;; Let binding
(let [x (some-fn arg)] body)

;; Pattern matching
(match expr
  (ok value)   (ok value)
  (err _)      (err :tag {}))

;; Conditionals
(if condition then-expr else-expr)

;; Function calls
(function-name arg1 arg2)

;; Field access
(. object field-name)

;; Constructors
(ok value)
(err :tag payload)

;; Map literals
{:key value, :key2 value2}
```

### Literals

| Type | Examples |
|------|----------|
| Symbols | `foo`, `bar-baz`, `non-empty?`, `insert!`, `api-router/handle-request` |
| Keywords | `:provenance`, `:effects`, `:total`, `:not-found` |
| Strings | `"hello"`, `"SPEC-2024-0042"` |
| Integers | `42`, `-7`, `0` |
| Booleans | `true`, `false` |
| Durations | `50ms`, `200ms`, `10s`, `1h` |
| Regex | `#/.+@.+/` |
| Comments | `;; line comment` |

## Design Rationale

See [LANGUAGE.pt-BR.md](doc/LANGUAGE.pt-BR.md) (Portuguese) or [LANGUAGE.md](doc/LANGUAGE.md) (English) for the full design document. The core insight:

> The language ideal for AI is the one humans keep rejecting.

Every feature that helps an LLM reason about code — formal specs, effect tracking, totality checking, rich AST manipulation, exhaustive returns — adds cognitive load for humans. Pact embraces that trade-off: it's a language where the metadata-to-logic ratio is 3:1, because knowing *why*, *what's affected*, and *what guarantees must hold* is where AI agents spend their reasoning budget.

## Project Structure

```
pact-lang/
├── Cargo.toml
├── grammar.ebnf                  # Formal EBNF grammar
├── doc/
│   ├── LANGUAGE.md               # Language design document (English)
│   └── LANGUAGE.pt-BR.md         # Language design document (Portuguese)
├── src/
│   ├── main.rs                   # CLI entry point (compile, generate, check, parse)
│   ├── lib.rs                    # Module exports
│   ├── lexer.rs                  # Tokenizer (16 tests)
│   ├── parser.rs                 # S-expression CST parser (8 tests)
│   ├── ast.rs                    # Typed AST definitions
│   ├── lower.rs                  # CST → AST conversion (5 tests)
│   ├── diagnostics.rs            # Error/warning formatting
│   ├── semantic/
│   │   ├── mod.rs                # Analysis orchestration
│   │   ├── resolve.rs            # Name resolution
│   │   ├── effects.rs            # Effect checking (2 tests)
│   │   └── totality.rs           # Match exhaustiveness (2 tests)
│   ├── codegen/
│   │   ├── mod.rs
│   │   └── rust.rs               # Rust code emission (6 tests)
│   └── generate/
│       ├── mod.rs                # Module wiring + integration tests (4 tests)
│       ├── yaml_ast.rs           # YamlValue enum (Scalar, Mapping, Sequence)
│       ├── yaml_parser.rs        # Indentation-based YAML subset parser (12 tests)
│       ├── spec_ast.rs           # Typed spec structures (SpecDoc, Endpoint, etc.)
│       ├── spec_parser.rs        # YamlValue → SpecDoc conversion (11 tests)
│       └── pct_emitter.rs        # SpecDoc → .pct text emission (11 tests)
└── examples/
    ├── minimal.pct               # Smallest valid module
    ├── user-service.pct          # Canonical example (hand-written)
    ├── user-service.spec.yaml    # Example YAML spec for generate
    ├── inventory.spec.yaml       # Inventory service spec
    ├── auth-service.pct          # Authentication & sessions
    ├── inventory.pct             # Stock management & reservations
    └── notification.pct          # Multi-channel notifications
```

## Tests

```bash
cargo test
```

77 tests across all phases: lexer (16), parser (8), lowering (5), semantic analysis (4), codegen (6), generate (38).
