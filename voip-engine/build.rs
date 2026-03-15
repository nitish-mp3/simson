fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .out_dir("src/api/generated")
        .compile(&["proto/voip.proto"], &["proto/"])?;
    Ok(())
}
