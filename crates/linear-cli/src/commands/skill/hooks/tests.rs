use super::super::apply::RealFiles;
use super::super::slots::SkillEnv;
use super::*;

#[test]
fn settings_changed_after_planning_are_not_overwritten() {
    let home = tempfile::tempdir().expect("home");
    let layout = Layout::user(&SkillEnv {
        home: Some(home.path().to_path_buf()),
        ..SkillEnv::default()
    })
    .expect("layout");
    let mut planned = plan(&layout, HookAgent::Claude, Some(Action::Setup)).expect("plan");
    fs::create_dir_all(layout.claude_settings.parent().expect("parent")).expect("mkdir");
    fs::write(&layout.claude_settings, b"{\"model\":\"opus\"}").expect("write");
    planned.apply(&RealFiles, false);
    assert_eq!(planned.report.change, Some(Change::Conflict));
    assert_eq!(
        fs::read(&layout.claude_settings).expect("read"),
        b"{\"model\":\"opus\"}"
    );
}
