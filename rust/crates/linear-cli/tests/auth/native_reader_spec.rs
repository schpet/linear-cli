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
    assert_eq!(
        ReaderFlavor::MacSecurity.store_arguments(workspace, &secret),
        [
            "add-generic-password",
            "-a",
            workspace,
            "-s",
            "linear-cli",
            "-w",
            "dummy key\n中",
            "-U"
        ]
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
    assert_eq!(
        ReaderFlavor::SecretTool.store_arguments(workspace, &secret),
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
