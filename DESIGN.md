# Design Doc

Without further ado lets directly addressed what is needed straight from the word doc.

> ![important]
> Disclaimer: I have typed everything on my own for design doc and only used AI to generate mermaid diagrams, table formatting and prettification of the content here. Previous commits can be checked to confirm the same (though now I think how can that be used to check?)

## 1. Data Model

Fully normalized tables

```
businesses
api_keys
customers
invoices
invoice_line_items
payment_attempts
webhook_endpoints
webhook_deliveries
```

**Key strategy**: since we are using postgres, UUIDv7 which is unique and time sorted is a good choice. Being sorted reduces page miss on the PK index.

> All Keys mentioned as UUID as implied to be UUIDv7
> Since postgres has no UUIDv7 generator, the program generates it

### Fields per table

1. Business

| Field        | Application Type | PostgreSQL Type | Notes            |
| ------------ | ---------------- | --------------- | ---------------- |
| `id`         | UUID             | `UUID`          | Primary key      |
| `name`       | string           | `VARCHAR(255)`  | Business name    |
| `created_at` | timestamp        | `TIMESTAMPTZ`   | Server-generated |

2. API Key

| Field          | Postgres Type                      | Notes                                                                                                                   |
| -------------- | ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `id`           | `uuid`                             | Primary key. UUIDv7 generated app-side                                                                                  |
| `business_id`  | `uuid`                             | FK -> Business                                                                                                          |
| `key_id`       | `text` (or `varchar(16)` for now)  | Non-secret public identifier, unique, indexed. Sent by client alongside secret (e.g. Basic Auth username). Safe to log. |
| `secret_hash`  | `text` (or `bytea` see note below) | `HMAC-SHA256(pepper, key_secret)`. Pepper stored outside DB never in this table.                                        |
| `revoked_at`   | `timestamptz`, nullable            | Soft delete. `NULL` = active. Set on rotation cutover or manual revocation.                                             |
| `last_used_at` | `timestamptz`, nullable            | Updated on successful auth. Used to confirm old key is idle before revoking during rotation.                            |
| `created_at`   | `timestamptz`                      | Server-generated, default `now()`                                                                                       |

> [!important]
> `pepper` string is stored outside DB in secure storage. for demo purposes i am fetching it from a local .env or environment variables

> [!note]
> secret_hash as text vs bytea: text storing hex or base64-encoded output is simpler to work with (in debug tools) at the cost of ~33-100% more bytes than raw bytea. For a demo, `text` is the pragmatic choice, `bytea` is the "correct" choice if this were a real high-scale system.

3. Customer

| Field         | Application Type | PostgreSQL Type | Notes            |
| ------------- | ---------------- | --------------- | ---------------- |
| `id`          | UUID             | `UUID`          | Primary key      |
| `business_id` | UUID             | `UUID`          | FK → Business    |
| `name`        | string           | `VARCHAR(255)`  | Customer name    |
| `email`       | string           | `VARCHAR(320)`  | Customer email   |
| `created_at`  | timestamp        | `TIMESTAMPTZ`   | Server-generated |

4. Invoice
   A bill issued by a business to a customer.

| Field         | Application Type | PostgreSQL Type     | Notes                                                                  |
| ------------- | ---------------- | ------------------- | ---------------------------------------------------------------------- |
| `id`          | UUID             | `UUID`              | Primary key                                                            |
| `business_id` | UUID             | `UUID`              | FK → Business                                                          |
| `customer_id` | UUID             | `UUID`              | FK → Customer                                                          |
| `total_cents` | i64              | `BIGINT`            | Server-computed; USD                                                   |
| `state`       | enum             | `enum InvoiceState` | `draft`, `open`, `payment_processing`, `paid`, `void`, `uncollectible` |
| `due_date`    | date             | `DATE`              | Invoice due date                                                       |
| `created_at`  | timestamp        | `TIMESTAMPTZ`       | Server-generated                                                       |
| `updated_at`  | timestamp        | `TIMESTAMPTZ`       | Server-managed                                                         |

5. Invoice Line Item

