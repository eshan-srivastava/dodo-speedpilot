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

Key strategy: since we are using postgres, UUIDv7 which is unique and time sorted is a good choice. Being sorted reduces page miss on the PK index.

`unit_amount_cents` is trusted as supplied by the authenticated business on create invoice route. this service has no product/pricing catalog and cannot independently verify prices. The server-computed-total guarantee only protects against a client overriding the arithmetic (e.g. submitting mismatched line items and total), not against a business submitting incorrect unit prices for their own invoice, which is a business-side data integrity concern outside this service's scope, although we can make an extension in real production if needed, not that i have seen this in razorpay.

### 100X Scale

1. For invoice_events and payment_events we can keep them as a stream of events and process them asynchronously to keep load manageable
2.

## 2. Invoice State Machine

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
