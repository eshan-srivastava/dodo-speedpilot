coding guidelines:

1. prefer simple readable rust over idiomatic rust
2. prefer declaring deep modules that export minimal, useful surfaces for other modules to use
3. split modules based on feature/domain not function. so core, payments, invoices etc instead of core, handlers, services, models
4. do not write unsafe rust
5. prefer recovery over runtime panic (in golang terms). panic only if truly irrecoverable
6. run `cargo check` to verify compile time
