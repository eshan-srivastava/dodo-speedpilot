CREATE TYPE invoice_state_new AS ENUM ('draft', 'open', 'payment_processing', 'paid', 'void', 'uncollectible');
ALTER TABLE invoices ALTER COLUMN state TYPE invoice_state_new USING state::text::invoice_state_new;
DROP TYPE invoice_state;
ALTER TYPE invoice_state_new RENAME TO invoice_state;
