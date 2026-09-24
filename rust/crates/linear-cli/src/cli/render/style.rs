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

pub fn dim(value: &str, enabled: bool) -> String {
    apply(2, 22, value, enabled)
}

pub fn italic(value: &str, enabled: bool) -> String {
    apply(3, 23, value, enabled)
}

pub fn red(value: &str, enabled: bool) -> String {
    apply(31, 39, value, enabled)
}

pub fn green(value: &str, enabled: bool) -> String {
    apply(32, 39, value, enabled)
}

pub fn yellow(value: &str, enabled: bool) -> String {
    apply(33, 39, value, enabled)
}

pub fn bright_blue(value: &str, enabled: bool) -> String {
    apply(94, 39, value, enabled)
}

pub fn bright_magenta(value: &str, enabled: bool) -> String {
    apply(95, 39, value, enabled)
}
