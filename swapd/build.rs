fn main() {
    println!("cargo:rerun-if-changed=src/postgresql/migrations");

    tonic_build::configure()
        .emit_rerun_if_changed(true)
        .build_server(true)
        .compile_protos(&["proto/swap/swap.proto"], &["proto/swap"])
        .unwrap();

    tonic_build::configure()
        .emit_rerun_if_changed(true)
        .build_server(true)
        .compile_protos(
            &["proto/swap_internal/swap_internal.proto"],
            &["proto/swap_internal"],
        )
        .unwrap();

    tonic_build::configure()
        .emit_rerun_if_changed(true)
        .build_client(true)
        .compile_protos(&["proto/cln/node.proto"], &["proto/cln"])
        .unwrap();

    tonic_build::configure()
        .emit_rerun_if_changed(true)
        .build_client(true)
        .compile_protos(
            &["proto/lnd/lightning.proto", "proto/lnd/router.proto"],
            &["proto/lnd"],
        )
        .unwrap();
}
