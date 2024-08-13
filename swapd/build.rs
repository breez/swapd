fn main() {
    tonic_build::configure()
        .build_server(true)
        .compile(&["proto/swap/swap.proto"], &["proto"])
        .unwrap();

    tonic_build::configure()
        .build_client(true)
        .compile(&["proto/cln/node.proto"], &["proto/cln"])
        .unwrap();
}
