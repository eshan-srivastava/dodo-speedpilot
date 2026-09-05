# Invoice and Payment Service

# Video Demo Link

[Youtube](https://youtu.be/23K-shlLkK8)

Intentionally this file is minimal for the first commit and serve as timestamp for when the exercise began.
This will serve as a proof of time budget being followed.
Spec.md and Design.md being tied to the development will will be filled when first commit starts the timer.

checking commit user is fixed or not

This repository implements the assignment's Axum invoice service and a separate HTTP mock PSP.

## Run

Start PostgreSQL, the PSP, and the API with:

```sh
docker compose up --build
```

The development API key is `dev_key:dev_secret`. For example:

```sh
curl -u dev_key:dev_secret -X POST http://localhost:8080/customers \
  -H 'content-type: application/json' \
  -d '{"name":"Acme","email":"billing@example.test"}'
```

Create an invoice using the returned customer ID:

```sh
curl -u dev_key:dev_secret -X POST http://localhost:8080/invoices \
  -H 'content-type: application/json' \
  -d '{"customer_id":"<customer-id>","due_date":"2026-09-30","line_items":[{"description":"API development","quantity":2,"unit_amount_cents":15000}]}'
```

Pay it through the external mock PSP:

```sh
curl -u dev_key:dev_secret -X POST http://localhost:8080/invoices/<invoice-id>/pay \
  -H 'content-type: application/json' -H 'Idempotency-Key: payment-1' \
  -d '{"card_token":"tok_success"}'
```

`tok_timeout` and transport/HTTP PSP failures produce an `unknown` payment attempt. They do not mark an invoice paid or failed because the source specification does not define reconciliation of an ambiguous external result. The durable delivery table and worker provide non-blocking signed webhook retries with exponential backoff.

## Configuration

Configuration is supplied through environment variables. The main variables are `DATABASE_URL`, `PSP_BASE_URL`, `PSP_TIMEOUT_MS`, `API_BIND_ADDRESS`, `WEBHOOK_TIMEOUT_MS`, `WEBHOOK_MAX_RETRIES`, `API_KEY_PEPPER`, `DEV_API_KEY_ID`, and `DEV_API_KEY_SECRET`.

## Verification

```sh
cargo check
cargo test
```
