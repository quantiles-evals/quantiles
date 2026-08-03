#[expect(
    clippy::allow_attributes,
    clippy::pedantic,
    reason = "ConnectRPC and Buffa generated code uses allow attributes"
)]
pub(super) mod generated {
    connectrpc::include_generated!();
}

pub(super) use generated::quantiles::benchmark::v1;
