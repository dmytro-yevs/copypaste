# ADR-0031: Stage Windows file paste-back under an owner-only DACL

Status: accepted, 2026-09-27.

## Decision

Windows file paste-back writes decrypted bytes only beneath a `paste-files`
directory in the app data directory. The directory receives a protected DACL
for the current SID, with object and container inheritance. Each pasted item is
placed in a random `tempfile` child directory, retained for ten minutes, swept
while idle, deleted at startup and removed on backend drop.

`clipboard-win` publishes the resulting single path as `CF_HDROP`. The source
reader admits only local drive paths; UNC and verbatim UNC references are not
captured.

## Why this narrow implementation exists

`tempfile` safely creates unique child directories but does not install an
owner-only Windows DACL or keep retained plaintext on an expiry schedule.
`clipboard-win` owns the file-list clipboard format but intentionally does not
own application storage. No maintained package combines those contracts.

The only direct FFI is Windows' documented SDDL conversion and DACL setter,
using the repository's existing current-SID crate and the same protected,
owner-only SDDL shape as the IPC pipe. It does not parse ACLs or implement a
second security model.