| Field               | Application Type | PostgreSQL Type | Notes                        |
| ------------------- | ---------------- | --------------- | ---------------------------- |
| `id`                | UUID             | `UUID`          | Primary key                  |
| `invoice_id`        | UUID             | `UUID`          | FK -> Invoice                |
| `description`       | string           | `VARCHAR(500)`  | Item description             |
| `quantity`          | i64              | `BIGINT`        | Must be positive             |
| `unit_amount_cents` | i64              | `BIGINT`        | USD cents; no floating point |
| `created_at`        | timestamp        | `TIMESTAMPTZ`   | Server-generated             |

`Invoice.total_cents` is calculated as:

`sumof(quantity (multiply) unit_amount_cents)`

The client does not supply the invoice total.

6. Payment Attempt
   a single attempt to pay 1 invoice through PSP

| Field           | Application Type | PostgreSQL Type               | Notes                                       |
| --------------- | ---------------- | ----------------------------- | ------------------------------------------- |
| `id`            | UUID             | `UUID`                        | Primary key                                 |
| `invoice_id`    | UUID             | `UUID`                        | FK → Invoice                                |
| `status`        | enum             | `enum payment_attempt_status` | `pending`, `succeeded`, `failed`, `unknown` |
| `card_token`    | string           | `VARCHAR(255)`                | Mock PSP token                              |
| `psp_reference` | UUID nullable    | `UUID`                        | Set on successful PSP response              |
| `failure_code`  | string nullable  | `VARCHAR(64)`                 | PSP failure code                            |
| `created_at`    | timestamp        | `TIMESTAMPTZ`                 | Server-generated                            |
| `updated_at`    | timestamp        | `TIMESTAMPTZ`                 | Server-managed                              |

7. Webhook Endpoint
   used for webhook delivery workers to send webhooks to targets

   | Field            | Type               | Notes                 |
   | ---------------- | ------------------ | --------------------- |
   | `id`             | UUID               | Primary key           |
   | `business_id`    | UUID               | FK -> Business        |
   | `url`            | string             | Delivery endpoint     |
   | `signing_secret` | string             | Used to sign payloads |
   | `created_at`     | timestamp          | Server-generated      |
   | `revoked_at`     | timestamp nullable | Null while active     |

8. Webhook Delivery
   used for checking by jobs to see which webhook still needs delivery

| Field                 | Application Type   | PostgreSQL Type | Notes                            |
| --------------------- | ------------------ | --------------- | -------------------------------- | --- |
| `id`                  | UUID               | `UUID`          | Primary key                      |
| `webhook_endpoint_id` | UUID               | `UUID`          | FK → Webhook Endpoint            |
| `event_type`          | string             | `VARCHAR(64)`   | e.g. `invoice.created`           |
| `payload`             | JSON               | `JSONB`         | Event payload                    |
| `status`              | enum               | `VARCHAR(32)`   | `pending`, `delivered`, `failed` |
| `attempt_count`       | i32                | `INTEGER`       | Number of delivery attempts      |
| `next_attempt_at`     | timestamp nullable | `TIMESTAMPTZ`   | Next retry time                  |
| `last_attempt_at`     | timestamp nullable | `TIMESTAMPTZ`   | Last delivery attempt            |
| `created_at`          | timestamp          | `TIMESTAMPTZ`   | Server-generated                 |
| `delivered_at`        | timestamp nullable | `TIMESTAMPTZ`   | Successful delivery time         |     |

### State Representations

invoice state -> also maps to invoice state machine states

```sql
CREATE TYPE invoice_state AS ENUM (
    'draft',
    'open',
    'payment_processing',
    'paid',
    'void',
    'uncollectible'
);
```

payment attempts

```
payment_attempts
    status: pending|succeeded|failed|unknown
    psp_reference
    failure_code
```

unknown state is useful for the transition
PSP said success -> service crashed before persistence

### Indexes

```
customers(business_id)

invoices(customer_id)
invoices(business_id, state, created_at) //most likely for dashboard by customer support
invoices(business_id, created_at)


payment_attempts(invoice_id)

webhook_endpoints(business_id)

webhook_deliveries(status, next_attempt_at)
```

