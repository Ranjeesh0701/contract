# Contract ⚡

[![Crates.io](https://img.shields.io/crates/v/contract.svg)](https://crates.io/crates/contract)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)

> **The universal API contract & zero-drift synchronization engine.**  
> _Think Prisma, but for your distributed services, frontend/backend teams, and autonomous AI agents._

`contract` synchronizes API schemas, types, and clients across independent repositories—giving you compile-time safety and zero runtime surprises regardless of your tech stack.

---

## Why Contract?

In modern distributed systems and AI-assisted workflows, APIs drift constantly:

* 💥 A backend engineer or AI agent updates a field name or enum variant.
* 💔 Downstream frontends, mobile apps, and microservices break in production.
* 🔄 Multi-agent swarms burn context and tokens trapped in "guess payload → break build → fix" loops.

**Contract solves this at the root:**

| Feature | What it does | Why it matters |
| :--- | :--- | :--- |
| **Clean DSL (`.contract`)** | Human & AI-readable schema language | Simple, expressive, zero boilerplate |
| **Instant Polyglot Codegen** | Emits TypeScript, Python (Pydantic v2), Go, & Rust | Type-safe clients without manual glue code |
| **Universal Adapters** | Ingests OpenAPI 3.0, Protobuf, GraphQL, & JSON Schema | Works with your existing schemas out of the box |
| **Semantic Diffing** | Flags breaking changes (`[SAFE]` vs `[BREAKING]`) | Non-negotiable safety gate for CI/CD |
| **Zero-Drift Sync** | Tracks upstream contracts via `contract.toml` & `contract.lock` | Cryptographically pins APIs using SHA-256 |
| **AI-Agent Coordination** | Single source of truth across repo boundaries | Multi-agent swarms build features instead of guessing payloads |

---

## Installation

Install directly via Cargo:

```bash
cargo install contract
```

Or build from source:

```bash
git clone https://github.com/Ranjeesh0701/contract.git
cd contract
cargo install --path .
```

Verify the installation:

```bash
contract --help
```

---

## 60-Second Quickstart

### 1. Define your Contract (`schema.contract`)

```text
contract StorefrontApi {
    version: "1.0.0"
}

enum OrderStatus {
    Pending,
    Paid,
    Shipped,
    Cancelled
}

model Order {
    id: String
    userId: Int64
    status: OrderStatus
    totalAmount: Float64
    notes: String?
}

service OrderService {
    getOrder(String): Order
    createOrder(Order): Order
}
```

### 2. Validate Syntactic & Semantic Integrity

```bash
contract check schema.contract
# ✔ Contract 'StorefrontApi' (1.0.0) is valid: 2 types, 1 service, 2 operations.
```

### 3. Generate Type-Safe Clients

Generate SDKs in any supported language with a single command:

```bash
# TypeScript Fetch Client & Types
contract generate -i schema.contract -l ts -o client.ts

# Python Pydantic v2 Models & API Client
contract generate -i schema.contract -l python -o client.py

# Go Structs & net/http Client
contract generate -i schema.contract -l go -o client.go

# Rust Serde Structs
contract generate -i schema.contract -l rust -o types.rs
```

---

## Catch Breaking Changes in CI

Never ship a broken API change again. `contract diff` semantically inspects schemas and flags breaking changes:

```bash
contract diff v1.contract v2.contract
```

```text
Contract Diff: StorefrontApi -> StorefrontApi
  [BREAKING] types.Order.fields.totalAmount: type changed from Float64 to String
  [BREAKING] types.OrderStatus.variants.Cancelled: variant was removed
  [SAFE]     types.Order.fields.discountCode: optional field was added

Summary: 2 breaking change(s), 1 safe change(s)
Error: Breaking changes detected in contract!
```

Add it to your CI pipeline:

```bash
contract diff upstream/main.contract current.contract --fail-on-breaking
```

---

## Built for Multi-Agent AI Development

When multiple AI agents work across different repos (e.g., one on backend, one on frontend):

```text
Without Contract (Wasteful Loop):
[Backend Agent] ──► Changes API ──► [Frontend Agent Guesses] ──► [Build Breaks] ──► [Debug & Fix Loop]

With Contract (Deterministic):
[Backend Agent] ──► Updates .contract ──► `contract sync` ──► [Frontend Agent]
                           │                                         │
                           ▼                                         ▼
                 Single Source of Truth                   100% Typed / Zero Guesswork
```

- **Zero Guesswork**: The frontend agent consumes the automatically generated SDK directly.
- **Token Efficiency**: Agents share concise `.contract` files instead of dumping thousand-line backend implementations into context windows.
- **Machine-Actionable Diff**: Agents receive structured diagnostics (`[BREAKING] field totalAmount: Float64 -> String`) to guide self-healing refactors.

---

## Ingest Existing Schemas

Already have OpenAPI, Protobuf, GraphQL, or JSON Schema? Import them directly into `.contract` DSL:

```bash
# OpenAPI / Swagger
contract import swagger.json -o api.contract

# Protocol Buffers (.proto)
contract import service.proto -o api.contract

# GraphQL SDL
contract import schema.graphql -o api.contract

# JSON Schema
contract import schema.json -o api.contract
```

---

## Cross-Repo Synchronization (`contract.toml`)

Manage external contract dependencies just like Cargo or npm dependencies.

1. Create a `contract.toml` in your project root:

```toml
[dependencies]
billing = { path = "../services/billing/schema.contract" }
users   = { path = "../services/users/schema.contract" }

[[generate]]
contract = "billing"
lang = "typescript"
output = "src/sdk/billing.ts"

[[generate]]
contract = "users"
lang = "python"
output = "clients/users.py"
```

2. Run `contract sync`:

```bash
contract sync
```

This validates upstream contracts, detects breaking changes, updates `contract.lock` with SHA-256 hashes, and regenerates client code.

3. Verify zero drift in CI:

```bash
contract drift
```

---

## CLI Reference

| Command | Usage | Description |
| :--- | :--- | :--- |
| **`check`** | `contract check <file>` | Validates schema syntax and referential integrity |
| **`diff`** | `contract diff <old> <new>` | Detects and flags breaking changes |
| **`generate`** | `contract generate -i <file> -l <ts\|python\|go\|rust> [-o <out>]` | Emits idiomatic client SDKs or model code |
| **`import`** | `contract import <file> [-o <out.contract>]` | Ingests OpenAPI, Protobuf, GraphQL, or JSON Schema |
| **`sync`** | `contract sync [--force]` | Synchronizes `contract.toml` and updates `contract.lock` |
| **`drift`** | `contract drift` | Verifies local lockfile matches upstream contracts |

---

## Using as a Rust Library

Embed `contract` directly into your custom Rust tooling:

```toml
[dependencies]
contract = "0.1.0"
```

```rust
use contract::parser::parse;
use contract::diff::diff;
use contract::codegen::TypeScriptGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let contract_ast = parse("contract App { model User { id: String } }")?;
    let ts_client = TypeScriptGenerator::generate(&contract_ast)?;
    println!("{}", ts_client);
    Ok(())
}
```

---

## License

MIT License — Copyright (c) 2026 Ranjeesh. See [LICENSE](LICENSE) for details.
