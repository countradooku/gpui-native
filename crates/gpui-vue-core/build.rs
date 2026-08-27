fn main() {
    if std::env::var("CARGO_CFG_TARGET_FAMILY").as_deref() != Ok("wasm") {
        napi_build::setup();
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        if let Err(error) = linux_runtime_link_shims() {
            println!("cargo:warning=Could not prepare Linux runtime library shims: {error}");
        }
    }
}

/// Some minimal Linux images ship the runtime libraries GPUI needs without
/// the unversioned `-dev` linker names. Build local symlinks in `OUT_DIR` when a
/// compatible SONAME is available; normal development installations bypass
/// this entirely.
#[cfg(unix)]
fn linux_runtime_link_shims() -> std::io::Result<()> {
    use std::path::{Path, PathBuf};

    const SEARCH_DIRS: &[&str] = &[
        "/usr/lib64",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/lib64",
        "/lib/x86_64-linux-gnu",
        "/lib/aarch64-linux-gnu",
    ];
    const LIBRARIES: &[(&str, &[&str])] = &[
        ("libxcb.so", &["libxcb.so.1"]),
        ("libxkbcommon.so", &["libxkbcommon.so.0"]),
        ("libxkbcommon-x11.so", &["libxkbcommon-x11.so.0"]),
    ];

    let output =
        PathBuf::from(std::env::var_os("OUT_DIR").ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "OUT_DIR is not set")
        })?)
        .join("native-link-shims");
    std::fs::create_dir_all(&output)?;
    let mut created = false;

    for (link_name, sonames) in LIBRARIES {
        let development_link_exists = SEARCH_DIRS
            .iter()
            .map(Path::new)
            .any(|directory| directory.join(link_name).exists());
        if development_link_exists {
            continue;
        }

        let runtime = SEARCH_DIRS.iter().map(Path::new).find_map(|directory| {
            sonames
                .iter()
                .map(|soname| directory.join(soname))
                .find(|candidate| candidate.exists())
        });
        let Some(runtime) = runtime else {
            continue;
        };

        let link = output.join(link_name);
        if !link.exists() {
            std::os::unix::fs::symlink(runtime, link)?;
        }
        created = true;
    }

    if created {
        println!("cargo:rustc-link-search=native={}", output.display());
    }
    Ok(())
}

#[cfg(not(unix))]
fn linux_runtime_link_shims() -> std::io::Result<()> {
    Ok(())
}