`unit_amount_cents` is trusted as supplied by the authenticated business on create invoice route. this service has no product/pricing catalog and cannot independently verify prices. The server-computed-total guarantee only protects against a client overriding the arithmetic (e.g. submitting mismatched line items and total), not against a business submitting incorrect unit prices for their own invoice, which is a business-side data integrity concern outside this service's scope, although we can make an extension in real production if needed, not that i have seen this in razorpay.

### 100X Scale

1. For invoice_events and payment_events we can keep them as a stream of events and process them asynchronously to keep load manageable
2. read replicas for list heavy queries on customers, invoices, line items
3. move webhook delivery into infra backed queue
4. partition large tables of payments and webhooks by time. hot data is then only on small parition that can be optimized for performance.
5.

## 2. Invoice State Machine

I listed the postgres enum states in previous section, here is more detail on them

- `draft`
- `open`
- `payment_processing`
- `paid`
- `void`
- `uncollectible`

with this table explaining what each state represents

| State                | Meaning                                                                                                                                                               | Terminal? |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------: |
| `draft`              | Invoice has been created but is not yet available for collection.                                                                                                     |        No |
| `open`               | Invoice is collectible and may be paid. Only state `pay` can start from.                                                                                              |        No |
| `payment_processing` | A payment attempt has claimed the invoice and the PSP call is in flight or the outcome is ambiguous (`unknown`). Non-payable; concurrent `pay` requests are rejected. |        No |
| `paid`               | Invoice has been successfully paid.                                                                                                                                   |       Yes |
| `void`               | Invoice has been intentionally invalidated.                                                                                                                           |       Yes |
| `uncollectible`      | Invoice is considered no longer collectible.                                                                                                                          |       Yes |

ascii diagram generated by AI

```text
                  +--------+
                  | draft  |
                  +--------+
                   |      |
             open  |      | void
                   v      v
              +--------+ +------+
              |  open  | | void |
              +--------+ +------+
               |   |  |
   claim (pay) |   |  |mark uncollectible
               v   |  v
  +-------------------+  +----------------+
  | payment_processing|  | uncollectible |
  +-------------------+  +----------------+
   success|   failure| ambiguous
          |   |      |(stays, needs reconciliation)
          |   |      v
          |   | (stays in payment_processing)
          v   v
       +--------+ +------+
       |  paid  | | open | (revert on well-formed failure)
       +--------+ +------+
```

Valid transitions involving the claim:

| From                 | To                   | Trigger                                             |
| -------------------- | -------------------- | --------------------------------------------------- |
| `open`               | `payment_processing` | `POST /invoices/{id}/pay` wins the atomic claim.    |
| `payment_processing` | `paid`               | PSP returns well-formed `succeeded`.                |
| `payment_processing` | `open`               | PSP returns well-formed `failed` (retryable again). |
| `payment_processing` | (stays)              | PSP outcome ambiguous (`unknown`): no auto-resolve. |
| `open`               | `void`               | Manual void.                                        |
| `open`               | `uncollectible`      | Marked uncollectible.                               |

Any `pay` against a non-`open` state (`payment_processing`, `paid`, `void`, `uncollectible`) is rejected with 409 Conflict. The claim itself is:

```sql
UPDATE invoices
SET state = 'payment_processing', updated_at = now()
WHERE id = $1 AND state = 'open';
```

`rows_affected = 1` wins the race and proceeds to the PSP call; `0` means a concurrent request already claimed/paid it (or it was voided) and the loser is rejected without touching the PSP. This is a single atomic statement: Postgres guarantees only one concurrent transaction flips `open` -> `payment_processing`, no lock is held across the PSP call, and it resolves in microseconds.

## 3. Payment Correctness and Failure Modes

Flow for `POST /invoices/{id}/pay` (implemented in `pay_invoice`):

Pre-check — same `business_id + invoice_id + Idempotency-Key` already stored → replay the stored response (409 if the fingerprint differs). This is outside any write transaction.

Steps 1+2 — one short transaction, no PSP call inside, no lock held across the PSP call. It contains both the atomic claim and the attempt/idempotency reservation, so a same-key loser rolls back cleanly with no orphan row:

Step 1 — claim the invoice (conditional update):

```sql
UPDATE invoices
SET state = 'payment_processing', updated_at = now()
WHERE id = $1 AND business_id = $2 AND state = 'open'
RETURNING total_cents;
```

