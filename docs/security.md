# RelayKey Security Model

This document describes the baseline security assumptions, controls, and implementation guidelines for RelayKey.

RelayKey’s primary security goals are:

- **Never expose upstream vendor credentials**
- **Prevent quota/cost abuse** (especially from dev/CI and noisy workloads)
- **Provide strong attribution and immutable auditability**
- **Limit blast radius** across environments, services, and customers
- **Reduce risk from untrusted clients and misconfiguration**

---

## Threat model

RelayKey assumes:

- Clients presenting a virtual key may be compromised or malicious.
- Partner APIs are external and may be unreliable, rate-limited, or return unexpected responses.
- Misconfiguration and accidental misuse (CI backfills, debugging scripts, etc.) are common.
- Network boundaries cannot be trusted by default.

RelayKey is designed to protect upstream keys and enforce policy **before** forwarding traffic.

---

## Authentication and authorization

### Virtual keys (data plane)

- Clients authenticate using `X-RelayKey: vk_...`.
- Virtual keys map to:
  - workspace
  - policy
  - upstream binding
  - environment/tags

**Storage requirement**
- Never store raw virtual keys.
- Store only a keyed hash (HMAC) of the key.

**Recommended approach**
- `key_hash = HMAC-SHA256(server_secret, raw_key)`
- Compare hashes in constant time.
- Rotate server hashing secret only with a migration/dual-verify plan.

### Admin auth (control plane)

- Admin endpoints must use a separate auth mechanism from the data plane:
  - JWTs, OAuth, or long-lived admin tokens
- Admin access must be workspace-scoped (RBAC recommended).

---

## Secret management (upstream credentials)

### Principles

- Upstream credentials **must never be returned** to clients.
- Upstream credentials **must never be logged**.
- Secrets should be fetched at runtime from a dedicated secrets provider whenever possible.

### Storage options

1) **Secret references (preferred)**
- Store only a `secret_ref` (e.g. AWS Secrets Manager ARN/path, Vault path).
- Fetch secret values at request time.

2) **Encrypted storage (fallback)**
- Store encrypted blobs in Postgres using envelope encryption (KMS or equivalent).
- Decrypt only in memory.

### Caching

- If caching secret values, use a **short TTL** (e.g. 30–120 seconds).
- Ensure:
  - cache eviction on credential disable/rotation
  - cache is in-memory only (no disk)
- Offer a “no secret caching” mode for strict customers.

---

## Logging and observability safety

### Redaction rules

- Never log:
  - `X-RelayKey`
  - `Authorization`
  - partner credential headers (e.g. `X-Api-Key`, `X-Client-Secret`)
  - payment proofs or payment challenges containing secrets

### Request/response bodies

- Default: do not log bodies.
- If body logging exists for debugging, it must be:
  - opt-in per environment
  - disabled in production
  - filtered/redacted for known sensitive fields

### Correlation

- Use a generated request id:
  - `X-Relay-Request-Id`
- Logs should include only:
  - workspace id (or stable hash)
  - virtual_key id
  - partner id
  - status/latency/blocked reason

---

## Network and proxy security

### SSRF prevention

RelayKey must not forward traffic to arbitrary hosts.

- Only forward to configured partner base URLs.
- Reject requests if:
  - partner is unknown
  - computed upstream host does not match the partner allowlist
- Do not allow clients to override upstream host via headers.

### Header safety

- Strip hop-by-hop headers (RFC 7230), including:
  - `Connection`
  - `Keep-Alive`
  - `Proxy-Authenticate`
  - `Proxy-Authorization`
  - `TE`
  - `Trailer`
  - `Transfer-Encoding`
  - `Upgrade`
- Consider stripping or rewriting:
  - `Host`
  - `X-Forwarded-*` (set your own consistent values)
- Maintain a safe allowlist of forwarded headers.

### Size and time limits

- Enforce max request body size.
- Enforce max response size (or stream with limits).
- Apply strict upstream timeouts (connect + read + total).

---

## Rate limits, quotas, and abuse prevention

### Rate limiting

- Implement per-virtual-key limits in Redis (token bucket/leaky bucket).
- Optionally support per-route limits for expensive endpoints.

### Monthly quotas

- Track per-virtual-key usage using Redis counters keyed by `{workspace}:{vk}:{YYYYMM}`.
- Prefer counting **forwarded requests** (not blocked) to prevent attackers from “spending” quota without upstream calls.
- Decide whether to count:
  - all forwarded requests, or
  - only successful upstream responses (better UX, but higher vendor cost risk)

### Retry amplification controls

Retries can accidentally multiply vendor usage.

- Enforce a max retry budget per request.
- Enforce per-key retry caps (e.g. “no more than 10% amplification” alarms).
- Do not charge quota for internal retries if the intent is “cost protection”.

---

## Auditability and data retention

### Usage/audit events

- Emit append-only usage events for every request decision:
  - forwarded / blocked
  - blocked reason
  - upstream status
  - latency

### Immutability

- Prefer append-only tables (no updates) for raw events.
- Any corrections should be represented as additional events or rollups.

### Retention

- Define retention by tier:
  - short retention in base plans
  - long retention + exports in enterprise plans

---

## x402 module security (optional)

If x402 is enabled:

- Payment proof verification must be isolated behind a `PaymentProvider` interface.
- Payment challenges must include:
  - a request hash / intent id
  - an expiration time
- Proofs must be:
  - single-use or scoped to a narrow window
  - bound to the request hash (prevents replay across endpoints)

Never log:
- payment proofs
- full payment payloads
- wallet addresses if your customers consider them sensitive (treat as PII-like)

---

## Key rotation procedures

### Virtual keys

- Provide rotation endpoints that:
  - create a new key
  - return it once
  - mark the old key disabled after a grace period (optional)

### Upstream credentials

- Support multiple credentials in a binding for zero-downtime rotation:
  - add new credential
  - shift selection to new credential
  - disable old credential

---

## Recommended baseline checks

- Unit tests for redaction and header stripping
- Integration tests for:
  - SSRF prevention
  - rate limit enforcement
  - quota enforcement
  - key rotation correctness
- Security reviews for:
  - secrets provider integration
  - request forwarding code paths

---

## Non-goals (for MVP)

- Full formal verification of policy correctness
- Advanced DLP or content inspection
- End-to-end encryption through partners (RelayKey must see requests to inject auth)