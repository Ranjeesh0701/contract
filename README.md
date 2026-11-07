# Contract ⚡

> **A cross-repository contract and type-safety platform.**  
> *Think "Prisma for APIs and Distributed Services".*

`Contract` synchronizes types, contracts, and APIs across independent repositories, services, clients, and servers—regardless of programming language or technology stack.

---

## The Problem

In modern architectures (microservices, mobile apps, frontend/backend splits), contracts drift silently:
- A backend engineer alters a field from `Int64` to `String` or removes an enum variant.
- Frontend apps and dependent services fail at runtime in production.
- Teams resort to manual meetings or loose OpenAPI specs with no compile-time guarantees or drift enforcement.

## The Solution

`Contract` provides a declarative schema DSL, canonical intermediate representation (IR), semantic breaking change detection, cryptographic lockfiles, and multi-language client code generation.

```text
[ .contract DSL / OpenAPI / JSON ]
               │
               ▼
      [ Canonical IR (AST) ]
        │         │        │
        ▼         ▼        ▼
  [Semantic]  [Breaking] [Typed SDKs]
 [Validation]  [Diffing] [ TS / Rust ]
```

---

## Key Features

- 📜 **Prisma-Like Schema DSL (`.contract`)**: Human-readable syntax for models, enums, optionality, lists, maps, and services.
- 🔄 **Universal Interoperability**: Ingest OpenAPI 3.0 / Swagger specs or export clean `.contract` schemas with zero configuration.
- 🛡️ **Semantic Breaking-Change Detector**: Automatically classifies changes into `[SAFE]`, `[WARNING]`, or `[BREAKING]` to prevent API regressions in CI/CD.
- 🚀 **Executable Client SDK Generation**: Emits ready-to-use, type-safe TypeScript `fetch` clients and Rust `serde` structs with JSDoc/rustdoc comments.
- 🔒 **Cross-Repo Synchronization & Lockfiles**: Track upstream contract dependencies via `contract.toml`, pin them with cryptographic SHA-256 hashes in `contract.lock`, and detect drift.

---

## Repository Structure

Built as a modular, high-performance Rust virtual workspace:

| Crate | Purpose |
| :--- | :--- |
| **[`crates/ir`](crates/ir)** | Canonical AST, types (`Record`, `EnumType`, `Service`), and two-pass semantic validator. |
| **[`crates/parser`](crates/parser)** | Zero-dependency tokenizer, parser, and schema emitter for `.contract` files. |
| **[`crates/diff`](crates/diff)** | Semantic diffing engine for breaking change detection. |
| **[`crates/codegen`](crates/codegen)** | Multi-language code generator (TypeScript Fetch Client SDK & Rust structs). |
| **[`crates/adapter-openapi`](crates/adapter-openapi)** | OpenAPI 3.0 / Swagger JSON ingestion pipeline. |
| **[`crates/sync`](crates/sync)** | Dependency tracker (`contract.toml`), lockfile manager (`contract.lock`), and drift engine. |
| **[`crates/cli`](crates/cli)** | Unified command-line interface (`contract`). |

---

## Quickstart

### 1. Build & Install CLI

```bash
cargo build --release
# Optionally install the CLI globally:
cargo install --path crates/cli
```

### 2. Define a Contract (`schema.contract`)

```text
contract ECommerceApi {
    version: "1.0.0"
    description: "Online storefront service contract"
}

enum OrderStatus {
    Pending,
    Paid,
    Shipped,
    Delivered,
    Cancelled
}

model Address {
    street: String
    city: String
    country: String
    postalCode: String
}

model User {
    id: Int64
    name: String
    email: String
    shippingAddress: Address?
    tags: String[]
}

model Order {
    id: String
    userId: Int64
    status: OrderStatus
    items: String[]
    totalAmount: Float64
    notes: String?
}

service OrderService {
    getOrder(String): Order
    createOrder(Order): Order
}
```

---

## CLI Usage

### Validate Contracts

Ensures syntax, uniqueness, and referential symbol integrity:
```bash
contract check schema.contract
```
```text
✔ Contract 'ECommerceApi' (v1.0.0) is valid. (4 types, 1 services)
```

### Detect Breaking Changes (`diff`)

Compares two contract versions and flags breaking changes:
```bash
contract diff v1.contract v2.contract
```
```text
Contract Changes (2 total):

  [SAFE]     types.OrderStatus.variants.Refunded -> Added variant 'Refunded' to enum 'OrderStatus'
  [BREAKING] types.User.fields.email             -> Field 'email' was removed from record 'User'

✖ Found 1 breaking change(s)!
```
*(Exits with status code `1` on breaking changes—ideal for GitHub Actions / CI gates).*

### Generate Idiomatic Code

Generate 100% strictly typed TypeScript clients:
```bash
contract generate -i schema.contract -l ts -o src/api/client.ts
```

Or strict Rust structs:
```bash
contract generate -i schema.contract -l rust -o src/api/models.rs
```

#### TypeScript Client Example:
```typescript
import { ECommerceApiClient } from "./api/client";

const api = new ECommerceApiClient("https://api.store.com");

// 100% typed autocompletion:
const order = await api.orderService.getOrder("order-123");
console.log(order.totalAmount);
```

### Import Existing OpenAPI / Swagger

Convert existing OpenAPI 3.0 specs directly into `.contract` DSL:
```bash
contract import swagger.json -o api.contract
```

---

## Cross-Repository Synchronization

In a client or frontend repository, declare your contract dependencies in `contract.toml`:

```toml
[project]
name = "storefront-web"
version = "0.1.0"

[dependencies]
store = { path = "../service-store/schema.contract" }
pets = { path = "../service-pets/openapi.json" }

[[generate]]
contract = "store"
lang = "ts"
out = "src/api/store.ts"
```

### Sync Dependencies
```bash
contract sync
```
- Fetches upstream schemas.
- Cryptographically pins versions in `contract.lock`.
- Prevents breaking changes from being pulled without `--force`.
- Automatically emits freshly generated SDKs.

### Check for Upstream Drift
```bash
contract drift
```
```text
ℹ Dependency 'store' has safe upstream updates. (1 changes)
✔ Dependency 'pets' is up to date.

Run 'contract sync' to update local locks and regenerate clients.
```

---

## Testing

Run all unit and integration tests across the workspace:
```bash
cargo test
```

## License

MIT
