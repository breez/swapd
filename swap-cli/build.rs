fn main() {
    tonic_build::configure()
        .type_attribute(".", "#[derive(serde::Serialize,serde::Deserialize)]")
        .build_server(true)
        .compile_protos(
            &["../swapd/proto/swap_internal/swap_internal.proto"],
            &["../swapd/proto/swap_internal"],
        )
        .unwrap();
}
