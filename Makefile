.PHONY: test-robot test-integration test-all build-robot help

test-robot: ## Run the Robot Framework blackbox integration test suite
	mkdir -p tests/robot/results
	CURRENT_UID=$(shell id -u) CURRENT_GID=$(shell id -g) \
		docker compose -f tests/robot/docker-compose.yml run --rm robot

build-robot: ## Build the Docker images used by the Robot Framework test suite
	CURRENT_UID=$(shell id -u) CURRENT_GID=$(shell id -g) \
		docker compose -f tests/robot/docker-compose.yml build

test-integration: ## Run the Rust integration test suite
	cargo test --test integration

test-all: test-integration test-robot ## Run all tests (Robot Framework + Rust integration)

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-20s %s\n", $$1, $$2}'
