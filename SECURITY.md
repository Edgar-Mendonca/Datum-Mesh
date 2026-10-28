# Security

## Reporting a problem

Please report a security problem privately: use **Security › Report a vulnerability** on this repository (GitHub private advisory), or open an issue that only says you have something to report and we will contact you. Do not post details in a public issue.

## What Datum Mesh does and does not do

- Models are `.dmesh` files (JSON) on your own computer.
- The only network call is the update check and download from this repository's GitHub Releases. Every download is checked against its SHA-256 checksum before it installs.
- The window can only open links to the Datum repositories and site (`open_url` refuses anything else).
- Model files are treated as untrusted: the reader is tested with thousands of damaged files and must never crash the app; a damaged file opens as far as possible and lists what it left out.
- The installer is not code-signed yet (planned for 1.0), so Windows shows a SmartScreen warning on the first install.
