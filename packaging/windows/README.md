# Windows release setup

The production release workflow signs the Windows installer and submits it to
winget. Both are free for this open-source project. This page lists the one-time
setup; nothing here is needed for local builds.

## Code signing with SignPath Foundation

Without signing, SmartScreen warns about the downloaded installer. SignPath
Foundation issues a free, publicly trusted code-signing certificate to
open-source projects and signs releases from CI.

1. Apply at <https://signpath.org/apply> with the public repository
   `dmytro-yevs/copypaste`. Meet the
   [Foundation terms](https://signpath.org/terms): enable multi-factor
   authentication for every maintainer, and publish a code signing policy page
   that names the team roles, credits SignPath Foundation, and links the privacy
   policy.
2. After approval, in the SignPath project:
   - Connect the GitHub repository as a trusted build system (the
     "GitHub.com" connector).
   - Create two artifact configurations with these slugs and contents:
     - `application`: [signpath/application.xml](signpath/application.xml)
       (CopyPaste's own executables, its bridge library, and the uninstaller
       that the release generates before signing; upstream libraries such as
       the Flutter engine stay unsigned, as the Foundation terms require).
     - `installer`: [signpath/installer.xml](signpath/installer.xml)
   - Note the organization ID, project slug, and release signing policy slug.
     Requests to that policy wait for manual approval in SignPath.
   - Create a CI user with submitter access to the policy and copy its API token.
   - Copy the SHA-1 thumbprint of the policy's signing certificate.
3. In GitHub, open **Settings → Secrets and variables → Actions** and add:

   | Kind | Name | Value |
   | --- | --- | --- |
   | Secret | `SIGNPATH_API_TOKEN` | CI user API token |
   | Variable | `SIGNPATH_ORGANIZATION_ID` | Organization ID |
   | Variable | `SIGNPATH_PROJECT_SLUG` | Project slug |
   | Variable | `SIGNPATH_SIGNING_POLICY_SLUG` | Release signing policy slug |
   | Variable | `SIGNPATH_CERTIFICATE_THUMBPRINT` | Signing certificate SHA-1 thumbprint |

Set all five or none. With none, releases keep signing with the
`WINDOWS_SIGNING_CERTIFICATE_*` PFX secrets. A partial set fails the release
preflight. The release verifies every SignPath signature against the configured
thumbprint, a publicly trusted chain, SHA-256 digests, and an RFC 3161
timestamp, so update `SIGNPATH_CERTIFICATE_THUMBPRINT` when SignPath renews the
certificate.

## winget

Users install with `winget install CopyPaste.CopyPaste`. Packages installed
through winget skip the SmartScreen download prompt.

1. Submit the first version once, after a release is published:

   ```powershell
   winget install Microsoft.WingetCreate
   ```

   ```powershell
   wingetcreate new https://github.com/dmytro-yevs/copypaste/releases/download/v<version>/CopyPaste-v<version>-windows-x86_64-setup.exe
   ```

   Use `CopyPaste.CopyPaste` as the package identifier, the `nullsoft`
   installer type, and `user` scope, then submit the pull request it creates in
   `microsoft/winget-pkgs` and wait for it to merge.
2. Fork `microsoft/winget-pkgs` to the account that owns the token below.
3. Create a classic personal access token with the `public_repo` scope and add
   it as the repository secret `WINGET_TOKEN`.

Each published release then opens a pull request to `microsoft/winget-pkgs`
for the new version. Without `WINGET_TOKEN`, the release skips winget.
