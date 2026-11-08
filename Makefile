# Swift Download Manager: one place for the everyday commands.
#
#   make help                 list what can be done
#   make build test           build the program, run the tests
#   sudo make install         install it under /usr/local (PREFIX=/usr DESTDIR=... for packagers)
#   make deb                  the Debian/Ubuntu package for this computer (amd64 or arm64)
#   make arch                 the Arch Linux package for this computer (x86_64 or aarch64)
#   make windows-installer    the Windows setup program (run in Git Bash or PowerShell on Windows; ARCH=arm64 for ARM)
#   make cross                builds for many platforms in Docker, into out/: Linux (Intel/AMD, ARM, RISC-V, POWER, IBM Z, static and glibc),
#                             Windows (x64, x86, ARM) and BSD; CROSS='linux-aarch64 windows-*' for some only; make cross-list shows them
#   make checksums            SHA256SUMS for everything in source/dist, and make sign-checksums to sign that file
#
# The program lives in the folder "source". Everything built goes to source/dist.

SHELL      := /bin/sh
CARGO      ?= cargo
PREFIX     ?= /usr/local
DESTDIR    ?=
APP        := source
NAME       := swift-download-manager
VERSION    := $(shell grep -m1 '^version' $(APP)/Cargo.toml | cut -d'"' -f2)
BIN        := $(APP)/target/release/swift_download_manager
ICON_SIZES := 32 48 64 128 256 512
# Windows installer: x64 (default) or arm64.
ARCH       ?= x64
# The key that signs SHA256SUMS (a GPG key id or fingerprint); needed for sign-checksums only.
GPG_KEY    ?=

.DEFAULT_GOAL := help
.PHONY: help build test check fmt run clean install uninstall deb arch windows-installer cross cross-list cross-test out extension docker checksums sign-checksums version

help: ## Show this list
	@echo "Swift Download Manager $(VERSION)"
	@echo
	@grep -E '^[a-zA-Z_-]+:.*## ' $(MAKEFILE_LIST) | awk 'BEGIN { FS = ":.*## " } { printf "  make %-20s %s\n", $$1, $$2 }'

version: ## Print the version
	@echo $(VERSION)

build: ## Build the program (release)
	cd $(APP) && $(CARGO) build --release --locked

test: ## Run the tests
	cd $(APP) && $(CARGO) test --locked

check: ## Lint with clippy (warnings are errors for the new code you touch; see CONTRIBUTING.md)
	cd $(APP) && $(CARGO) clippy --all-targets --locked

fmt: ## Check the formatting
	cd $(APP) && $(CARGO) fmt --check

run: ## Run the program
	cd $(APP) && $(CARGO) run --release --locked

clean: ## Remove what was built
	cd $(APP) && $(CARGO) clean
	rm -rf $(APP)/dist

install: build ## Install under PREFIX (default /usr/local): program, menu entry, icons, browser add-on
	install -Dm755 $(BIN) $(DESTDIR)$(PREFIX)/bin/$(NAME)
	install -Dm644 $(APP)/packaging/linux/$(NAME).desktop $(DESTDIR)$(PREFIX)/share/applications/$(NAME).desktop
	mkdir -p $(APP)/dist
	for size in $(ICON_SIZES); do \
	  $(BIN) --write-icon "$(APP)/dist/icon-$$size.png" $$size && \
	  install -Dm644 "$(APP)/dist/icon-$$size.png" "$(DESTDIR)$(PREFIX)/share/icons/hicolor/$${size}x$${size}/apps/$(NAME).png"; \
	done
	install -d $(DESTDIR)$(PREFIX)/share/$(NAME)
	cp -r $(APP)/browser-extension $(DESTDIR)$(PREFIX)/share/$(NAME)/
	install -Dm644 $(APP)/README.md $(DESTDIR)$(PREFIX)/share/doc/$(NAME)/README.md
	install -Dm644 LICENSE $(DESTDIR)$(PREFIX)/share/doc/$(NAME)/LICENSE
	@command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q $(DESTDIR)$(PREFIX)/share/applications || true
	@echo "Installed. Start it with: $(NAME)"

uninstall: ## Remove what "make install" installed (settings and downloads are not touched)
	rm -f $(DESTDIR)$(PREFIX)/bin/$(NAME) $(DESTDIR)$(PREFIX)/share/applications/$(NAME).desktop
	for size in $(ICON_SIZES); do rm -f "$(DESTDIR)$(PREFIX)/share/icons/hicolor/$${size}x$${size}/apps/$(NAME).png"; done
	rm -rf $(DESTDIR)$(PREFIX)/share/$(NAME) $(DESTDIR)$(PREFIX)/share/doc/$(NAME)

deb: ## Debian/Ubuntu package for this computer (needs dpkg-deb; amd64 on Intel/AMD, arm64 on ARM)
	cd $(APP) && bash packaging/linux/build-deb.sh

arch: ## Arch Linux package for this computer (run as a normal user on Arch; packages the last commit)
	cd $(APP) && bash packaging/arch/build-arch.sh

windows-installer: ## Windows setup program, ARCH=x64 or arm64 (on Windows; SIGN=1 signs it, see packaging/windows/sign.ps1)
	powershell -NoProfile -ExecutionPolicy Bypass -File $(APP)/packaging/windows/build-installer.ps1 -Arch $(ARCH) $(if $(SIGN),-Sign,)

cross: ## Build for many platforms in Docker into out/ (CROSS='linux-aarch64 windows-*' for some; see cross-list)
	bash $(APP)/packaging/cross/cross.sh build $(CROSS)

cross-list: ## List the platforms "make cross" can build for
	@bash $(APP)/packaging/cross/cross.sh list $(CROSS)

cross-test: ## Start the Linux programs from "make cross" on their own CPU in Docker (emulated) and check them
	bash $(APP)/packaging/cross/cross.sh test $(CROSS)

out: ## Copy the installers and packages of this version (windows-installer, deb, arch) from source/dist into out/, next to the cross builds
	mkdir -p out
	find $(APP)/dist -maxdepth 2 -type f -name '*$(VERSION)*' \( -name '*-setup-*.exe' -o -name '*.deb' -o -name '*.pkg.tar.zst' \) -exec cp -f {} out/ \;
	@ls -1 out

extension: ## Browser add-on packages (Chrome zip, Firefox xpi)
	cd $(APP) && python3 packaging/browser/build_extension.py

docker: ## Build the server image
	cd $(APP) && docker build -t $(NAME):$(VERSION) .

checksums: ## SHA256SUMS for every file in source/dist
	cd $(APP)/dist && find . -maxdepth 2 -type f ! -name SHA256SUMS ! -name '*.asc' ! -name '*.sig' ! -name '*.png' | sed 's|^\./||' | sort | xargs sha256sum > SHA256SUMS
	@cat $(APP)/dist/SHA256SUMS

sign-checksums: checksums ## Sign SHA256SUMS with GPG_KEY=<key id> (writes SHA256SUMS.asc)
	@test -n "$(GPG_KEY)" || { echo "Say which key: make sign-checksums GPG_KEY=<fingerprint>"; exit 1; }
	gpg --batch --yes --armor --detach-sign --local-user "$(GPG_KEY)" --output $(APP)/dist/SHA256SUMS.asc $(APP)/dist/SHA256SUMS
	@echo "Check it with: gpg --verify $(APP)/dist/SHA256SUMS.asc $(APP)/dist/SHA256SUMS"
