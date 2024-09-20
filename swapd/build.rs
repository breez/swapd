fn main() {
    println!("cargo:rerun-if-changed=migrations");
    tonic_build::configure()
        .build_server(true)
        .compile(&["proto/swap/swap.proto"], &["proto/swap"])
        .unwrap();

    tonic_build::configure()
        .build_server(true)
        .compile(
            &["proto/swap_internal/swap_internal.proto"],
            &["proto/swap_internal"],
        )
        .unwrap();

    tonic_build::configure()
        .build_client(true)
        .compile(&["proto/cln/node.proto"], &["proto/cln"])
        .unwrap();
}
