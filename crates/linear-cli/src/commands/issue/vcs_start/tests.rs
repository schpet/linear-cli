use super::free_suffix;

#[test]
fn an_existing_branch_gets_the_first_free_suffix() {
    let mut checked = Vec::new();
    let branch = free_suffix("eng-1", |name| {
        checked.push(name.to_owned());
        Ok(name != "eng-1-3")
    })
    .expect("free suffix");
    assert_eq!(branch, "eng-1-3");
    assert_eq!(checked, ["eng-1-1", "eng-1-2", "eng-1-3"]);
}
