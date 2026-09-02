# Contributing

Lirvena accepts changes that can be reviewed using public evidence and public
interfaces. Do not copy private server plans, mappings, captures, credentials or
non-public protocol material into this repository.

Before submitting a change, run:

```text
python tools/public_repo_guard.py
python -m unittest discover -s tools/tests -p "test_*.py"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Keep source files and functions focused. Put shared semantics in one module and
use narrow adapters around dependencies or protocol-specific behavior. New
dependencies require a license, maintenance and API-stability review.
