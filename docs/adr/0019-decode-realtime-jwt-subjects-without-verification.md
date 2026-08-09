# ADR-0019: Decode realtime JWT subjects without verification

Status: accepted

## Decision

Decode the realtime token's `sub` with the existing `base64` and `serde_json`
dependencies. Require exactly three non-empty base64url segments and an object
payload with a non-empty string `sub`, but do not verify the signature.

The Supabase auth service issues the token and the Realtime server verifies it
when the channel joins. This client reads `sub` only to construct the mandatory
per-user subscription filter; a malformed token is a hard session error before
the socket opens. The decoded value is therefore not an authentication result.

## Dependency exemption

This is rule 1 exemption 3. [`jsonwebtoken`
11](https://docs.rs/jsonwebtoken/11.0.0/jsonwebtoken/) requires an
`aws_lc_rs`, `rust_crypto`, or application-supplied verification provider.
[`jwt-simple` 0.12](https://docs.rs/jwt-simple/0.12.17/jwt_simple/) brings its
own signing, verification and encryption algorithms. Either would add a second
JWT crypto stack for a call site that deliberately has no verification key.

The two operations needed here already belong to maintained dependencies in
the tree. Adding a verification crate while calling only its dangerous or peek
API would increase the audit surface without strengthening authentication.

## Consequences

The decoder owns only JWT shape and claim-type checks. Authentication stays on
the server, and every decoder rejection continues to surface as
`MissingUserId`. If the client ever obtains signing keys and becomes a token
verifier, this exemption no longer applies and a maintained JWT crate is
required.
