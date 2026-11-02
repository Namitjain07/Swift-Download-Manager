# Security Policy

## Supported versions

Security fixes are made for the latest release only. Please update to the newest version from the
[releases page](https://github.com/Namitjain07/Swift-Download-Manager/releases/latest) before reporting.

| Version | Supported |
| --- | --- |
| Latest release (1.2.x) | Yes |
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
- the installers and the packaged files in a release

Problems that need an attacker who already controls the user's computer, or ordinary bugs without a security effect,
should go through the normal [issue form](https://github.com/Namitjain07/Swift-Download-Manager/issues/new/choose).

## Staying safe as a user

- Download the app only from this repository's releases, and compare the file with the `SHA256SUMS` file that comes
  with each release.
- The Windows program and the Mac app are not signed with a paid certificate, so the system shows a warning on first
  start. That is expected for this project.
