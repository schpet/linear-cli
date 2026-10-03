//! Portable, dummy-only public adapter contracts. No native OS service calls.
use linear_cli::auth::keyring::ReaderFlavor;
use linear_cli::config::ConfigSecret;

#[test]
fn flavor_argv_preserves_literal_workspace_and_existing_mutation_policy() {
    let workspace = "dummy space;'中";
    assert_eq!(ReaderFlavor::MacSecurity.executable(), "/usr/bin/security");
    assert_eq!(
        ReaderFlavor::MacSecurity.lookup_arguments(workspace),
        [
            "find-generic-password",
            "-a",
            workspace,
            "-s",
            "linear-cli",
            "-w"
        ]
    );
    assert_eq!(
        ReaderFlavor::SecretTool.lookup_arguments(workspace),
        ["lookup", "service", "linear-cli", "account", workspace]
    );
    let secret = ConfigSecret::new("dummy key\n中".to_owned());
    assert!(
        ReaderFlavor::MacSecurity
            .store_command(workspace, &secret)
            .is_err()
    );
    let plain = ConfigSecret::new("lin_api_ab-1.2".to_owned());
    let mac = ReaderFlavor::MacSecurity
        .store_command("acme-co", &plain)
        .unwrap();
    assert_eq!(mac.arguments, ["-i"]);
    assert_eq!(
        String::from_utf8(mac.input).unwrap(),
        "add-generic-password -U -a acme-co -s linear-cli -w lin_api_ab-1.2\n"
    );
    assert_eq!(
        ReaderFlavor::MacSecurity.delete_arguments(workspace),
        [
            "delete-generic-password",
            "-a",
            workspace,
            "-s",
            "linear-cli"
        ]
    );
    let secret_tool = ReaderFlavor::SecretTool
        .store_command(workspace, &secret)
        .unwrap();
    assert_eq!(secret_tool.input, "dummy key\n中".as_bytes());
    assert_eq!(
        secret_tool.arguments,
        vec![
            "store".to_owned(),
            "--label".to_owned(),
            format!("linear-cli: {workspace}"),
            "service".to_owned(),
            "linear-cli".to_owned(),
            "account".to_owned(),
            workspace.to_owned()
        ]
    );
    assert_eq!(
        ReaderFlavor::SecretTool.delete_arguments(workspace),
        ["clear", "service", "linear-cli", "account", workspace]
    );
}
