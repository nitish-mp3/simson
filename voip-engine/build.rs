fn main() {
    // Rerun only when the proto file changes.
    println!("cargo:rerun-if-changed=proto/voip.proto");
    println!("cargo:rerun-if-changed=proto/");

    // Proto compilation is best-effort; if protoc is unavailable (e.g. during
    // cross-compilation) the build continues in stub mode (no _generated_proto
    // feature set).  All gRPC types fall back to the inline stubs in
    // src/api/grpc.rs.
    if let Err(e) = tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .out_dir("src/api/generated")
        .compile(&["proto/voip.proto"], &["proto/"])
    {
        println!("cargo:warning=proto compilation skipped: {e}");
    }
}
