# Spec

Spec file to be used as source of truth for what is being built in the project.
Any explicitly mentioned instructions must not be violated.
Any instruction that appears as vague or warrants confirmation should be flagged when building

# Aim

Build a minimal Invoice & Payment Service.
It models the core of a billing product: a business creates invoices for its customers, customers pay those invoices, and the business is notified of state changes via webhooks

The primary engineering focus is the invoice state machine, payment idempotency, external PSP failure handling, transactional data changes, and webhook delivery model.

# Must Have

- API key authentication, scoped to a business.
- Customer CRUD (create, get, list) scoped to the authenticated business.
- Invoice creation with server-computed totals from line items, get by ID,
  list filterable by state.
- Payment attempts against invoices via a mock PSP, idempotent via
  `Idempotency-Key`.
- Invoice state machine with defined states, documented transitions, and
  API-level rejection of invalid transitions.
- Signed, retried, non-blocking webhook delivery for invoice lifecycle
  events.
- PostgreSQL persistence with migrations.
- Single-command environment bring-up via `docker compose up`.
- README with run instructions and curl examples.
- API documentation (OpenAPI YAML or Markdown) with a consistent error
  format.
- A mock PSP service built to the provided token-behavior spec.

# DO NOT BUILD

- Subscriptions, recurring billing, plans, proration.
- Refunds or partial payments (may be discussed as future work in
  `DESIGN.md`, not implemented).
- Multi-currency or FX.
- Tax calculation.
- A frontend or UI.
- Real email sending (logging "would send email" is acceptable).
- Production-grade rate limiting (discussed, not implemented).
- OAuth or any auth mechanism beyond API keys.
- `AI_USAGE.md` generation.
- `DESIGN.md` generation (structure left open, not required as part of this
  spec).
- ER diagrams as images/graphics — a text representation of entities and
  relationships is sufficient.

# Service Topology

Three runtime components, each as a container under Docker Compose:

1. invoice-service — the main REST API (Axum). Owns all business logic: auth, customers, invoices, payment attempts, state machine, webhook dispatch.
2. mock-psp — a separate Rust binary exposing an HTTP endpoint that simulates a payment service provider, per the token-behavior table defined in section later. It is a distinct process from invoice-service, not a route prefix on the same binary.
3. postgres — PostgreSQL instance.

invoice-service calls mock-psp over HTTP as a genuine external dependency (real network hop, real timeout/error handling required) — not as an in-process function call like a monolith.

# Models

#

# Tests

# Design.md references

1. API-key storage, hashing, transmission, and revocation.

2. The complete invoice state machine, including all valid transitions, triggers, and terminal states.

3. PostgreSQL transaction and concurrency strategy for payments/idempotency.

4. How PSP timeouts and network errors are represented and reconciled.

5. How invoice/payment consistency is handled around external PSP calls.

6. Webhook signing and retry design.

7. How the system would evolve to support refunds/partial payments and other explicitly out-of-scope features.

8. Rate-limiting considerations, without implementing production-grade rate limiting.
