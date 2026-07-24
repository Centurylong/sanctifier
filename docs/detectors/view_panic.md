# view_panic

* **Finding Code:** `SANCT_VIEW_PANIC`
* **Category:** Panic Handling
* **Severity:** Warning

## Description

Detects panic, unwrap, or expect calls inside read-only or view functions that could cause unexpected transaction aborts.

## Remediation

Return Option or Result types instead of panicking in view/getter functions.
