# RelayKey API

This document describes the public-facing API surface of RelayKey.

RelayKey exposes two planes:

- **Data plane** – proxying and enforcement (`/proxy/*`)
- **Control plane** – configuration and reporting (`/admin/*`)

---

## Data plane (proxy)

All partner traffic is routed through RelayKey.

### Endpoint

```

/proxy/{partner}/{*path}

```

Example:

```

/proxy/sumsub/api/v1/applicants

```

RelayKey forwards the request to the configured partner base URL and path.

---

### Authentication

Clients authenticate using a virtual API key:

```

X-RelayKey: vk_...

```

The virtual key determines:

- the policy
- the binding to upstream credentials
- rate limits and budgets
- optional payment requirements

---

### Supported HTTP behavior

RelayKey forwards:

- HTTP method
- path and query string
- request body
- selected headers

RelayKey strips hop-by-hop headers and injects the upstream credential before forwarding.

---

### Responses

RelayKey returns the upstream response body and status code by default.

RelayKey may return its own errors before forwarding.

---

### Common RelayKey responses

#### 401 – Unauthorized

The virtual key is missing, invalid, or disabled.

#### 403 – Forbidden

The request is blocked by policy (endpoint or environment restrictions).

#### 429 – Too Many Requests

The request was blocked by:

- rate limit
- monthly usage budget

The response includes a machine-readable error code.

#### 402 – Payment Required (optional)

Returned only when the policy requires x402 payment enforcement.

The response includes a payment challenge payload.

---

### RelayKey headers

RelayKey may attach diagnostic headers:

```

X-Relay-Request-Id
X-Relay-Blocked-Reason (only on blocked requests)

```

---

## Control plane (admin)

Administrative endpoints are exposed under:

```

/admin/*

```

All admin endpoints require an administrative access token.

---

### Partners

```

POST   /admin/partners
GET    /admin/partners

```

Partners represent third-party providers (e.g. KYC, custody, pricing APIs).

---

### Upstream credentials

```

POST   /admin/upstream-credentials
GET    /admin/upstream-credentials
POST   /admin/upstream-credentials/{id}/disable

```

Upstream credentials reference secrets stored in an external secrets provider or encrypted storage.

---

### Bindings

```

POST   /admin/bindings
PUT    /admin/bindings/{id}
GET    /admin/bindings

```

Bindings define how one or more upstream credentials are selected for a partner.

---

### Policies

```

POST   /admin/policies
GET    /admin/policies

```

Policies define:

- rate limits
- monthly quotas
- endpoint allow/deny lists
- retry and timeout behavior
- billing mode (free / subscription / x402)

---

### Virtual keys

```

POST   /admin/virtual-keys
GET    /admin/virtual-keys
POST   /admin/virtual-keys/{id}/rotate
POST   /admin/virtual-keys/{id}/disable

```

The raw virtual key is returned only once at creation or rotation time.

---

### Usage and metrics

```

GET /admin/usage
GET /admin/errors

```

These endpoints return aggregated usage and error metrics per workspace, partner, and virtual key.

---

## Versioning and stability

The API is evolving.

Breaking changes may occur during early development.  
All APIs are considered unstable unless explicitly documented as stable.
