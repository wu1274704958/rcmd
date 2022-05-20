local server
cargo run --example test_udp --release 0.0.0.0 8080 As147258369
cargo run --example udp_c --release -- -i 127.0.0.1 -p 8080