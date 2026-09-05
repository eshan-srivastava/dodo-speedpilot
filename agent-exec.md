coding guidelines:

1. prefer simple readable rust over idiomatic rust
2. prefer declaring deep modules that export minimal, useful surfaces for other modules to use
3. split modules based on feature/domain not function. so core, payments, invoices etc instead of core, handlers, services, models
4. do not write unsafe rust
