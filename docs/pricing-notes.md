# RelayKey – Pricing Notes

This document captures internal pricing assumptions and packaging guidance for RelayKey.
It is not customer-facing and is intended to evolve as early customers are onboarded.

RelayKey is positioned as a **governance and control plane for third-party APIs**, with an optional monetization module layered on top.

---

## Core pricing philosophy

RelayKey should be priced around:

- operational risk reduction (quota protection, isolation, auditability)
- engineering time saved
- cost control for paid APIs

This is not a developer tool priced per seat.
It is infrastructure and should be priced as such.

The primary buyer is:

- platform engineering
- infrastructure
- security / compliance
- backend leadership

---

## Packaging overview

RelayKey should be sold as:

1. a base platform subscription
2. usage-based proxy traffic
3. optional enterprise capabilities
4. optional monetization module (x402)

---

## Base platform subscription

The base subscription covers:

- workspaces and access control
- virtual API keys (sub-keys)
- policies
- bindings
- upstream credential management
- rate limits and monthly budgets
- isolation and attribution
- partner-aware retry and reliability logic
- usage and error dashboards

This should be a fixed monthly fee per workspace.

**Initial guidance**

- Small / early-stage teams: low hundreds per month
- Mid-sized teams: mid hundreds per month
- Enterprise: negotiated

The base plan should not be tied to the number of users or seats.

---

## Usage-based pricing

Usage pricing should be based on:

- proxied requests

Count a request when RelayKey forwards it to the upstream provider.

Do not charge for:

- requests blocked by policy
- requests blocked by rate limits
- requests blocked by quota
- internal retries

This aligns pricing with actual value and avoids penalizing customers for protection logic.

---

## Why usage-based pricing makes sense

RelayKey sits on the critical path for:

- partner API access
- high-volume batch workloads
- automated pipelines

Usage-based pricing:

- aligns cost with traffic
- scales naturally as customers grow
- is easy for customers to model against existing vendor spend

---

## Recommended usage tiers (illustrative)

This should be tuned with early customers, but a reasonable starting structure:

- base plan includes a fixed monthly allowance of proxied requests
- additional usage is billed in blocks (for example per 100k or 1M requests)

Do not publish very fine-grained pricing early.
Keep tiers simple.

---

## Enterprise tier

The enterprise tier should include:

- private networking / private connectivity
- customer-managed encryption keys
- extended audit and usage retention
- compliance-oriented exports
- custom partner adapters
- higher support and SLA commitments

This tier should be contract-based.

---

## Optional monetization module (x402)

The x402 layer is a separate paid module.

It enables customers to:

- expose APIs as paid endpoints
- resell third-party APIs
- recover internal costs between teams

This module should be priced independently of core governance.

Recommended pricing approaches:

- a fixed monthly add-on fee
- plus a small percentage or flat fee per paid request processed through the x402 flow

The x402 module should not be bundled into the base plan.

This keeps RelayKey positioned primarily as an infrastructure governance platform, not a billing product.

---

## Cost attribution and forecasting as premium features

Advanced features such as:

- cost attribution by virtual key, service, or customer
- projected quota exhaustion
- projected API spend

should be packaged as higher-tier features, not part of the entry plan.

These features are most valuable to:

- finance teams
- operations teams
- platform leadership

---

## Free tier guidance

A limited free or internal-only tier may be useful for:

- development
- early evaluation
- proof of concept

The free tier should be:

- single workspace
- low usage cap
- limited retention
- no enterprise features

The goal is onboarding, not long-term free usage.

---

## What RelayKey should NOT charge for

RelayKey should not charge based on:

- number of virtual keys
- number of environments
- number of partners
- number of policies
- number of users

These are core control-plane concepts and should not become pricing friction.

---

## Competitive positioning notes

RelayKey pricing should be framed against:

- the cost of quota exhaustion at critical vendors
- the engineering cost of building and maintaining internal governance layers
- the operational cost of outages caused by partner API failures

It should not be framed against generic API gateway pricing.

---

## Early customer discovery targets

For pricing validation, focus on teams that:

- already pay significant monthly fees to KYC, compliance, or data vendors
- operate multiple environments and pipelines
- have experienced at least one quota or rate-limit incident

These customers will best validate willingness to pay.

---

## Internal success metrics for pricing

Early pricing should be evaluated against:

- time-to-close for first 3–5 customers
- ease of explaining the value in a single conversation
- alignment with customers’ existing API vendor spend models
- minimal procurement and legal friction

If pricing requires extensive explanation, it is too complex.
