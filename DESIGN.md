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

| Field         | Application Type | PostgreSQL Type     | Notes                                            |
| ------------- | ---------------- | ------------------- | ------------------------------------------------ |
| `id`          | UUID             | `UUID`              | Primary key                                      |
| `business_id` | UUID             | `UUID`              | FK → Business                                    |
| `customer_id` | UUID             | `UUID`              | FK → Customer                                    |
| `total_cents` | i64              | `BIGINT`            | Server-computed; USD                             |
| `state`       | enum             | `enum InvoiceState` | `draft`, `open`, `paid`, `void`, `uncollectible` |
| `due_date`    | date             | `DATE`              | Invoice due date                                 |
| `created_at`  | timestamp        | `TIMESTAMPTZ`       | Server-generated                                 |
| `updated_at`  | timestamp        | `TIMESTAMPTZ`       | Server-managed                                   |

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
| --------------------- | ------------------ | --------------- | -------------------------------- |
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
- `paid`
- `void`
- `uncollectible`

with this table explaining what each state represents

Suggested semantics:

| State           | Meaning                                                           | Terminal? |
| --------------- | ----------------------------------------------------------------- | --------: |
| `draft`         | Invoice has been created but is not yet available for collection. |        No |
| `open`          | Invoice is collectible and may be paid.                           |        No |
| `paid`          | Invoice has been successfully paid.                               |       Yes |
| `void`          | Invoice has been intentionally invalidated.                       |       Yes |
| `uncollectible` | Invoice is considered no longer collectible.                      |       Yes |

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
       success|   |  |mark uncollectible
              |   |  v
              |   |+----------------+
              |   +>| uncollectible |
              |     +----------------+
              v
           +--------+
           |  paid  |
           +--------+
```

## 3. Payment Correctness and Failure Modes

## 4. Webhook Design

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

1. Zero downtime key rotation - would need

## 7. Production Readiness gap

The document mentioned common points such as Observability, rate limiting, audit logs, refunds, dunning. Here are some production readiness gap points not from the ones listed:

1. As discussed in API key model, for sake of time, each API key assumes all responsibilites. Ideally it would be better to have API keys be granular in terms of permissions assigned to them.
2. `A Customer can belong to only one business`. While useful for cutting down scope of the exercise, in production most likely we will have customers that use multiple businesses powered by our payment service, that violates a fundamental data assumption and hence

## 8. Other mentioned things

1. Rate limiting ?
