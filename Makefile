DEPTH = 1

all:
	cargo run --release -- -d ${DEPTH}
	cargo clean
