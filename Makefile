.PHONY: build test integration

build:
	CGO_ENABLED=0 go build -o shello .

test:
	go test ./...

integration: build
	bash tests/integration.sh
