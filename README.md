# RelayKey

**Virtual API keys, quotas, and optional x402 monetization for third-party APIs.**

RelayKey is a governance and control plane for paid and partner APIs.  
It sits between your services and third-party providers (KYC, compliance, custody, pricing, blockchain infra, etc.) and makes it safe to share a single upstream API key across many environments, services, and customers.

RelayKey focuses on **control, isolation, attribution, and cost protection first**, with an optional monetization layer built on top.

---

## What RelayKey does

RelayKey introduces **virtual API keys (sub-keys)** in front of real vendor credentials.

Your real upstream API keys never leave RelayKey.

```

virtual key → policy → upstream API key

````

All traffic is evaluated and governed before it is forwarded to the vendor.

---

## Core capabilities

- **Virtual API keys (sub-keys)**  
  Issue unlimited keys for developers, CI, batch jobs, environments, services, and customer workloads, all backed by the same upstream credential.

- **Per-key rate limits and monthly budgets**  
  Enforce request-rate limits and hard monthly usage caps before requests reach the vendor.

- **Isolation and attribution**  
  Isolate traffic by environment, service, team, or customer and attribute every request to its source.

- **Shared-key multiplexing**  
  Safely serve many end customers through a single upstream API key with per-customer limits and tracking.

- **Partner-aware reliability**  
  Apply vendor-specific retry, backoff, and circuit-breaker policies to reduce outages and retry amplification.

---

## Optional monetization layer (x402)

RelayKey can optionally enforce payment before a request is forwarded using an HTTP 402 (Payment Required) flow.

This enables:

- exposing APIs as paid endpoints
- reselling partner APIs
- internal cost recovery between teams

The upstream provider remains completely unaware of payments.

The monetization layer is isolated from the core governance features and can be enabled per policy.

---

## Why RelayKey is different from an API gateway

Traditional API gateways are designed to protect and publish your own APIs.

RelayKey is built specifically for third-party and partner APIs and is designed around:

- shared upstream credentials
- vendor-defined quotas and paid usage models
- partner-specific failure semantics

RelayKey is a governance and control plane for external APIs.

---

## High-level request flow

1. Authenticate the virtual key (`X-RelayKey`)
2. Load policy and binding configuration
3. Enforce endpoint and environment rules
4. Apply rate limits
5. Apply monthly usage budgets
6. (Optional) enforce x402 payment policy
7. Select an upstream credential
8. Fetch the secret from a configured secrets provider
9. Forward the request with partner-aware retries and timeouts
10. Emit a usage and audit event

---

## Key metrics

- requests per virtual key
- forwarded vs retried requests
- blocked requests (rate limit, budget, policy, payment)
- upstream error rates by partner
- per-key and per-customer cost attribution

---

## Repository structure

This repository is organized as a Rust workspace with a clear separation between the data plane and the control plane.

- `crates/relaykey-app`  
  Main binary, router wiring, configuration, telemetry and lifecycle.

- `crates/relaykey-gateway`  
  Data-plane middleware and proxy logic (authentication, policy guards, rate limiting, quotas, forwarding, retries).

- `crates/relaykey-admin`  
  Control-plane API for managing partners, credentials, bindings, policies and virtual keys.

- `crates/relaykey-payments`  
  Payment providers and x402 enforcement logic (optional module).

- `crates/relaykey-db`  
  SQLx pool, queries and models.

- `crates/relaykey-core`  
  Domain types, policy models, cryptography utilities and shared errors.

- `migrations/`  
  Postgres schema and migrations.

- `docs/`  
  Architecture and operational documentation.

---

## Local development

### Prerequisites

- Rust (stable)
- Docker (for Postgres and Redis)

### Start infrastructure

```bash
docker compose up -d
````

### Configure environment

```bash
cp .env.example .env
```

Edit values as needed.

### Run database migrations

```bash
./scripts/migrate.sh
```

### Run RelayKey

```bash
cargo run -p relaykey-app
```

Health check:

```
GET /health
```

---

## Proxy usage

Partner requests are proxied through:

```
/proxy/{partner}/{*path}
```

Example:

```bash
curl \
  -H "X-RelayKey: vk_..." \
  http://localhost:8080/proxy/sumsub/api/v1/applicants
```

The real upstream credential is injected by RelayKey and is never exposed to clients.

---

## Admin API

Administrative and configuration endpoints are exposed under:

```
/admin/*
```

The control plane manages:

* partners
* upstream credentials
* bindings
* policies
* virtual keys
* usage and error metrics

---

## Security baseline

* Virtual keys are stored only as HMAC hashes. Raw keys are never persisted.
* Upstream credentials are never logged and are injected only at request time.
* Hop-by-hop headers are stripped and sensitive headers are redacted.
* Egress is restricted to configured partner base URLs to prevent SSRF.
* Usage and audit events are immutable and append-only.

---

## Project status

RelayKey is an active, early-stage project under heavy development.

APIs, internal modules, and data models may change rapidly.

---

## Commercial use and redistribution

This project is proprietary.

All rights are reserved by the authors.

Commercial use, redistribution, or deployment of RelayKey (or derivative works) by third parties is not permitted without explicit written permission.
