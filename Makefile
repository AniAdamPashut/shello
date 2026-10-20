.PHONY: build test integration integration-docker

build:
	cargo build --release
	cp target/release/shello shello

test:
	cargo test

integration: build
	bash tests/install_test.sh
	bash tests/integration.sh

# Runs the integration test for bash, zsh and fish in an Arch Linux container
# (glibc, so the host-built binary runs there).
integration-docker: build
	docker run --rm --network host -v "$(CURDIR)":/w:ro archlinux:latest sh -c \
		'pacman -Sy --noconfirm --needed zsh fish python >/dev/null 2>&1 && cp -r /w /r && cd /r && SHELLS="bash zsh fish" bash tests/integration.sh'
