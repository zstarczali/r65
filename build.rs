//! Puts the program icon into the Windows executable. Does nothing on other systems.

fn main() {
    println!("cargo:rerun-if-changed=assets/r65.rc");
    println!("cargo:rerun-if-changed=assets/r65.ico");
    embed_resource::compile("assets/r65.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("couldn't embed the program icon");
}
