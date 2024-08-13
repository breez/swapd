fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile(&["proto/swap/swap.proto"], &["proto"])
        .unwrap();
}
