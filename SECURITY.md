# Security

## Reporting a vulnerability

Please report security problems privately through GitHub: open the [Security tab](https://github.com/eshaanjain26/parley/security) of this repository and click "Report a vulnerability". Don't open a public issue for a vulnerability.

Include the Parley version, your macOS version, steps to reproduce and what an attacker could do.

## Supported versions

Only the latest release gets security fixes. Parley checks for updates on launch, so fixes reach users through the normal update.

## What Parley protects

- Meeting content at rest: SQLCipher-encrypted database, key in the macOS Keychain.
- API keys: macOS Keychain only.
- Updates: every update bundle is signed with the project's minisign key and verified before install, including the version it was signed for, so an altered update feed can't push an older or different build.
- The interface's Content Security Policy allows only Parley's own scripts and connections, and AI output is rendered as Markdown sanitized with DOMPurify.

Out of scope: someone with access to your unlocked Mac and login Keychain, and the data handling of third-party providers you choose to use.
