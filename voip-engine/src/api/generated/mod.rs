// Generated protobuf code for the VoIP gRPC service.
//
// When building with `cargo build`, `tonic-build` compiles the proto
// definitions from `proto/voip.proto` into Rust source that is placed
// in `$OUT_DIR`.  The `include!` macro below pulls those generated
// types into this module at compile time.
//
// If the generated file does not yet exist (first checkout before
// running the build), compilation will still succeed because the
// include is behind a cfg gate.  All types that the rest of the
// crate needs are additionally defined as inline stubs inside
// `api/grpc.rs` behind the opposite cfg gate so that `cargo check`
// works without a prior `cargo build`.

/// Include protobuf-generated types when available.
#[cfg(feature = "_generated_proto")]
include!(concat!(env!("OUT_DIR"), "/voip.rs"));

/// Re-export the VoIP service server trait when generated code is present.
#[cfg(feature = "_generated_proto")]
pub use voip_engine_server::VoipEngineServer;

// When the generated code is not available, the stub types defined in
// `api/grpc.rs::proto` are used instead.  They mirror every message
// and enum from the `.proto` file so that the gRPC service implementation
// compiles cleanly in either mode.
