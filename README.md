# ais-lang

A compiler for **AIS** (AI S-expressions) — a programming language designed for AI agents, not humans.

AIS inverts the usual ratio: programs are *mostly specification, provenance, and constraints* with a thin layer of computation. The logic is the easy part. Knowing **why** something exists, **what else is affected**, and **what guarantees must hold** is where the language spends its budget.

## What is AIS?

AIS is an S-expression based language where every function carries rich, machine-readable metadata:

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
# Compile an AIS file to Rust source code
ais-lang compile examples/user-service.ais -o output/

# Check for errors without generating code
ais-lang check examples/user-service.ais

# Parse only (show the concrete syntax tree)
ais-lang parse examples/minimal.ais
```

## Compiler Pipeline

The compiler has 6 phases:

```
Source (.ais) → Lexer → Parser (CST) → Lowering (AST) → Semantic Analysis → Codegen (Rust)
```

| Phase | What it does |
|-------|-------------|
| **Lexer** | Tokenizes source into symbols, keywords, strings, integers, durations, regex literals |
| **Parser** | Builds a generic S-expression tree (lists, vectors, maps, atoms) — no semantic knowledge |
| **Lowering** | Converts CST to typed AST (Module, TypeDef, FnDef, Expr, Pattern, etc.) |
| **Semantic analysis** | Name resolution, effect checking, match exhaustiveness |
| **Codegen** | Emits Rust source: structs, traits, enums, functions with doc comments |

## What Gets Generated

Given an AIS module, the compiler produces Rust code with:

| AIS construct | Rust output |
|---------------|-------------|
| `(type User ...)` | `pub struct User` with `validate()` method |
| `(effect-set db-read ...)` | `pub trait DbRead` with typed methods |
| `(fn get-user ...)` | `pub fn get_user<Ctx: DbRead + ...>()` with trait-bounded context |
| `(returns (union ...))` | `pub enum GetUserResult` with `http_status()` and `Display` |
| `:provenance`, `:called-by`, etc. | Doc comments preserving all metadata |
| `:invariants`, `:min-len`, `:max-len` | Validation logic in `validate()` |

## Examples

The `examples/` directory contains several AIS modules:

### `minimal.ais` — Starting point
The smallest valid module. One type, one effect set, one function.

```bash
ais-lang compile examples/minimal.ais -o output/
```

### `user-service.ais` — Canonical example
The reference example from the language spec. A user CRUD service with two functions (`get-user-by-id`, `create-user`), full provenance, effect tracking, and union return types.

```bash
ais-lang compile examples/user-service.ais -o output/
```

### `auth-service.ais` — Authentication
Token-based authentication with session management. Demonstrates multiple effect sets (session reads/writes, user lookup, audit logging), expiration handling, and password verification flows.

```bash
ais-lang compile examples/auth-service.ais -o output/
```

### `inventory.ais` — Inventory management
Stock tracking with reservations. Multiple types (`Product`, `StockEntry`, `Reservation`), cross-type queries, quantity constraints, and write-heavy operations.

```bash
ais-lang compile examples/inventory.ais -o output/
```

### `notification.ais` — Notifications
Multi-channel delivery with template rendering. Shows send effects (`email-gateway`, `sms-gateway`), long latency budgets (2000ms), and chained operations (render → deliver → persist).

```bash
ais-lang compile examples/notification.ais -o output/
```

## Language Reference

### Module

Every AIS file contains a single module:

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

See [LANGUAGE.pt-BR.md](../LANGUAGE.pt-BR.md) for the full design document. The core insight:

> The language ideal for AI is the one humans keep rejecting.

Every feature that helps an LLM reason about code — formal specs, effect tracking, totality checking, rich AST manipulation, exhaustive returns — adds cognitive load for humans. AIS embraces that trade-off: it's a language where the metadata-to-logic ratio is 3:1, because knowing *why*, *what's affected*, and *what guarantees must hold* is where AI agents spend their reasoning budget.

## Project Structure

```
ais-lang/
├── Cargo.toml
├── grammar.ebnf                  # Formal EBNF grammar
├── src/
│   ├── main.rs                   # CLI entry point
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
│   └── codegen/
│       ├── mod.rs
│       └── rust.rs               # Rust code emission (6 tests)
└── examples/
    ├── minimal.ais               # Smallest valid module
    ├── user-service.ais          # Canonical example from the spec
    ├── auth-service.ais          # Authentication & sessions
    ├── inventory.ais             # Stock management & reservations
    └── notification.ais          # Multi-channel notifications
```

## Tests

```bash
cargo test
```

39 tests across all phases: lexer (16), parser (8), lowering (5), semantic analysis (4), codegen (6).
