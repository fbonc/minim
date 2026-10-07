//! Work around `axuielement`'s Swift bridge auto-linking the Swift back-compat
//! shims (libswiftCompatibility56 and friends) from a search path that only
//! exists under a full Xcode install. Under the Command Line Tools layout the
//! shims live elsewhere, so the final link fails with "library not found".
//! Emit the directory that actually holds them so linking succeeds regardless
//! of which toolchain is selected.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let mut candidates = Vec::new();
    if let Some(developer_dir) = xcode_developer_dir() {
        // Full Xcode keeps the shims under the toolchain; CLT keeps them at the
        // developer dir root. Try both.
        candidates.push(format!(
            "{developer_dir}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx"
        ));
        candidates.push(format!("{developer_dir}/usr/lib/swift/macosx"));
    }
    candidates.push("/Library/Developer/CommandLineTools/usr/lib/swift/macosx".to_string());

    for dir in candidates {
        if std::path::Path::new(&dir)
            .join("libswiftCompatibility56.a")
            .exists()
        {
            println!("cargo:rustc-link-search=native={dir}");
            break;
        }
    }
}

fn xcode_developer_dir() -> Option<String> {
    let output = std::process::Command::new("xcode-select")
        .arg("-p")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if dir.is_empty() { None } else { Some(dir) }
}
