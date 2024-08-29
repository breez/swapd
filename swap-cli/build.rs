fn main() {
    tonic_build::configure()
        .build_server(true)
        .compile(
            &["../swapd/proto/swap_internal/swap_internal.proto"],
            &["../swapd/proto/swap_internal"],
        )
        .unwrap();
}
