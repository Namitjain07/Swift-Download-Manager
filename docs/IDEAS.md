# Ideas for Swift Download Manager

A brainstorm of what could come next, grouped by what it is for. It is a list of ideas, not promises: the
[roadmap in the README](../README.md#roadmap-and-feature-checklist) says what is finished and what is planned.

**Effort**: S = a day or less, M = a few days, L = a week or more. **Value**: ★ nice, ★★ many people will notice, ★★★ a reason to switch.

## Several networks at once (Wi-Fi + Ethernet + USB)

| Idea | Effort | Value | Why |
| --- | --- | --- | --- |
| Weights and caps per network ("the phone may carry at most 2 connections or 5 GB a day") | M | ★★★ | A phone's data plan is the thing people worry about most |
| "Use the phone only when the main line is slower than X MB/s" | M | ★★★ | Makes the phone a booster instead of a constant drain |
| Measure each network first (a short test download) and give connections in proportion from the start | S | ★★ | Today every network starts equal until it has shown its speed |
| A picture of the networks in the progress window: one channel per network, labelled with its speed | S | ★★ | The flow picture already draws channels; it could draw networks instead of parts |
| Detect captive portals (a hotel Wi-Fi that answers every request with a login page) and rest that network | M | ★★ | Such a network "works" but delivers a web page instead of the file |
| Better kind detection on Windows by asking for the adapter's description (it tells "Remote NDIS", "Apple Mobile Device Ethernet" even when the name is "Ethernet 3") | M | ★★ | Names on Windows say little |
| IPv6-only and dual-stack networks used properly | M | ★ | Today a card is used by its IPv4 address when it has one |
| Choose the networks for one download only | S | ★ | A big file over everything, a small one over Wi-Fi |
| Linux helper: a note or script that sets up policy routing so that source-address binding works on every kernel | S | ★ | Older kernels cannot bind to a device without privileges |

## More speed from the same line

- Download one file from **several mirrors** at once and check them against each other (Metalink) (M, ★★★)
- Adapt the number of connections to what the server and line really give, and stop adding connections that do not help (M, ★★)
- HTTP/2 and HTTP/3 where a server offers them, fewer connections for the same speed (L, ★)
- Write to disk in larger batches and avoid re-reading parts that are already there (S, ★)

## More places to download from

- **Streams**: HLS (`.m3u8`) and DASH, joined into one file (L, ★★★)
- **Video sites** through an installed `yt-dlp`, with a progress window like any other download (M, ★★★)
- **FTP and SFTP** (M, ★★), **torrents and magnet links** (L, ★★★)
- Direct links from Google Drive, Dropbox and OneDrive share pages (M, ★★)
- A **site grabber** with depth, file types, size limits and "same folder only" (M, ★★), RSS and podcast feeds as a watch list (M, ★★)

## After a download finishes

- Check the file against a `.sha256` or `.md5` file that lies next to it, automatically (S, ★★)
- Unpack archives, move files by rules (type, site, size), run a program or script (M, ★★★)
- Scan with the system's antivirus (Windows Defender, ClamAV) before the file appears in its folder (M, ★★)
- Tell somebody: a notification, a webhook, Telegram, Discord, e-mail (S to M, ★★)

## The web interface and the server

| Idea | Effort | Value |
| --- | --- | --- |
| **Point the browser add-on at a server** (address and a token in the add-on's pop-up), so that links from any laptop go to the one central machine | M | ★★★ |
| Live updates by server-sent events instead of asking every 1.5 seconds | S | ★★ |
| Quotas per person: disk space, bytes per day, a share of the bandwidth | M | ★★★ |
| A fair-share speed limit (everybody gets an equal part of the line when it is busy) | M | ★★ |
| Share links with an expiry date for finished files, and "download these 5 files as one zip" | M | ★★★ |
| Sign in with an identity provider (OIDC: Google, GitHub, Keycloak) and two-step sign-in (TOTP) | L | ★★★ |
| A history page with charts (bytes per day, per person, per network) and an audit log (who added or removed what) | M | ★★ |
| Tokens with a narrower scope ("may add links, nothing else") and an expiry date | S | ★★ |
| `docker-compose.yml` with a reverse proxy (Caddy or nginx) and HTTPS set up, and a Helm chart | S | ★★★ |
| **Several workers, one page**: small download agents on other machines (each with its own line or networks) that one central page controls | L | ★★★ |
| An OpenAPI description of the JSON API, and a small command-line client | S | ★★ |

## The desktop program

- A floating **drop target** and a global shortcut that adds the link on the clipboard (S, ★★)
- A **theme editor** and import/export of themes as a small JSON file (M, ★)
- Statistics: how much was downloaded this week, per site, per network (M, ★)
- Translations (the text is already in one place per window) and a screen-reader pass (L, ★★)
- Installers and package managers: winget, Scoop, Homebrew, AUR, Flatpak, Snap, an MSI (M, ★★★)
- Optional, signed update check that only ever asks and never installs by itself (M, ★★)
- macOS notarisation, so that macOS stops warning about an unknown developer (M, ★★★)

## Quality and trust

- Fuzz the HTTP parser and the link parsers, and a benchmark suite so speed changes are visible (M, ★)
- A software bill of materials with every release, reproducible builds (M, ★★)
- Run the tests on real machines with two network cards (Windows, Linux, macOS) before each release, and write down what was seen (S, ★★★)

## Suggested next steps

1. **The add-on talking to a server** (token already exists): the most useful thing for a team.
2. **Weights and caps per network**, so a phone can safely be a booster.
3. **Mirrors and Metalink**, **after-download actions** and **HLS streams**: the things people ask IDM-style programs for most.
4. **docker-compose and HTTPS** examples, so the server is a five-minute job.
