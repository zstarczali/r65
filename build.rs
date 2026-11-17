//! Puts the program icon and the version information (the Details tab of the file's
//! Properties) into the Windows executable. Does nothing on other systems.
//!
//! The resource script is written to OUT_DIR, so the version always comes from
//! Cargo.toml and the name, description and copyright from the constants at the
//! top of src/driver.rs.

use std::path::PathBuf;
use std::{env, fs};

/// The text of `pub const <name>: &str = "...";` in driver.rs.
fn driver_const(src: &str, name: &str) -> String {
    let start = format!("pub const {name}: &str = \"");
    let from = src.find(&start).unwrap_or_else(|| panic!("{name} not found in src/driver.rs")) + start.len();
    let len = src[from..].find('"').unwrap_or_else(|| panic!("{name} has no closing quote in src/driver.rs"));
    src[from..from + len].to_string()
}

/// A string for the resource script: a quote is written twice.
fn rc_str(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn main() {
    println!("cargo:rerun-if-changed=assets/r65.ico");
    println!("cargo:rerun-if-changed=src/driver.rs");

    let driver = fs::read_to_string("src/driver.rs").expect("couldn't read src/driver.rs");
    let name = driver_const(&driver, "APP_NAME");
    let description = driver_const(&driver, "DESCRIPTION");
    let copyright = driver_const(&driver, "COPYRIGHT");

    let env_num = |key: &str| env::var(key).ok().and_then(|v| v.parse::<u16>().ok()).unwrap_or(0);
    let (major, minor, patch) = (env_num("CARGO_PKG_VERSION_MAJOR"), env_num("CARGO_PKG_VERSION_MINOR"), env_num("CARGO_PKG_VERSION_PATCH"));
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let license = env::var("CARGO_PKG_LICENSE").unwrap_or_default();
    let repository = env::var("CARGO_PKG_REPOSITORY").unwrap_or_default();
    let company = copyright.trim_start_matches("(C)").trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ').to_string();
    let comments = match (license.is_empty(), repository.is_empty()) {
        (false, false) => format!("{license} license, {repository}"),
        (false, true) => format!("{license} license"),
        (true, false) => repository,
        (true, true) => String::new(),
    };

    let icon = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets").join("r65.ico");
    let icon = icon.to_string_lossy().replace('\\', "/");

    let rc = format!(
        r#"#pragma code_page(65001)

1 ICON {icon}

1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3F
FILEFLAGS 0x0
FILEOS 0x40004
FILETYPE 0x1
FILESUBTYPE 0x0
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", {company}
            VALUE "FileDescription", {description}
            VALUE "FileVersion", {version}
            VALUE "InternalName", "r65"
            VALUE "LegalCopyright", {copyright}
            VALUE "OriginalFilename", "r65.exe"
            VALUE "ProductName", {name}
            VALUE "ProductVersion", {version}
            VALUE "Comments", {comments}
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#,
        icon = rc_str(&icon),
        company = rc_str(&company),
        description = rc_str(&description),
        version = rc_str(&version),
        copyright = rc_str(&copyright),
        name = rc_str(&name),
        comments = rc_str(&comments),
    );

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("r65.rc");
    fs::write(&out, rc).expect("couldn't write the resource script");
    embed_resource::compile(&out, embed_resource::NONE)
        .manifest_optional()
        .expect("couldn't embed the program icon and version information");
}
