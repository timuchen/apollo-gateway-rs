
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("sources/grpc_source/api.proto")?;
    Ok(())
}