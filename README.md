# breadb

## Tests

Add integration tests as `tests/<topic>.rs`. Each `#[test]` function runs as
an independent case, and the root package can import workspace crates through
their package names.

Run the full test suite with `cargo test`, or one test file with
`cargo test --test core`. Tests next to a crate's source can stay in that
crate when they need private implementation details.
