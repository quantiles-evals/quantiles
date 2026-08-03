fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/quantiles/benchmark/v1/benchmark_registry.proto");

    connectrpc_build::Config::new()
        .files(&["proto/quantiles/benchmark/v1/benchmark_registry.proto"])
        .use_buf()
        .include_file("_connectrpc.rs")
        .compile()?;

    Ok(())
}
