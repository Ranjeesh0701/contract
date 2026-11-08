# Contract ⚡

> **A cross-repository contract and type-safety platform.**  
> _Think "Prisma for APIs and Distributed Services"._

`Contract` synchronizes types, contracts, and APIs across independent repositories, services, clients, and servers—regardless of programming language or technology stack.

---

## Table of Contents

- [The Problem & The Solution](#the-problem--the-solution)
- [Built for the AI Era: Multi-Agent Coordination](#built-for-the-ai-era-multi-agent-coordination)
- [Installation](#installation)
- [Quickstart (3 Steps)](#quickstart-3-steps)
- [Schema DSL Reference (`.contract`)](#schema-dsl-reference-contract)
- [Client SDK Generation](#client-sdk-generation)
  - [TypeScript SDK](#typescript-sdk)
  - [Python SDK (Pydantic v2)](#python-sdk-pydantic-v2)
  - [Rust Types (Serde)](#rust-types-serde)
- [Ingesting OpenAPI / Swagger](#ingesting-openapi--swagger)
- [Semantic Breaking-Change Detection (`diff`)](#semantic-breaking-change-detection-diff)
- [Cross-Repo Dependency Sync & Drift](#cross-repo-dependency-sync--drift)
- [CLI Reference](#cli-reference)
- [Repository Architecture](#repository-architecture)
- [Testing](#testing)

---

## The Problem & The Solution

In modern architectures (microservices, frontend/backend splits, mobile apps), API contracts drift silently:

- A backend engineer alters an `Int64` field to `String`, removes a field, or deletes an enum variant.
- Frontend apps and downstream services break in production at runtime.
- Teams rely on manual meetings or loose OpenAPI specs with no compile-time guarantees or drift enforcement.

**Contract** provides:

1. **Declarative Schema DSL (`.contract`)**: Clean, human-readable schemas for models, enums, aliases, and services.
2. **Multi-Language Codegen**: Emits typed TypeScript `fetch` clients, Python Pydantic v2 models & clients, and Rust `serde` structs with native doc comments.
3. **Pluggable Adapters**: Direct ingestion from OpenAPI 3.0 / Swagger JSON with zero configuration.
4. **Semantic Breaking-Change Engine**: Classifies API changes into `[SAFE]` and `[BREAKING]` for CI/CD gates.
5. **Cryptographic Synchronization**: Track upstream contract dependencies via `contract.toml` and pin them with SHA-256 hashes in `contract.lock`.

```text
[ .contract DSL / OpenAPI 3.0 ]
               │
               ▼
       [ Canonical IR (AST) ] ◄───► [ Semantic Validator ]
         │         │        │
         ▼         ▼        ▼
   [Semantic]  [Breaking] [Multi-Language SDKs]
  [Validation]  [Diffing] [ TypeScript / Python / Rust ]
```

---

## Built for the AI Era: Multi-Agent Coordination

In the emerging era of AI-native engineering, multiple autonomous AI agents work concurrently across repositories—for example, a **backend agent** refactoring API endpoints while a **frontend agent** updates the web application.

### The Problem: The "Build, Break & Fix" Token Loop

Without a deterministic contract layer, multi-agent swarms get trapped in wasteful trial-and-error loops:

1. The **Backend Agent** modifies routes or payload types.
2. The **Frontend Agent** attempts to guess the new payload shape from conversational prompts or incomplete code context.
3. Tests break with runtime 500 errors or TypeScript mismatches.
4. Agents burn context window and API tokens repeatedly trying to debug, hallucinate alternate fixes, and re-test.

```text
Traditional AI Flow:
[Backend Agent] ──► Guessing ──► [Build Fails] ──► [Debug] ──► [Fix] ──► [Repeat Loop]
```

### The Solution: Contract as the Deterministic Coordination Protocol

`Contract` acts as the shared, deterministic communication bus for AI coding agents:

```text
Contract-Powered AI Flow:
[Backend Agent] ──► Updates .contract ──► `contract sync` ──► [Frontend Agent]
                           │                                         │
                           ▼                                         ▼
                 Deterministic Ground Truth                Zero Guesswork / 100% Typed
```

- 🤖 **Zero-Hallucination Codegen**: Running `contract sync` deterministically emits fully typed SDKs and Pydantic models. The frontend agent doesn't guess types—it simply consumes the generated SDK guided by the compiler.
- 🎯 **Machine-Readable AST Diagnostics**: Instead of agents reading ambiguous git diffs, `contract diff` outputs structured, machine-actionable diagnostics (`[BREAKING] types.Order.fields.totalAmount: Float64 -> String`).
- ⚡ **Maximum Token Density**: Rather than burning thousands of tokens feeding entire server files into an LLM's context window, a `.contract` schema provides complete semantic clarity in just a few dozen lines.
- 🛡️ **Autonomous Agent Safety Gates**: Agents can run `contract drift` as a pre-flight sanity check before opening a PR, preventing broken changes from ever touching human review or CI.

---

## Installation

### 1. Requirements

- [Rust](https://www.rust-lang.org/) (2024 edition / 1.85+)
- Cargo

### 2. Build from Source

Clone the repository and build the binary:

```bash
git clone https://github.com/your-org/contract.git
cd contract
cargo build --release
```

The compiled binary will be located at `target/release/contract`.

### 3. Install Globally (Optional)

```bash
cargo install --path crates/cli
```

Verify the installation:

```bash
contract --help
```

---

## Quickstart (3 Steps)

### Step 1: Write a Contract (`schema.contract`)

```text
contract ECommerceApi {
    version: "1.0.0"
    description: "Storefront API contract"
}

type OrderId: String

enum OrderStatus {
    Pending,
    Paid,
    Shipped,
    Cancelled
}

model Order {
    id: OrderId
    userId: Int64
    status: OrderStatus
    totalAmount: Float64
    notes: String?
}

service OrderService {
    getOrder(OrderId): Order
    cancelOrder(OrderId): Order
}
```

### Step 2: Validate the Contract

```bash
contract check schema.contract
```

Output:

```text
✔ Contract 'ECommerceApi' (v1.0.0) is valid. (3 types, 1 services)
```

### Step 3: Generate Client Code

Generate TypeScript:

```bash
contract generate -i schema.contract -l ts -o client.ts
```

Generate Python:

```bash
contract generate -i schema.contract -l python -o client.py
```

Generate Rust:

```bash
contract generate -i schema.contract -l rust -o models.rs
```

---

## Schema DSL Reference (`.contract`)

### 1. Header Metadata

Every `.contract` file starts with a `contract` block:

```text
contract PaymentApi {
    version: "1.2.0"
    description: "Handles payment processing and refunds"
}
```

### 2. Supported Types

| Type         | Description             | TypeScript               | Python        | Rust            |
| :----------- | :---------------------- | :----------------------- | :------------ | :-------------- |
| `String`     | UTF-8 text              | `string`                 | `str`         | `String`        |
| `Int32`      | 32-bit signed integer   | `number`                 | `int`         | `i32`           |
| `Int64`      | 64-bit signed integer   | `number`                 | `int`         | `i64`           |
| `Float64`    | 64-bit floating point   | `number`                 | `float`       | `f64`           |
| `Bool`       | Boolean flag            | `boolean`                | `bool`        | `bool`          |
| `Bytes`      | Binary byte array       | `Uint8Array`             | `bytes`       | `Vec<u8>`       |
| `T?`         | Optional / nullable     | `T \| null \| undefined` | `Optional[T]` | `Option<T>`     |
| `T[]`        | List / array            | `T[]`                    | `list[T]`     | `Vec<T>`        |
| `Map<K, V>`  | Key-value map           | `Record<K, V>`           | `dict[K, V]`  | `HashMap<K, V>` |
| `Identifier` | Reference to model/enum | Named Type               | Named Type    | Named Type      |

### 3. Type Aliases

```text
type UserId: Int64
type Headers: Map<String, String>
```

### 4. Enums

```text
enum Priority {
    Low,
    Medium,
    High
}
```

### 5. Models (Records)

```text
model Customer {
    id: UserId
    email: String
    tags: String[]
    address: Address?
}
```

### 6. Services & Operations

```text
service CustomerService {
    getCustomer(UserId): Customer
    listCustomers(): Customer[]
}
```

---

## Client SDK Generation

`Contract` generates clean, idiomatic code with zero boilerplate.

### TypeScript SDK

Generates strict interfaces, string union types, typed service interfaces, and executable `fetch` client classes:

```bash
contract generate -i schema.contract -l ts -o src/api/client.ts
```

**Using the generated TypeScript client:**

```typescript
import { ECommerceApiClient } from "./src/api/client";

// Instantiate client with baseUrl and optional custom fetchFn (for auth/middleware)
const api = new ECommerceApiClient("https://api.store.com");

// Fully typed call:
const order = await api.orderService.getOrder("order_123");
console.log(order.id, order.totalAmount, order.status);
```

### Python SDK (Pydantic v2)

Generates Pydantic v2 `BaseModel`s, `str` Enums, Abstract Base Classes (`ABC`) for backend servers, and typed HTTP clients:

```bash
contract generate -i schema.contract -l python -o src/api/client.py
```

**Using the generated Python client:**

```python
from src.api.client import ECommerceApiClient

client = ECommerceApiClient("https://api.store.com")

# Fully typed Pydantic models & autocompletion:
order = client.order_service.get_order("order_123")
print(order.id, order.total_amount, order.status)
```

> [!TIP]
> **Minimal Imports**: The Python generator inspects your contract and emits only the imports that are strictly required (`urllib`, `abc`, `Enum`, `Optional`, and `Field` are omitted when not used), ensuring 100% compliance with `ruff` and `flake8` linters.

### Rust Types (Serde)

Generates Rust structs with automatic camelCase $\to$ `snake_case` serde renames, enums, and rustdoc comments:

```bash
contract generate -i schema.contract -l rust -o src/api/models.rs
```

```rust
use src::api::models::{Order, OrderStatus};

let order = Order {
    id: "order_123".to_string(),
    user_id: 42,
    status: OrderStatus::Paid,
    total_amount: 99.50,
    notes: None,
};
```

---

## Ingesting OpenAPI / Swagger

If your backend already exposes OpenAPI 3.0 or Swagger specs (e.g., FastAPI, Spring Boot, Go, NestJS), you can convert it directly to `.contract` DSL:

```bash
contract import swagger.json -o api.contract
```

- Converts `components.schemas` into `model` and `enum` definitions.
- Detects required vs optional fields.
- Converts REST paths into typed RPC `service` operations.

---

## Semantic Breaking-Change Detection (`diff`)

Catch API regressions in CI/CD before they reach production:

```bash
contract diff v1.contract v2.contract
```

Output:

```text
Contract Changes (3 total):

  [SAFE]     types.OrderStatus.variants.Refunded -> Added variant 'Refunded' to enum 'OrderStatus'
  [SAFE]     types.Order.fields.trackingNumber   -> Added optional field 'trackingNumber' to record 'Order'
  [BREAKING] types.Order.fields.totalAmount      -> Field 'totalAmount' changed type from Float64 to String

✖ Found 1 breaking change(s)!
```

### Change Rules

| Change                                    | Classification | Reason                                               |
| :---------------------------------------- | :------------- | :--------------------------------------------------- |
| **Add optional field (`notes: String?`)** | `[SAFE]`       | Existing clients will safely receive or omit it.     |
| **Add required field (`taxId: String`)**  | `[BREAKING]`   | Existing callers will fail when submitting payloads. |
| **Add enum variant**                      | `[SAFE]`       | Extends allowed inputs.                              |
| **Remove enum variant**                   | `[BREAKING]`   | Existing clients sending that variant will fail.     |
| **Remove field or model**                 | `[BREAKING]`   | Callers expecting that data will crash.              |
| **Change type (`Int64` $\to$ `String`)**  | `[BREAKING]`   | Incompatible serialization/deserialization.          |
| **Remove service operation**              | `[BREAKING]`   | Endpoint no longer exists.                           |

> [!NOTE]
> `contract diff` exits with code `1` when breaking changes are detected, making it drop-in ready for GitHub Actions and GitLab CI gates.

---

## Cross-Repo Dependency Sync & Drift

For independent client repositories (e.g. Next.js web app, iOS client, microservice), `Contract` functions like `package.json` and `npm install` for APIs.

### 1. Declare Dependencies in `contract.toml`

Create `contract.toml` in your client repository:

```toml
[project]
name = "storefront-web"
version = "0.1.0"

[dependencies]
store = { path = "../services/store/schema.contract" }
pets = { path = "../services/pets/petstore.json" }

[[generate]]
contract = "store"
lang = "ts"
out = "src/api/store.ts"

[[generate]]
contract = "store"
lang = "python"
out = "src/api/store.py"

[[generate]]
contract = "pets"
lang = "ts"
out = "src/api/pets.ts"
```

### 2. Synchronize Dependencies (`contract sync`)

```bash
contract sync
```

Output:

```text
✔ Successfully synchronized contracts:
  - Locked dependency 'store'
  - Locked dependency 'pets'

✔ Generated client/server files:
  - Emitted 'src/api/store.ts'
  - Emitted 'src/api/store.py'
  - Emitted 'src/api/pets.ts'
```

- Fetches upstream `.contract` or OpenAPI specs.
- Records cryptographic SHA-256 hashes and snapshots in `contract.lock`.
- Generates all target SDK files automatically.
- **Breaking-change gate**: If upstream contains breaking changes, `contract sync` **aborts** and protects your repo. Pass `--force` to override.

### 3. Check for Upstream Drift (`contract drift`)

Run in CI to detect if upstream services have published changes without modifying your local code:

```bash
contract drift
```

Output:

```text
ℹ Dependency 'store' has safe upstream updates. (1 changes)
✔ Dependency 'pets' is up to date.

Run 'contract sync' to update local locks and regenerate clients.
```

---

## CLI Reference

| Command    | Usage                                                          | Description                                                           |
| :--------- | :------------------------------------------------------------- | :-------------------------------------------------------------------- |
| `check`    | `contract check <file>`                                        | Validates schema syntax and referential integrity                     |
| `diff`     | `contract diff <old> <new>`                                    | Compares two versions and flags breaking changes                      |
| `generate` | `contract generate -i <file> -l <ts\|python\|rust> [-o <out>]` | Emits client SDK or model code                                        |
| `import`   | `contract import <file.json> [-o <out.contract>]`              | Ingests OpenAPI 3.0 / Swagger JSON into `.contract` DSL               |
| `sync`     | `contract sync [--force]`                                      | Synchronizes `contract.toml`, updates `contract.lock`, and emits code |
| `drift`    | `contract drift`                                               | Verifies local lockfile matches upstream contracts                    |

---

## Repository Architecture

Built as a modular, high-performance Rust virtual workspace:

| Crate                                    | Purpose                                                                                                                 |
| :--------------------------------------- | :---------------------------------------------------------------------------------------------------------------------- |
| **[`crates/ir`](crates/ir)**             | Canonical AST, types (`Record`, `EnumType`, `Service`), and two-pass semantic validator.                                |
| **[`crates/parser`](crates/parser)**     | Zero-dependency tokenizer, parser, and schema emitter for `.contract` files.                                            |
| **[`crates/diff`](crates/diff)**         | Semantic diffing engine for breaking change detection.                                                                  |
| **[`crates/codegen`](crates/codegen)**   | Multi-language code generator (TypeScript Fetch SDK, Python Pydantic models & clients, Rust structs).                   |
| **[`crates/adapters`](crates/adapters)** | Pluggable schema adapter interface (`SchemaAdapter`, `AdapterRegistry`) supporting OpenAPI 3.0, Swagger, Protobuf, etc. |
| **[`crates/sync`](crates/sync)**         | Dependency tracker (`contract.toml`), lockfile manager (`contract.lock`), and drift engine.                             |
| **[`crates/cli`](crates/cli)**           | Unified command-line interface (`contract`).                                                                            |

---

## Testing

Run all unit and integration test suites across the workspace:

```bash
cargo test
```

---

## License

MIT
