use linear_cli::platform::spinner;

#[test]
fn spinner_policy_matches_terminal_json_and_no_color_inputs() {
    for (json, tty, no_color_absent, expected) in [
        (false, true, true, true),
        (false, false, true, false),
        (true, true, true, false),
        (false, true, false, false),
    ] {
        assert_eq!(spinner::enabled(json, tty, no_color_absent), expected);
    }
}

#[test]
fn spinner_frames_and_clear_are_the_frozen_terminal_bytes() {
    assert_eq!(spinner::frame(0), "\r\x1b[K⠋\x1b[0m ");
    assert_eq!(spinner::frame(1), "\r\x1b[K⠙\x1b[0m ");
    assert_eq!(spinner::frame(9), "\r\x1b[K⠏\x1b[0m ");
    assert_eq!(spinner::frame(10), spinner::frame(0));
    assert_eq!(spinner::CLEAR, b"\r\x1b[K");
    assert_eq!(spinner::TICK_INTERVAL.as_millis(), 75);
}
