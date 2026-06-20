.PHONY: test

test:
	cargo test --workspace --exclude vulnerable-contract --exclude kani-poc-contract
	cd frontend && npm test