- Row returned → you won the race, proceed to step 2 in the same transaction.
- No row → invoice wasn't `open` (concurrent claim, already paid, void, etc.). Roll back, re-check idempotency (a same-key winner may have committed after the pre-check), then reject with 409 Conflict (`invoice is not payable in its current state`), no PSP call, no waiting.

Concurrent losers block only for microseconds on the row lock, then re-evaluate the `WHERE` against the new row version and match 0 rows — never blocking on a PSP call.

Step 2 — record the payment attempt row (separate insert, this is where the `(business_id, invoice_id, idempotency_key)` unique constraint resolves same-key races):

```sql
INSERT INTO payment_attempts (id, invoice_id, idempotency_key, card_token, status, created_at)
VALUES (...)
-- status = 'pending' at this point, PSP hasn't responded yet
```

If the idempotency insert hits the `(business_id, invoice_id, idempotency_key)` unique violation, a same-key winner committed between the pre-check and this insert — roll back (undoing this loser's claim and orphan attempt row atomically) and return the winner's stored response. Our attempt never touched the PSP so it cannot double-charge.

Step 3 — call the PSP (now, after the DB work, not holding any lock):

With a client-side timeout well under 30s (`PSP_TIMEOUT_MS`, default 2000ms) so `tok_timeout` doesn't hang the request. Three outcomes: success, well-formed failure (`insufficient_funds`/`card_declined`), ambiguous (timeout/network error).

Step 4 — finalize (second conditional update, using the payment attempt outcome):

```sql
UPDATE invoices
SET state = 'paid', updated_at = now()
WHERE id = $1 AND state = 'payment_processing';
```

or on failure:

```sql
UPDATE invoices
SET state = 'open', updated_at = now()
WHERE id = $1 AND state = 'payment_processing';
```

Also update the `payment_attempts` row status to `succeeded`/`failed`/`unknown` accordingly.

On ambiguous outcome (timeout/network error): set `payment_attempts.status = 'unknown'` and leave the invoice in `payment_processing` rather than reverting to `open` — because the PSP may still succeed asynchronously (that is literally what `tok_timeout` simulates: eventual success after 30s). Reverting to `open` here would let a second request re-attempt payment while the first one might still land — the exact overcharge risk this design prevents. This is the one branch that deliberately doesn't auto-resolve; it needs reconciliation (poll/webhook from PSP, timeout-based recovery, or manual `void`/`uncollectible`), tracked as future work in section 6 rather than silently swept away.

### PSP timeout

Synchronous timeout.

HTTP client has `PSP_TIMEOUT_MS` (default 2000ms), well under the mock PSP's 30s sleep for `tok_timeout`.

On timeout / connection drop / HTTP 500 / malformed body the attempt is recorded as:

```text
payment_attempt = unknown (failure_code = psp_unavailable | psp_http_error | invalid_psp_response)
invoice = payment_processing (left as-is, NOT reverted to open)
```

Rationale: the PSP may still charge the card asynchronously after our timeout fires, so reverting to `open` would allow a retry that double-charges. Invoices stuck in `payment_processing` need reconciliation — a future `GET /payments/{merchant_reference}` poll against the PSP, a PSP webhook, or manual `void`/`uncollectible` — none of which the mock PSP supports, so this is deferred (see section 6). Retry with the same `Idempotency-Key` replays the stored `unknown` response without a second PSP call; retry with a new key is rejected with 409 while the invoice is not `open`.

### PSP succeeds, service crashes before persistence

if

```
1. Create payment attempt
2. Call PSP
3. PSP charges card
4. PSP returns success
5. Service crashes
6. DB update never happens
```

If the PSP supported idempotency we could just use the idempotency header to ensure that PSP does not allow successfuly retry as payment actually processed.

Realistic option for demo is
Assign a merchant transaction ID and later ask from the PSP:

```json
GET /payments/{merchant_reference}
```

skipping this implementation since PSP is a mock one.

### Idempotency key reused with different body

```
same key + same request = return original response / error that already processed (if this is a false retry)
same key + different request = 409 Conflict
```

idempotency key should be scoped to

```json
business_id
idempotency_key
request_hash
response_status
response_body
```

### /pay on a paid invoice

Treat it like 409 Conflict.
Being explicit makes it easier for caller to reason about as it knows the intent: actual new payment or faulty double payment.

## 4. Webhook Design

### Signing:

HMAC-SHA256 on webhook_secret and payload, as is standard with payment services.

For project purposes the hmac runs only on webhook secret and payload, but we can add timestamp to it to stop replay attacks that occur after a fixed interval for production security/

### Retry Numbers

Total Retry Window - 3 hours
with attempts having exponential backoff + random value.
something like
1, 10, 30, 120, 600, 1800, 7200 (an example).
Random addition to exponential backoff prevents potential thundering herd if many payments start failing at same time due to other reasons.

### Webhook Delivery

For demo purposes running a background Tokio task is what I am choosing.
In production we should move to a durable queue backed by infra like SQS etc and have stateless workers consume them.
For a middle ground proof-of-concept i have included webhook deliveries persisting to postgres as way to provide persistence to delivery status.

### Lists

A webhook listing route can be useful for businesses to debug and inspect if needed for integration and debugging purposes.

Otherwise for missed webhook updates, querying `GET /invoice/{id}` also returns latest status.

## 5. API Key Model

Shape: 2 fields, both string. key_id and key_secret.

Generation:

1. Key_id is short 12-16 bytes (I picked 16) that just needs to be purely unique. we store this as plaintext as it being leaked alone is not a problem. We can embed business id as "<env>-<biz_id>-<random>" and then split with biz_id later when looking up in db.
2. Key_secret is long 32-64 bytes (I picked 32 as API keys would ideally be granular not master control), generated cryptographically securely.

Storage:

1. key_id: plaintext storage, and index on this, so incoming key info can be looked up directly
2. key_secret: stored as fast hmac256(pepper, key_secret). incoming is hashed again to compare if key matches.

Transmission: Standard `Authorization: Basic base64(key_id:key_secret)` format. The API header is standard, recognized by most infra and hence better for quick onboarding of the business. While Stripe uses Authorization Header, I have decided to split API key and hence going with Basic username password way of header. Although semantically it is not the best fit since it implies OAuth (and we don't have that yet) but OAuth can always be added later and Stripe already does it. Only if we need to send multiple authorization tokens (like a user + business token) then custom header makes sense. However for invoice and payment service, where current use case is payment creation and capture, Authorization header is better.

Rotation: For exercise purposes the rotation mechanism is simple, click to regenerate new. This wouldn't work in production as businesses wouldn't like downtime, hence a proper in depth solution would be to have 2 keys active simultaneously - similar to AWS access keys. Since we have key id and key secret, only the key secret is regenerated. In production we might consider the option to regenerate api key id too.

Revocation: Soft delete by marking a field `revoked_at` which is a timestamp. This is compared with current time before secret is compared. This will only fail if we decide to cache a key:secret pair's correctness, which I don;t think we should since hashing and lookup are already optimized.

Blast Radius: For the exercise I have made API keys having all permissions. This won't work in production. If we go with granular permission API keys the blast radius is limited to the business whose key is impacted that too only for the functions that the key has. To limit this we can also enable IP whitelisting as done with payment services usually. The split of key_id and key_secret allows us to log key_id in logs directly without PII risk and identify any bugs.

## 6. What I cut and Why

1. Zero downtime key rotation - would need in production, felt trivial but did not implement
2. proper webhook delivery - even though I have worked with similar systems, implementing would take time
3. Code cleanliness - currently all the logic is in app.rs file. This works for speedrunning the project via AI generated code but is not maintainable. Extensive refactoring to clean the code into properly separated modules is tantalizing to me but time constraints do not allow.

## 7. Production Readiness gap

The document mentioned common points such as Observability, rate limiting, audit logs, refunds, dunning. Here are some production readiness gap points not from the ones listed:

1. As discussed in API key model, for sake of time, each API key assumes all responsibilites. Ideally it would be better to have API keys be granular in terms of permissions assigned to them.
2. `A Customer can belong to only one business`. While useful for cutting down scope of the exercise, in production most likely we will have customers that use multiple businesses powered by our payment service, that violates a fundamental data assumption and hence might warrant a lot of changes
3.
