use super::*;
use crate::config::test_support::TempTree;

#[test]
fn repo_root_is_the_nearest_git_or_jj_ancestor() {
    let tree = TempTree::new();
    tree.mkdir("plain/sub");
    assert_eq!(repo_root(&tree.join("plain/sub"), &RealFileSource), None);

    tree.mkdir("git/.git");
    tree.mkdir("git/a/b");
    assert_eq!(
        repo_root(&tree.join("git/a/b"), &RealFileSource),
        Some(tree.join("git"))
    );

    // A worktree's `.git` is a file.
    tree.write("worktree/.git", b"gitdir: /elsewhere\n");
    tree.mkdir("worktree/src");
    assert_eq!(
        repo_root(&tree.join("worktree/src"), &RealFileSource),
        Some(tree.join("worktree"))
    );

    tree.mkdir("jj/.jj");
    tree.mkdir("jj/nested/.jj-not");
    assert_eq!(
        repo_root(&tree.join("jj/nested"), &RealFileSource),
        Some(tree.join("jj"))
    );
}

#[test]
fn config_file_reader_distinguishes_missing_empty_large_and_directory() {
    let tree = TempTree::new();
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.join("missing.toml")),
        ReadCandidate::Absent
    ));
    tree.write("empty.toml", b"");
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.join("empty.toml")),
        ReadCandidate::Contents(file) if file.bytes.is_empty()
    ));
    tree.write("large.toml", &vec![b'x'; 1024 * 1024 + 1]);
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.join("large.toml")),
        ReadCandidate::TooLarge { .. }
    ));
    fs::create_dir(tree.join("directory.toml")).unwrap();
    assert!(matches!(
        read_config_candidate(&RealFileSource, &tree.join("directory.toml")),
        ReadCandidate::Poisoned { .. }
    ));
}
