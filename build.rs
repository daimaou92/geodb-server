fn main() -> std::io::Result<()> {
    let mut pb = prost_build::Config::new();
    pb.protoc_arg("--experimental_allow_proto3_optional");
    pb.compile_protos(&["src/protos/geo.proto"], &["src/protos"])?;
    Ok(())
}
