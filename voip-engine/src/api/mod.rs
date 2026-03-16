pub mod grpc;
pub mod rest;
pub(crate) mod generated;

pub use grpc::{ServiceState, VoipGrpcService};
pub use rest::build_rest_router;
