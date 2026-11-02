# Security Policy

## Supported versions

Security fixes are made for the latest release only. Please update to the newest version from the
[releases page](https://github.com/Namitjain07/Swift-Download-Manager/releases/latest) before reporting.

| Version | Supported |
| --- | --- |
| Latest stable release (1.4.x) | Yes |
| Latest pre-release (1.6.0-beta.x: several networks, web interface, server, Docker) | Yes, and reports are especially welcome |
| Older releases | No |

## Reporting a vulnerability

**Please do not report a vulnerability in a public issue, pull request or discussion.**

Use GitHub's private reporting instead:

1. Open the [**Security** tab](https://github.com/Namitjain07/Swift-Download-Manager/security) of this repository.
2. Choose **Report a vulnerability** and fill in the form.

If that button is not there, open a normal issue that only says you have a security report, with no details,
and the maintainer will arrange a private way to receive it.

Please include:

- what the problem is and what an attacker could do with it
- the version of the app and the system you use (Windows, Linux or Mac, and which version)
- the steps to reproduce it, or a small proof of concept
- whether the browser add-on is involved

You can expect an answer within about a week. This is a small project run by one person, so a fix may take longer;
you will be told what is happening along the way. If you want to be named in the release notes, say so.

## What counts

Reports about these are especially welcome:

- the local listener the browser add-on talks to (`127.0.0.1`, port 9614), for example a web page or another program
  making the app do something the user did not agree to
- links, file names or server answers that make the app write outside the chosen download folder, or run something
- the handling of cookies, logins and proxy settings (for example leaking them to another site)
- the browser add-on's permissions and what it sends to the app
- the MCP control door used by AI assistants (`--mcp`): it listens only on `127.0.0.1` on a random port, needs the
  secret token from `mcp-token` in the settings folder, and must refuse web pages (an `Origin` header, a foreign
  `Host`) and oversized requests; assistants must never be able to delete files, choose a folder or read passwords
- the web interface (`Options > Web interface`, and `--server` with the Docker image): the sign-in and session cookie,
  the per-person separation of downloads and files (nobody may see, change or fetch somebody else's), the
  `Host`/`Origin`/`X-SDM-Request` checks that keep other web pages out, the page's content security policy and anything that lets a
  file name or link put code on the page, and the refusal of links to private and local addresses (a server must not be a way into the network behind
  it; redirects included)
- the installers and the packaged files in a release

Problems that need an attacker who already controls the user's computer, or ordinary bugs without a security effect,
should go through the normal [issue form](https://github.com/Namitjain07/Swift-Download-Manager/issues/new/choose).

## Windows signing and antivirus

Windows releases are signed with a code-signing key that belongs to this project.

- **The private key is never in this repository.** It lives only in the repository's encrypted GitHub Actions secrets
  (`WINDOWS_SIGNING_PFX_BASE64`, `WINDOWS_SIGNING_PFX_PASSWORD`) and in the maintainer's private backup. `.gitignore` refuses
  `*.pfx`, `*.pem`, `*.key` and `*.p12`. If you ever find a private key in this repository, report it as a vulnerability.
- The matching public certificate is `source/packaging/windows/swift-download-manager-codesign.cer`. Its SHA-256 fingerprint is
  `22:B1:AE:1A:D3:BC:C4:0D:F5:3E:81:FD:D1:6A:E1:8E:83:22:2A:FC:51:B9:E6:A1:6A:F5:66:34:AE:F3:86:7A` and it is valid until 2031-11-01.
- It is **self-signed**: it proves the file came from the holder of this key and was not changed, but Windows does not trust the
  issuer, so SmartScreen may still say "unknown publisher". Only a certificate from a public authority (or a service such as
  Azure Trusted Signing) removes that, and a good reputation takes time. Do not install this certificate as a trusted root for
  anything other than checking these files.
- Releases are built by GitHub Actions (`.github/workflows/release.yml`), which checks that the signature matches the committed
  certificate before it publishes anything.
- If the key is lost or may have been exposed, a new key and certificate will be made, the old fingerprint will be marked as
  revoked here, and the next release will say so.
- Windows Defender sometimes flags new, unsigned programs by machine-learning guess (for example `Trojan:Win32/Bearfoos.A!ml`).
  Version 1.2.0 was flagged this way; it was a false positive. See the README for what changed and how to report one.

## Staying safe as a user

- Download the app only from this repository's releases, and compare the file with the `SHA256SUMS` file that comes
  with each release. On Windows, also check the signature (see above).
- The Windows program is signed with the project's own certificate and the Mac app is not signed at all, so the system
  shows a warning on first start. That is expected for this project.
