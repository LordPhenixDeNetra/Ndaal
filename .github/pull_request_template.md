## Task and requirements

<!-- Task Tnn.mm from CAHIER_DES_TACHES.md and the requirements it covers (EF-..., ENF-...). -->

## What changed

## Checks run (CLAUDE.md section 11)

<!-- What you ran and the result, including what failed or could not be run. -->

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `./scripts/check-deny.sh`
- [ ] `./scripts/check-hygiene.sh`
- [ ] `./scripts/check-linux.sh` (required on macOS and Windows)

## Checklist

- [ ] Every commit is signed off (`git commit -s`, DCO).
- [ ] A bug fix comes with a test that failed before the fix.
- [ ] Documentation updated if behavior, formats or APIs changed.
