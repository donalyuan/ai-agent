use std::process::Command;

#[test]
fn import_command_requires_plaintext_credential_confirmation() {
    let output = Command::new(env!("CARGO_BIN_EXE_import_model_config"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--confirm-plaintext-credentials"));
}
