//! Public owned Secret controls. Shared ordinary text stays literal.
use linear_cli::platform::prompt::{PromptKey, PromptOutcome, PromptSession};
use std::{collections::VecDeque, io::Cursor};
#[test]
fn script_secret_is_owned_and_masked_with_cr_lf_eof_and_no_debug_disclosure() {
    for script in [
        b"dummy_secret\r".as_slice(),
        b"dummy_secret\n",
        b"dummy_secret\r\n",
    ] {
        let mut session = PromptSession::script_cr_or_lf(Cursor::new(script), Vec::new());
        let outcome = session
            .secret("Enter your Linear API key", "dummy hint")
            .unwrap();
        let key = match outcome {
            PromptOutcome::Submitted(key) => key,
            _ => panic!("submitted"),
        };
        assert_eq!(key.expose(), "dummy_secret");
        assert!(!format!("{key:?}").contains("dummy_secret"));
        let output = session.into_output().unwrap();
        assert!(!String::from_utf8_lossy(&output).contains("dummy_secret"));
        assert!(String::from_utf8_lossy(&output).contains("************"));
    }
    let mut session = PromptSession::script_cr_or_lf(Cursor::new(Vec::<u8>::new()), Vec::new());
    assert!(matches!(
        session
            .secret("Enter your Linear API key", "dummy hint")
            .unwrap(),
        PromptOutcome::EndOfInput
    ));
}
#[test]
fn attended_secret_backspace_unicode_cursor_and_interrupt_are_mask_only() {
    let mut keys = VecDeque::from([
        PromptKey::Character('a'),
        PromptKey::Character('中'),
        PromptKey::Character('x'),
        PromptKey::Backspace,
        PromptKey::Left,
        PromptKey::Character('b'),
        PromptKey::End,
        PromptKey::Enter,
    ]);
    let mut session =
        PromptSession::<Cursor<Vec<u8>>, Vec<u8>>::keys(Vec::new(), 80, 24, move || {
            Ok(keys.pop_front().unwrap_or(PromptKey::EndOfInput))
        })
        .unwrap();
    let key = match session
        .secret("Enter your Linear API key", "dummy hint")
        .unwrap()
    {
        PromptOutcome::Submitted(key) => key,
        _ => panic!("submitted"),
    };
    assert_eq!(key.expose(), "ab中");
    let output = session.into_output().unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(!text.contains('中'));
    assert!(!text.contains("ab中"));
    let mut keys = VecDeque::from([PromptKey::Character('x'), PromptKey::Interrupt]);
    let mut session =
        PromptSession::<Cursor<Vec<u8>>, Vec<u8>>::keys(Vec::new(), 80, 24, move || {
            Ok(keys.pop_front().unwrap_or(PromptKey::EndOfInput))
        })
        .unwrap();
    assert!(matches!(
        session
            .secret("Enter your Linear API key", "dummy hint")
            .unwrap(),
        PromptOutcome::Interrupted
    ));
}
