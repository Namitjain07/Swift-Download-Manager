# Contributing to Swift Download Manager

Thank you for wanting to help. Bug reports, ideas, documentation fixes and code are all welcome.
Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Report a bug or ask for a feature

- **Bug:** open an [issue](https://github.com/Namitjain07/Swift-Download-Manager/issues/new/choose) and pick
  **Bug report**. Say which version and system you use, what you did, and what happened. Please remove
  private parts of links first (tokens, session ids, passwords).
- **Feature idea:** pick **Feature request** and describe the problem you want solved, not only the solution.
- **Security problem:** do **not** open a public issue. Follow [SECURITY.md](SECURITY.md).

Search the existing issues first; your topic may already be there.

## Set up

You need [Rust](https://rustup.rs) (a recent stable version; the project uses the 2024 edition).
The app lives in the [`source/`](source/) folder:

```bash
git clone https://github.com/Namitjain07/Swift-Download-Manager.git
cd Swift-Download-Manager/source
cargo run --release     # opens the app
cargo test              # runs the automatic tests
```

The [`Makefile`](Makefile) in the main folder has the same and more (`make help` lists it): `make build test`, `make install`,
and the packages (`make deb`, `make arch`, `make windows-installer`; the Windows installer needs [Inno Setup](https://jrsoftware.org/isinfo.php)).
How releases are signed and how to check them is in [SECURITY.md](SECURITY.md); the private keys are never in the repository.

On Linux the app also needs the usual desktop libraries (they are already there on desktop systems).
To try the window without a screen, `cargo run --release -- --demo` shows sample data and downloads nothing.

## Where things are

| Path | What it is |
| --- | --- |
| `source/src/engine.rs` | The download engine: connections, splitting, resume, retries, speed limits |
| `source/src/bridge.rs` | The local link listener for the browser add-on, and clipboard watching |
| `source/src/app.rs`, `source/src/app/` | The window, dialogs, queues, scheduler and link grabber |
| `source/browser-extension/` | The browser add-on (this is where to edit it) |
| `source/packaging/` | Installer scripts for Linux, Windows and Mac |

## Making a change

1. Fork the repository and create a branch from `main`.
2. Keep the change focused: one fix or feature per pull request.
3. Add or update a test when you change the download engine, the listener or the link handling.
   The tests in `source/src/engine_tests.rs` start small local servers, so they run without internet.
4. Run `cargo test` in `source/` and make sure it passes. Pull requests are also checked by GitHub Actions on
   Linux, Windows and macOS.
5. Follow the style of the code around your change. The project is not formatted with `rustfmt` yet, so please do not
   reformat whole files; it hides the real change in the review.
6. Update [README.md](README.md) if you change something a user can see or do.
7. Open a pull request and fill in the template. For a change to the window, a screenshot helps a lot
   (see "Retaking the screenshot" in the README).

### Please do not include

- Built files: the programs in `linux/`, `windows/` and `mac/`, the packaged browser add-ons, and the source `.zip`
  are made by the maintainer for each release. Change the source and leave those alone.
- Edits to the version number or `CHANGELOG.md`; the maintainer does that when a release is made.

### The browser add-on

Edit the files in `source/browser-extension/`. To try your change, run
`python3 packaging/browser/build_extension.py` in `source/`, then load `dist/browser/chrome/` as an unpacked
extension (Chrome, Edge, Brave, Opera) or `dist/browser/firefox/` through `about:debugging` (Firefox).

## Commit messages

Write what the change does in the first line, in the present tense ("Retry a broken connection once more"),
and add a few lines of why when it is not obvious.

## License

By contributing, you agree that your contribution is released under the project's [license](LICENSE).
