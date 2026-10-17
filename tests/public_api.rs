//! Shipped command contracts for standalone and virtual workspace manifests.

use std::env;

use async_fs_io::{
    TempDir,
    ensure_dir,
    read_string_bounded,
    write_bytes,
};
use tokio::process::Command;

#[tokio::test]
async fn version_identifies_the_installed_package() {
    let result = Command::new(env!("CARGO_BIN_EXE_cargo-fmt-toml"))
        .args(["fmt-toml", "--version"])
        .output()
        .await
        .expect("version command");
    assert!(result.status.success());
    let expected = format!(
        "cargo-fmt-toml {}.{}.{}\n",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH")
    );
    assert_eq!(
        String::from_utf8(result.stdout).expect("version text"),
        expected
    );
}

#[tokio::test]
async fn check_is_nonmutating_and_formatting_preserves_manifest_semantics() {
    let directory = TempDir::create(env::temp_dir()).await.expect("fixture");
    ensure_dir(directory.path().join("src"))
        .await
        .expect("source directory");
    write_bytes(directory.path().join("src/lib.rs"), b"//! Fixture.\n")
        .await
        .expect("source");
    let manifest = directory.path().join("Cargo.toml");
    let original = "[features]\ndefault=[]\nbackend=[]\n[package]\nversion='1.2.3'\nname='format-fixture'\nedition='2024'\n";
    write_bytes(&manifest, original.as_bytes())
        .await
        .expect("manifest");
    let invoke = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fmt-toml"));
        command
            .args(["fmt-toml", "--quiet", "--workspace-path"])
            .arg(directory.path());
        command
    };
    let check = invoke()
        .arg("--check")
        .output()
        .await
        .expect("check command");
    assert_eq!(
        check.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert_eq!(
        read_string_bounded(&manifest, 65536)
            .await
            .expect("unchanged manifest"),
        original
    );
    assert!(
        invoke()
            .output()
            .await
            .expect("format command")
            .status
            .success()
    );
    let formatted = read_string_bounded(&manifest, 65536)
        .await
        .expect("formatted manifest");
    assert_eq!(
        toml::from_str::<toml::Value>(original).expect("original TOML"),
        toml::from_str::<toml::Value>(&formatted).expect("formatted TOML")
    );
    assert!(
        invoke()
            .arg("--check")
            .output()
            .await
            .expect("idempotent check")
            .status
            .success()
    );
}

#[tokio::test]
async fn virtual_workspace_root_is_formatted_and_checked() {
    let directory = TempDir::create(env::temp_dir()).await.expect("fixture");
    ensure_dir(directory.path().join("member/src"))
        .await
        .expect("member directory");
    write_bytes(
        directory.path().join("member/src/lib.rs"),
        b"//! Fixture.\n",
    )
    .await
    .expect("source");
    write_bytes(
        directory.path().join("member/Cargo.toml"),
        b"[package]\nname='member'\nversion='0.1.0'\nedition='2024'\n",
    )
    .await
    .expect("member manifest");
    let manifest = directory.path().join("Cargo.toml");
    let original = "[workspace.dependencies]\nzlib='1'\naaa='1'\n[workspace]\nmembers=['member']\nresolver='3'\n";
    write_bytes(&manifest, original.as_bytes())
        .await
        .expect("root manifest");
    let invoke = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fmt-toml"));
        command
            .args(["fmt-toml", "--quiet", "--workspace-path"])
            .arg(directory.path());
        command
    };
    assert!(
        invoke()
            .output()
            .await
            .expect("format workspace")
            .status
            .success()
    );
    let formatted = read_string_bounded(&manifest, 65536)
        .await
        .expect("root manifest");
    assert_ne!(
        formatted, original,
        "virtual workspace root must be formatted"
    );
    assert!(
        formatted.find("aaa").expect("aaa dependency")
            < formatted.find("zlib").expect("zlib dependency")
    );
    assert_eq!(
        toml::from_str::<toml::Value>(original).expect("original TOML"),
        toml::from_str::<toml::Value>(&formatted).expect("formatted TOML")
    );
    write_bytes(&manifest, original.as_bytes())
        .await
        .expect("restore root drift");
    let result = invoke()
        .arg("--check")
        .output()
        .await
        .expect("check workspace");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        read_string_bounded(&manifest, 65536)
            .await
            .expect("root manifest"),
        original
    );
}
