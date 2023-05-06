const SUFFIX: [&str; 9] = ["B", "K", "M", "G", "TB", "PB", "EB", "ZB", "YB"];

pub fn humanize_bytes<T: Into<f64>>(size: T) -> String {
    let size = size.into();

    if size <= 0.0 {
        return "0 B".to_string();
    }

    let base = size.log10() / 1024_f64.log10();

    let mut result = format!("{:.1}", 1024_f64.powf(base - base.floor()))
        .trim_end_matches(".0")
        .to_owned();

    result.push(' ');
    result.push_str(SUFFIX[base.floor() as usize]);

    result
}

pub fn humanize_percentage(v: f64) -> String {
    format!("{:.1}%", 100.0 * v)
}

const INFINITY: i64 = 8640000;
const INFINITY_SYMBOL: &str = "∞";

// qBittorrent/src/base/utils/misc.cpp - userFriendlyDuration
pub fn humanize_eta(v: i64) -> String {
    let mut minutes = v / 60;
    let mut hours = minutes / 60;
    let mut days = hours / 24;
    let years = days / 365;
    match v {
        v if v < 0 => INFINITY_SYMBOL.to_owned(),
        v if v >= INFINITY => INFINITY_SYMBOL.to_owned(),
        v if v == 0 => "0s".to_owned(),
        v if v < 60 => "< 1m".to_owned(),
        _ if minutes < 60 => format!("{}m", minutes),
        _ if hours < 24 => {
            minutes -= hours * 60;
            format!("{}h {}m", hours, minutes)
        }
        _ if days < 365 => {
            hours -= days * 24;
            format!("{}d {}h", days, hours)
        }
        _ if days >= 365 => {
            days -= years * 365;
            format!("{}y {}d", years, days)
        }
        _ => INFINITY_SYMBOL.to_owned(),
    }
}
