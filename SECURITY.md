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
  `Host`) and oversized requests; assistants must never be able to delete files, choose a folder or read passwords.
  Also what an assistant can be talked into by a page or a document it read (prompt injection): the access level and the
  site lists must hold whatever the client does; links to private networks, `localhost` and the cloud metadata address
  must be refused, including through redirects and changing DNS answers; the limits on adding and changing; names and
  errors from remote servers reaching an assistant uncleaned; and anything that makes the audit log (`mcp-audit.log`)
  hold a password or an access key
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
  (`SDM_CODESIGN_PFX_BASE64`, `SDM_CODESIGN_PFX_PASSWORD`; the two secrets of the earlier key are still there) and in the maintainer's private backup. `.gitignore` refuses
  `*.pfx`, `*.pem`, `*.key` and `*.p12`. If you ever find a private key in this repository, report it as a vulnerability.
- The **current** certificate (from version 1.9.0, for the programs and the installers) is `source/packaging/windows/swift-download-manager-codesign-2.cer`.
  Its SHA-256 fingerprint is `1F:6A:E5:BB:2F:0F:C8:2C:B5:B6:6F:02:65:96:22:BE:E7:49:4F:35:D8:72:A4:CD:19:5E:94:63:91:B8:A1:5D` and it is valid until 2031-11-07.
- The **earlier** certificate is `source/packaging/windows/swift-download-manager-codesign.cer`; it signed every release up to 1.8.0 and was
  replaced by a new key, not because of any known exposure. Those releases stay valid. Its SHA-256 fingerprint is
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

## Checking the Linux packages and the checksums (GPG)

Every release has a `SHA256SUMS` file that lists all the other files, and `SHA256SUMS.asc`, its signature. The Arch Linux packages also have
a `.sig` file each. They are signed with a GPG key that belongs to this project:

- Fingerprint: `0EBC C36E 6AC4 83A3 7A61  EA5B B217 B8AB B696 672D` (Ed25519, expires 2029-11-06), user id
  `Swift Download Manager releases (Namitjain07)`. The public key is `source/packaging/keys/swift-download-manager-release.gpg.asc` and is attached to
  every release. The private key is only in the repository secrets (`SDM_GPG_PRIVATE_KEY`, `SDM_GPG_PASSPHRASE`) and in the maintainer's private backup.
- Compare the fingerprint above with the one you see (`gpg --show-keys swift-download-manager-release.gpg.asc`) and then:

```bash
gpg --import swift-download-manager-release.gpg.asc
gpg --verify SHA256SUMS.asc SHA256SUMS            # "Good signature" and the fingerprint above
sha256sum -c SHA256SUMS --ignore-missing          # for the files you downloaded
```

- Arch Linux: `pacman -U` checks the `.sig` next to the package when pacman knows the key. To let it: `sudo pacman-key --add swift-download-manager-release.gpg.asc`
  and `sudo pacman-key --lsign-key 0EBCC36E6AC483A37A61EA5BB217B8ABB696672D`, then `sudo pacman -U swift-download-manager-<version>-1-x86_64.pkg.tar.zst`.
- Debian and Ubuntu: the `.deb` is checked through `SHA256SUMS.asc` as above (a `.deb` file carries no signature of its own).
- If the key is lost or may have been exposed, a new key will be made, the old fingerprint will be marked as revoked here, and the next release will say so.

## Staying safe as a user

- Download the app only from this repository's releases, and compare the file with the `SHA256SUMS` file that comes
  with each release. On Windows, also check the signature (see above).
- The Windows program is signed with the project's own certificate and the Mac app is not signed at all, so the system
  shows a warning on first start. That is expected for this project.
