use std::path::Path;

fn main() {
    let proto_dir = Path::new("src/protocol/connect/protos");

    // Use `protoc` if available or fall back to a pure Rust parser.
    protobuf_codegen::Codegen::new()
        .cargo_out_dir("protos")
        .include(proto_dir)
        .input(proto_dir.join("queue.proto"))
        .input(proto_dir.join("repeat.proto"))
        .run_from_script();
}
