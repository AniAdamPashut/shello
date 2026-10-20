.PHONY: build test integration integration-docker

build:
	CGO_ENABLED=0 go build -o shello .

test:
	go test ./...

integration: build
	bash tests/integration.sh

# Runs the integration test for bash, zsh and fish in an Alpine container.
integration-docker: build
	docker run --rm --network host -v "$(CURDIR)":/w:ro alpine:3.20 sh -c \
		'apk add -q bash zsh fish util-linux-misc python3 coreutils findutils >/dev/null && cp -r /w /r && cd /r && SHELLS="bash zsh fish" bash tests/integration.sh'
