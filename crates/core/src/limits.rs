//! Claude's plan limits for the status bar (UC-06, ADR 0009): the 5-hour and 7-day windows as Claude hands them to its
//! status line, and, while *Model limits* is on, the weekly limits per model (Fable) from Anthropic's usage endpoint.
use serde_json::Value;

/// From this share used on, a meter is marked (UC-06.2).
pub const WARN: f32 = 80.0;
/// Anthropic is asked at most this often (NFR-03).
pub const ASK_EVERY: u64 = 5 * 60;

#[derive(Clone, Debug, PartialEq)]
pub struct Meter {
    /// "5-hour", "7-day", or the model's name ("Fable").
    pub name: String,
    pub percent: f32,
    /// When the window starts over (seconds since 1970).
    pub resets_at: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Limits {
    pub meters: Vec<Meter>,
    /// When the numbers were current (seconds since 1970).
    pub at: u64,
}

pub const FIVE_HOUR: &str = "5-hour";
pub const SEVEN_DAY: &str = "7-day";

/// A reset time as Claude gives it: seconds or milliseconds since 1970, or an ISO 8601 time.
fn reset(v: Option<&Value>) -> Option<u64> {
    match v? {
        Value::Number(n) => n.as_f64().map(|s| if s > 1e12 { s / 1000.0 } else { s } as u64),
        Value::String(s) => iso(s),
        _ => None,
    }
}

fn meter(name: &str, v: &Value) -> Option<Meter> {
    let percent = v.get("used_percentage").or_else(|| v.get("utilization")).or_else(|| v.get("percent"))?.as_f64()?;
    Some(Meter { name: name.into(), percent: percent as f32, resets_at: reset(v.get("resets_at")) })
}

/// The file Yggdrasil's status line writes: `{"t":…,"rate_limits":{"five_hour":{…},"seven_day":{…}}}`.
pub fn from_status_line(text: &str) -> Option<Limits> {
    let v: Value = serde_json::from_str(text.trim()).ok()?;
    let r = v.get("rate_limits")?;
    let meters: Vec<Meter> = [(FIVE_HOUR, "five_hour"), (SEVEN_DAY, "seven_day")]
        .iter()
        .filter_map(|(name, key)| meter(name, r.get(key)?))
        .collect();
    (!meters.is_empty()).then(|| Limits { meters, at: v.get("t").and_then(Value::as_u64).unwrap_or(0) })
}

/// Anthropic's usage endpoint's answer: its rows (`limits[]`, the session window, the week, and the week per model),
/// or the older fields (`five_hour`, `seven_day`).
pub fn from_usage(text: &str, now: u64) -> Option<Limits> {
    let v: Value = serde_json::from_str(text).ok()?;
    let mut meters = Vec::new();
    if let Some(rows) = v.get("limits").and_then(Value::as_array) {
        for row in rows {
            let model = row.pointer("/scope/model/display_name").and_then(Value::as_str).map(str::trim);
            let name = match (row.get("kind").and_then(Value::as_str), model) {
                (Some("session"), _) => FIVE_HOUR.to_string(),
                (Some("weekly_all"), _) => SEVEN_DAY.to_string(),
                (Some("weekly_scoped"), Some(m)) if !m.is_empty() => m.to_string(),
                _ => continue,
            };
            if !meters.iter().any(|m: &Meter| m.name == name)
                && let Some(m) = meter(&name, row)
            {
                meters.push(m);
            }
        }
    }
    for (name, key) in [(FIVE_HOUR, "five_hour"), (SEVEN_DAY, "seven_day")] {
        if !meters.iter().any(|m| m.name == name)
            && let Some(m) = v.get(key).and_then(|x| meter(name, x))
        {
            meters.push(m);
        }
    }
    (!meters.is_empty()).then_some(Limits { meters, at: now })
}

/// What the bar shows: the 5-hour and 7-day windows from the newer of the two, then the models' weeks.
pub fn merge(status: Option<&Limits>, usage: Option<&Limits>) -> Limits {
    let newer = match (status, usage) {
        (Some(s), Some(u)) => Some(if u.at > s.at { u } else { s }),
        (s, u) => s.or(u),
    };
    let mut meters: Vec<Meter> = [FIVE_HOUR, SEVEN_DAY]
        .iter()
        .filter_map(|name| {
            newer.and_then(|l| l.meters.iter().find(|m| m.name == *name)).or_else(|| {
                [status, usage].into_iter().flatten().find_map(|l| l.meters.iter().find(|m| m.name == *name))
            })
        })
        .cloned()
        .collect();
    if let Some(u) = usage {
        meters.extend(u.meters.iter().filter(|m| m.name != FIVE_HOUR && m.name != SEVEN_DAY).cloned());
    }
    Limits { meters, at: newer.map_or(0, |l| l.at) }
}

/// Older than this, numbers are shown as old (UC-06.5).
pub const OLD: u64 = 15 * 60;

/// The newest numbers of several sources (the Mac's and each sandbox's status line): each limit from the newest one
/// that has it.
pub fn newest(all: &[Limits]) -> Option<Limits> {
    let mut sorted: Vec<&Limits> = all.iter().collect();
    sorted.sort_by_key(|l| std::cmp::Reverse(l.at));
    let mut meters: Vec<Meter> = Vec::new();
    for l in &sorted {
        for m in &l.meters {
            if !meters.iter().any(|x| x.name == m.name) {
                meters.push(m.clone());
            }
        }
    }
    meters.sort_by_key(|m| [FIVE_HOUR, SEVEN_DAY].iter().position(|n| *n == m.name).unwrap_or(2));
    sorted.first().map(|l| Limits { meters, at: l.at })
}

/// Without the limits whose window has started over since: their numbers are wrong for sure (UC-06.5).
pub fn current(mut l: Limits, now: u64) -> Limits {
    l.meters.retain(|m| m.resets_at.is_none_or(|r| r > now));
    l
}

/// Whether Anthropic is to be asked now: only while *Model limits* is on, and at most every 5 minutes.
pub fn due(on: bool, last: Option<u64>, now: u64) -> bool {
    on && last.is_none_or(|l| now >= l + ASK_EVERY)
}

/// `2026-11-04T14:30:00Z`, with fractions or an offset, as seconds since 1970.
pub fn iso(s: &str) -> Option<u64> {
    let s = s.trim();
    let num = |a: usize, b: usize| s.get(a..b)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, sec) = (num(0, 4)?, num(5, 7)?, num(8, 10)?, num(11, 13)?, num(14, 16)?, num(17, 19)?);
    let rest = s.get(19..)?.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = match rest {
        "" | "Z" | "z" => 0,
        r if r.len() >= 6 && (r.starts_with('+') || r.starts_with('-')) => {
            let o = r[1..3].parse::<i64>().ok()? * 3600 + r[4..6].parse::<i64>().ok()? * 60;
            if r.starts_with('-') { -o } else { o }
        }
        _ => return None,
    };
    // Days since 1970 of a date in the proleptic Gregorian calendar (Howard Hinnant's days_from_civil).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    u64::try_from(days * 86400 + h * 3600 + mi * 60 + sec - offset).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uc_06_1_the_windows_claude_hands_its_status_line() {
        let l = from_status_line(
            r#"{"t":1793460000,"rate_limits":{"five_hour":{"used_percentage":42,"resets_at":1793470000},"seven_day":{"used_percentage":18.5,"resets_at":1793900000000}}}"#,
        )
        .unwrap();
        assert_eq!(l.at, 1793460000);
        assert_eq!(l.meters[0], Meter { name: "5-hour".into(), percent: 42.0, resets_at: Some(1793470000) });
        assert_eq!((l.meters[1].percent, l.meters[1].resets_at), (18.5, Some(1793900000)), "milliseconds too");
        assert_eq!(from_status_line(r#"{"t":1,"rate_limits":{}}"#), None);
        assert_eq!(from_status_line("not json"), None);
    }

    #[test]
    fn uc_06_3_the_models_weeks_from_anthropic() {
        let body = r#"{"limits":[
            {"kind":"session","group":"session","percent":42,"resets_at":"2026-11-04T14:30:00Z"},
            {"kind":"weekly_all","group":"weekly","percent":18,"resets_at":"2026-11-10T08:00:00.000+01:00"},
            {"kind":"weekly_scoped","group":"weekly","percent":61,"resets_at":"2026-11-10T08:00:00Z","scope":{"model":{"display_name":"Fable"}}},
            {"kind":"weekly_scoped","group":"weekly","percent":5,"scope":{"surface":{"display_name":"Apps"}}}
        ]}"#;
        let l = from_usage(body, 100).unwrap();
        let names: Vec<&str> = l.meters.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["5-hour", "7-day", "Fable"], "a surface's row isn't a model's");
        assert_eq!(l.meters[0].resets_at, iso("2026-11-04T14:30:00Z"));
        assert_eq!(l.meters[1].resets_at, Some(iso("2026-11-10T07:00:00Z").unwrap()));
        let old = from_usage(r#"{"five_hour":{"utilization":7.0,"resets_at":"2026-11-04T14:30:00Z"}}"#, 1).unwrap();
        assert_eq!((old.meters[0].name.as_str(), old.meters[0].percent), ("5-hour", 7.0));
    }

    #[test]
    fn uc_06_3_the_bar_takes_the_newer_windows_and_the_models_weeks() {
        let m = |name: &str, p: f32| Meter { name: name.into(), percent: p, resets_at: None };
        let status = Limits { meters: vec![m("5-hour", 50.0), m("7-day", 20.0)], at: 200 };
        let usage = Limits { meters: vec![m("5-hour", 40.0), m("7-day", 19.0), m("Fable", 61.0)], at: 100 };
        let both = merge(Some(&status), Some(&usage));
        assert_eq!(both.meters, vec![m("5-hour", 50.0), m("7-day", 20.0), m("Fable", 61.0)]);
        assert_eq!(both.at, 200);
        assert_eq!(merge(None, Some(&usage)).meters.len(), 3);
        assert_eq!(merge(None, None), Limits::default());
    }

    #[test]
    fn uc_06_5_a_limit_past_its_reset_is_left_out() {
        let m = |name: &str, r: u64| Meter { name: name.into(), percent: 69.0, resets_at: Some(r) };
        let l = Limits { meters: vec![m("5-hour", 100), m("7-day", 900)], at: 50 };
        assert_eq!(current(l, 500).meters, vec![m("7-day", 900)]);
    }

    #[test]
    fn uc_06_7_each_limit_from_the_newest_source_that_has_it() {
        let m = |name: &str, p: f32| Meter { name: name.into(), percent: p, resets_at: None };
        let mac = Limits { meters: vec![m("5-hour", 3.0), m("7-day", 69.0)], at: 100 };
        let sandbox = Limits { meters: vec![m("7-day", 84.0)], at: 900 };
        let n = newest(&[mac, sandbox]).unwrap();
        assert_eq!(n.meters, vec![m("5-hour", 3.0), m("7-day", 84.0)]);
        assert_eq!(n.at, 900);
        assert_eq!(newest(&[]), None);
    }

    #[test]
    fn nfr_03_2_anthropic_is_asked_only_while_turned_on_and_at_most_every_5_minutes() {
        assert!(!due(false, None, 1000));
        assert!(due(true, None, 1000));
        assert!(!due(true, Some(1000), 1000 + ASK_EVERY - 1));
        assert!(due(true, Some(1000), 1000 + ASK_EVERY));
    }

    #[test]
    fn iso_times() {
        assert_eq!(iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso("2000-03-01T00:00:00Z"), Some(951868800));
        assert_eq!(iso("2026-11-04T14:30:00.123456Z"), iso("2026-11-04T14:30:00Z"));
        assert_eq!(iso("2026-11-04T15:30:00+01:00"), iso("2026-11-04T14:30:00Z"));
        assert_eq!(iso("tomorrow"), None);
    }
}
