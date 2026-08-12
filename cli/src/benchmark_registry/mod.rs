//! Client support for resolving remotely hosted benchmark definitions.

pub use self::benchmark::RemoteBenchmark;
pub use self::client::select_remote_url;
pub use self::resolver::resolve_and_download;
pub use self::version::Version;

mod benchmark;
mod client;
mod download;
mod manifest;
mod proto;
mod resolver;
mod version;
