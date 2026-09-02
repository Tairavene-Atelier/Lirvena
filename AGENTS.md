# Repository instructions

This is the public Lirvena repository. Ceylith is a separate private repository.

Hard rules:

- Never add private plans, mappings, captures, credentials, exact
  implementations or non-public evidence.
- Use only Lirvena and Ceylith as product names. `寄声脉` is the official
  Chinese translation of Lirvena.
- Keep `CARGO_BUILD_JOBS=2`; do not use high build concurrency.
- Public Rust code forbids unsafe code, `todo!()`, `unimplemented!()`,
  `unwrap()`, `expect()` and production panics.
- Prefer maintained libraries behind narrow local adapters when their license,
  stability and weight fit the requirement.
- Implement shared semantics once. Do not hide distinct behavior behind a
  universal abstraction.
- Keep modules and functions focused; split files before they become difficult
  to review.
- Do not add source, script, dynamic-library, general-bytecode, arbitrary URL
  or remote-plugin execution.
- Run the public repository guard, formatting, strict clippy and workspace tests
  before every commit.
- Use ordinary commits and pushes. Never force-push or rewrite public history.
