fn main() -> std::io::Result<()> {
    println!("cargo:warning=Message");
    let mut pb = prost_build::Config::new();
    pb.protoc_arg("--experimental_allow_proto3_optional");
    pb.compile_protos(&["src/protos/country.proto"], &["src/protos"])?;
    Ok(())
}
