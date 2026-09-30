pub fn apply(open: u8, close: u8, value: &str, enabled: bool) -> String {
    if !enabled {
        return value.to_owned();
    }
    let opening = format!("\x1b[{open}m");
    let closing = format!("\x1b[{close}m");
    format!("{opening}{}{closing}", value.replace(&closing, &opening))
}

pub fn bold(value: &str, enabled: bool) -> String {
    apply(1, 22, value, enabled)
}
