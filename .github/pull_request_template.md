## What and why

<!-- What this changes, and the problem it solves. Link the issue: "Fixes #123". -->

## How it was checked

<!-- Tests added or changed; what you ran and saw. For audio code: the null test or
     render it keeps identical, and any measurement (paired A/B runs, which machine). -->

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets` is clean and `cargo fmt --all` changes nothing
- [ ] New interface strings are in both `en.txt` and `es.txt` (or the area's `.en`/`.es` files)
- [ ] A change to what a project saves bumps the schema, with a migration and a fixture
- [ ] A new or updated crate: `cargo run -p xtask -- notices` was run and `THIRD-PARTY-NOTICES.md` committed
- [ ] Screenshots for a visible change (light and dark)

## Notes for the reviewer

<!-- Anything risky, left for later, or decided on the way (an ADR if it is a decision). -->

## Licence

Your contribution is licensed as the crates you change are (GPL-3.0-or-later for the
application, MPL-2.0 for the engine crates — `LICENSE`, `docs/adr/0047-licensing-revised.md`).

- [ ] I agree to the [Cantus Contributor Licence Agreement 1.0](../CLA.md)
