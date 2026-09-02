# Third-party notices

Lirvena uses third-party crates listed by Cargo metadata and audited by
`cargo-deny`. The following source-derived implementation also requires an
explicit notice.

## QQ TEA envelope codec

`crates/qq-envelope/src/tea` is a Rust adaptation of the QQ TEA envelope
algorithm published by the Lagrange Core project in
`Utility/Crypto/Provider/Tea/TeaProvider.cs`.

- Upstream: <https://github.com/LagrangeDev/Lagrange.Core>
- License: GPL-3.0-only
- Local modifications: bounded allocation, typed zeroizing key, injected
  deterministic padding for tests, strict ciphertext validation and focused
  modules.

Lirvena distributes the adaptation under AGPL-3.0-only, which retains the
GPLv3 terms and adds the AGPL network-source condition for the combined work.

## QQ message and directory protobuf contracts

The field-level interoperability contracts in `crates/qq-message/src/outbound.rs`
and `crates/qq-directory/src/friend.rs` were independently transcribed to Rust
from the public Lagrange Core packet definitions and verified with local golden
vectors. No Lagrange runtime architecture or reflection machinery is embedded.

- Upstream: <https://github.com/LagrangeDev/Lagrange.Core>
- License: GPL-3.0-only
- Local modifications: explicit allocation limits, strict validation, typed
  inputs, pagination bounds, duplicate rejection and transport-independent APIs.

The Linux login backend delegates secp192k1 point validation, ephemeral-key
generation and ECDH to OpenSSL. MD5 is used only as the upstream QQ profile's
fixed shared-value derivation step; it is not used as a general integrity or
password primitive.
