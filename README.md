# Contract

**Define once. Synchronize everywhere. Zero API drift.**

[![Built with Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Crates.io](https://img.shields.io/crates/v/contract.svg)](https://crates.io/crates/contract)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![GitHub release](https://img.shields.io/github/v/release/Ranjeesh0701/contract?include_prereleases&color=55d6a2)](https://github.com/Ranjeesh0701/contract/releases)

Contract is a high-performance API contract platform and zero-drift synchronization engine built in 100% native Rust. It connects services, frontend apps, and autonomous AI agents through a single, deterministic source of truth—eliminating runtime type errors, broken integrations, and token-wasting AI debugging loops.

> **Project status:** v0.1.1 (Active development).

---

## What Contract Solves

In modern architectures (microservices, mobile apps, frontend/backend splits) and multi-agent AI environments, API schemas drift silently:

* 💥 A backend engineer or AI agent changes a field type, renames a property, or drops an enum variant.
* 💔 Downstream apps and services break in production at runtime.
* 🔄 Autonomous AI agents get trapped in expensive trial-and-error loops—guessing payload shapes, breaking builds, and burning context tokens attempting to self-repair.

### Contract provides:

1. **Declarative Schema DSL (`.contract`)**: Clean, human- and machine-readable schema language for records, enums, type aliases, and RPC services with zero boilerplate.
2. **Instant Polyglot Codegen**: Compiles schemas directly into idiomatic **TypeScript** (Fetch SDK & types), **Python** (Pydantic v2 models & client), **Go** (structs & `net/http` client), and **Rust** (`serde` types).
3. **Semantic Breaking-Change Detector (`contract diff`)**: Inspects schemas AST-to-AST and classifies changes into `[SAFE]` and `[BREAKING]` for CI/CD gates.
4. **Cryptographic Synchronization (`contract sync`)**: Track upstream service contracts via `contract.toml` and lock their exact SHA-256 hashes in `contract.lock`.
5. **Universal Schema Adapters**: Ingests and converts **OpenAPI 3.0 / Swagger**, **Protocol Buffers (`.proto`)**, **JSON Schema**, and **GraphQL SDL (`.graphql`)** without third-party tools.
6. **AI-Agent Deterministic Protocol**: Acts as the shared contract layer for AI coding swarms (Cursor, Claude, Codex, Gemini), replacing payload guesswork with compile-time certainty.

---

## Installation

> [!NOTE]
> **No dependencies required.** `contract` ships as a standalone, zero-dependency native binary. You do **not** need Rust, Node.js, or any runtime installed to run the CLI.

### Option 1: Quick Install (Recommended)

**macOS & Linux**:
```bash
curl -fsSL https://raw.githubusercontent.com/Ranjeesh0701/contract/main/scripts/install.sh | sh
```

**Windows (PowerShell)**:
```powershell
irm https://raw.githubusercontent.com/Ranjeesh0701/contract/main/scripts/install.ps1 | iex
```

### Option 2: Cargo

```bash
cargo install contract
```

### Option 3: Pre-built Binaries (GitHub Releases)

Download pre-compiled binaries for your platform from [GitHub Releases](https://github.com/Ranjeesh0701/contract/releases):

* **Linux x64**: `contract-linux-x64.tar.gz`
* **macOS Apple Silicon**: `contract-darwin-arm64.tar.gz`
* **macOS Intel**: `contract-darwin-x64.tar.gz`
* **Windows x64**: `contract-windows-x64.zip`

Extract and move the binary to any directory in your `PATH` (e.g., `/usr/local/bin`).

Verify the installation:
```bash
contract --help
```

---

## Quick Start

### 1. Define your Contract (`api.contract`)

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

### 2. Validate & Generate Client SDKs

Validate schema syntax and references:
```bash
contract check api.contract
```

Generate type-safe client libraries with a single command:

```bash
# TypeScript Fetch Client & Types
contract generate -i api.contract -l ts -o client.ts

# Python Pydantic v2 Models & Client
contract generate -i api.contract -l python -o client.py

# Go Structs & net/http Client
contract generate -i api.contract -l go -o client.go

# Rust Serde Types
contract generate -i api.contract -l rust -o types.rs
```

### 3. Catch Breaking Changes in CI

Prevent broken APIs from ever reaching production:

```bash
contract diff v1.contract v2.contract --fail-on-breaking
```

```text
Contract Diff: StorefrontApi -> StorefrontApi
  [BREAKING] types.Order.fields.totalAmount: type changed from Float64 to String
  [BREAKING] types.OrderStatus.variants.Cancelled: variant was removed
  [SAFE]     types.Order.fields.discountCode: optional field was added

Summary: 2 breaking change(s), 1 safe change(s)
Error: Breaking changes detected in contract!
```

---

## Cross-Repository Synchronization

Coordinate contracts across independent service repositories with zero drift:

### 1. Declare dependencies (`contract.toml`)

```toml
[dependencies]
billing = { path = "../services/billing/api.contract" }
auth    = { path = "../services/auth/api.contract" }

[[generate]]
contract = "billing"
lang = "typescript"
output = "src/sdk/billing.ts"

[[generate]]
contract = "auth"
lang = "python"
output = "clients/auth.py"
```

### 2. Sync and lock

```bash
contract sync
```

Validates upstream contracts, detects breaking changes, updates `contract.lock` with cryptographic hashes, and regenerates local client SDKs.

### 3. Verify zero drift in CI

```bash
contract drift
```

Exits with a non-zero code if local SDKs have drifted from upstream definitions.

---

## Ingest Existing Schemas

Already have schemas? Import them into `.contract` format instantly:

```bash
# OpenAPI / Swagger
contract import swagger.json -o api.contract

# Protocol Buffers (.proto)
contract import service.proto -o api.contract

# GraphQL SDL (.graphql)
contract import schema.graphql -o api.contract

# JSON Schema
contract import schema.json -o api.contract
```

---

## CLI Reference

| Command | Usage | Description |
|---|---|---|
| `contract check` | `contract check <file>` | Validate schema syntax, type references, and semantics |
| `contract diff` | `contract diff <old> <new>` | Compare two contract versions and flag breaking changes |
| `contract generate` | `contract generate -i <file> -l <lang> [-o <out>]` | Emit typed SDKs (`ts`, `python`, `go`, `rust`) |
| `contract import` | `contract import <file> [-o <out>]` | Ingest OpenAPI, Protobuf, GraphQL, or JSON Schema |
| `contract sync` | `contract sync [--force]` | Synchronize `contract.toml` dependencies and update lockfile |
| `contract drift` | `contract drift` | Check whether local lockfile matches upstream contracts |

---

## Using as a Rust Library

Embed Contract directly into your custom developer tooling or CI pipelines:

```toml
[dependencies]
contract = "0.1.1"
```

```rust
use contract::parser::parse;
use contract::diff::diff;
use contract::codegen::TypeScriptGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let old_ast = parse("contract Api { model User { id: String } }")?;
    let new_ast = parse("contract Api { model User { id: Int64 } }")?;

    let report = diff(&old_ast, &new_ast);
    if report.has_breaking_changes() {
        eprintln!("Breaking changes detected: {:?}", report.breaking_changes());
    }

    let ts_sdk = TypeScriptGenerator::generate(&new_ast);
    println!("{}", ts_sdk);
    Ok(())
}
```

---

## Contributing

Contributions, issues, and feature requests are welcome.

1. Fork the repository and create a feature branch.
2. Run `cargo test` to ensure all unit and integration tests pass.
3. Open a Pull Request with a clear summary of your changes.

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

**Contract — Define once. Synchronize everywhere. Zero API drift.**
