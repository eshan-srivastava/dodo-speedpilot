atomic invoice creation

```
BEGIN
│
├── verify customer belongs to business
│
├── validate/compute line item totals
│
├── INSERT invoice
│
├── INSERT invoice_line_items
│
└── COMMIT
```

POST /invoices

        Request
           │
           ▼
     ┌─────────────┐
     │ API auth    │
     └──────┬──────┘
            ▼
     ┌─────────────┐
     │ DTO/schema  │
     │ validation  │
     └──────┬──────┘
            ▼
     ┌─────────────┐
     │ Customer DB │◄── customer_id from payload
     │ lookup      │
     └──────┬──────┘
            ▼
     ┌─────────────┐
     │ Domain      │
     │ validation  │
     │ + total     │
     │ calculation │
     └──────┬──────┘
            ▼
     ┌─────────────┐
     │ PostgreSQL  │
     │ transaction │
     │             │
     │ invoice     │
     │ line items  │
     └──────┬──────┘
            ▼
        Response

customerID lookup

```
authenticated API key
        │
        ▼
business_id = 123
        │
        ▼
customer_id from request
        │
        ▼
SELECT customer
WHERE id = customer_id
AND business_id = 123
```
