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
- `DESIGN.md` generation (structure left open, not required as part of this spec).

# Service Topology

Three runtime components, each as a container under Docker Compose:

1. invoice-service — the main REST API (Axum). Owns all business logic: auth, customers, invoices, payment attempts, state machine, webhook dispatch.
2. mock-psp — a separate Rust binary exposing an HTTP endpoint that simulates a payment service provider, per the token-behavior table defined in section later. It is a distinct process from invoice-service, not a route prefix on the same binary.
3. postgres — PostgreSQL instance.

invoice-service calls mock-psp over HTTP as a genuine external dependency (real network hop, real timeout/error handling required) — not as an in-process function call like a monolith.

# Models

Only specifying the domain models without concrete fields here. Fields are in Design.md file.

Textual entity/relationship summary (AI generated ER diagram linked later):

- **Business**
  - Has many Customers.
  - Has many Invoices (denormalized via Customer, but ownership for
    authorization purposes is at the Business level as a Business only ever sees its own Customers and Invoices).
  - Authenticates via one or more API keys.
- **Customer**
  - Belongs to exactly one Business.
  - Fields: name, email (minimum required fields).
- **Invoice**
  - Belongs to exactly one Customer (and consequently, one Business).
  - Has many line items (embedded/child records: description, quantity,
    unit_amount_cents).
  - Has a server-computed total (integer cents).
  - Has a state (depending on payment attempts).
  - Has a due date.
  - Has many Payment Attempts.
- **Payment Attempt**
  - Belongs to exactly one Invoice.
  - Records a single try at paying that invoice: outcome (success,
    failure, or pending), the card token used, PSP reference,
    failure code (if any, for now lets go with simple), and timestamps.
- **API Key**
  - Belongs to exactly one Business.
  - Used to authenticate all API requests as that Business.
- **Webhook Endpoint**
  - Belongs to exactly one Business.
  - A registered URL that receives signed event notifications.
- **Webhook Delivery**
  - used for coordinating webhook delivery jobs

# Implementation Details

## Logging

Use a logger interface that is propogated to all the services - payments, invoices, etc. The actual implementation is a simple text based logger equivalent to log.printf or log.println from go's stdlib log

## Model Shapes

Refer to design.md file

## API Endpoints

### 1 Authentication

- All endpoints (except the mock PSP's own endpoint) require an API key
  scoped to a Business.
- Key storage, hashing, transmission mechanism (e.g. header), and
  revocation approach are implementation decisions — to be defended in
  `DESIGN.md`.

### 2 Customers

- `POST /customers` — create a customer under the authenticated business.
- `GET /customers/{id}` — get a customer (must belong to the authenticated
  business).
- `GET /customers` — list customers scoped to the authenticated business.

### 3 Invoices

- `POST /invoices` — create an invoice with line items
  (`description`, `quantity`, `unit_amount_cents`). Server computes the
  total; any client-supplied total is ignored/rejected.
- `GET /invoices/{id}` — get an invoice by ID (scoped to the authenticated
  business).
- `GET /invoices?state=...` — list invoices, filterable by state.

### 4 Payment Attempts

- `POST /invoices/{id}/pay` — accepts a mock card token, requires an
  `Idempotency-Key` header.
  - Records a payment attempt.
  - Calls the mock PSP.
  - Updates invoice state based on the PSP result.
  - Must be idempotent: repeated calls with the same `Idempotency-Key`
    against the same invoice must not create duplicate payment attempts
    or apply duplicate state transitions.
  - Must not corrupt invoice state if the PSP is slow (`tok_timeout`) or
    fails (`tok_network_error`).

### 5 Webhooks

- Businesses can register endpoint URLs to receive webhook notifications.
- Required events (minimum set): `invoice.created`, `invoice.paid`,
  `invoice.payment_failed`.
- Delivery requirements:
  - **Signed** — receivers must be able to verify authenticity (mechanism
    defined in `DESIGN.md`, e.g. HMAC signature header).
  - **Retried on failure** with a documented backoff strategy
    (documented in `DESIGN.md`).
  - **Non-blocking** — webhook delivery must not block or delay the API
    response to the triggering request (i.e. dispatched asynchronously).

### 6 Mock PSP endpoint

- Exposed by the `mock-psp` service (separate binary), not by
  `invoice-service`.
- Accepts a card token and returns behavior per §6.1.

## Mock PSP

### Token behavior table

| Token                    | Response                                         | Timing                              | Notes for caller                                                                                                                    |
| ------------------------ | ------------------------------------------------ | ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `tok_success`            | `{status: "succeeded", psp_ref: <uuid>}`         | ~100 ms                             | Normal success path.                                                                                                                |
| `tok_insufficient_funds` | `{status: "failed", code: "insufficient_funds"}` | ~100 ms                             | Normal, well-formed failure.                                                                                                        |
| `tok_card_declined`      | `{status: "failed", code: "card_declined"}`      | ~100 ms                             | Normal, well-formed failure.                                                                                                        |
| `tok_timeout`            | Eventually returns success                       | Sleeps 30 seconds before responding | Caller must not hang waiting on this; must have its own timeout/handling strategy shorter than 30s.                                 |
| `tok_network_error`      | HTTP 500, or the connection is dropped entirely  | Immediate/variable                  | Caller must handle both a hard error response and an abrupt connection drop (no assumption that a response body is always present). |

### Implications for `invoice-service`

- The call to `mock-psp` must be made with a client-side timeout well
  under 30 seconds, so `tok_timeout` cannot hang a request or a payment
  attempt indefinitely.
- A payment attempt that times out or hits a network error must not be
  silently treated as success or as failure — it must be recorded as a
  distinct outcome (e.g. `pending`/`unknown`) so invoice state is never
  corrupted by an ambiguous PSP response. Reconciliation strategy (if any)
  for late/ambiguous outcomes is a `DESIGN.md` topic.
- `Idempotency-Key` handling (see §8) must ensure that retried calls to
  `POST /invoices/{id}/pay` — including retries triggered by a client
  after a perceived timeout — do not create duplicate payment attempts or
  double-charge state transitions.

## API Route DTOs

Refer apispec.md

## Config Sourcing

Configuration should be supplied through environment variables, with documented local defaults suitable for Docker Compose.

configuration includes:

```text
DATABASE_URL
PSP_BASE_URL
PSP_TIMEOUT_MS
API_BIND_ADDRESS
WEBHOOK_TIMEOUT_MS
WEBHOOK_MAX_RETRIES
RUST_LOG
```

Secrets such as API-key hashing material or webhook-signing secrets must be a dummy value sourced from .env or environment variables, with environment variables overriding the .env values for these.

Sensitive secrets are

```text
api_key_pepper
webhook_sign_secret
```

## Design.md references for implementations

1. Detailed shapes of the domain entities and reasoning.

2. API-key storage, hashing, transmission, and revocation.

3. The complete invoice state machine, including all valid transitions, triggers, and terminal states.

4. PostgreSQL transaction and concurrency strategy for payments/idempotency.

5. How PSP timeouts and network errors are represented and reconciled.

6. How invoice/payment consistency is handled around external PSP calls.

7. Webhook signing and retry design.

# Tests

These tests must be passed to confidently validate the correctness of business logic

1. One concurrency test that fires N concurrent POST /pay requests for the same invoice and asserts that at most one succeeds, no double-charges occur, and the final state is consistent.
2. One idempotency test that retries the same request with the same key and asserts the same response is returned without a second PSP call.
3. One PSP-failure test that uses tok_timeout or tok_network_error and asserts the invoice is not stuck in a bad state.
