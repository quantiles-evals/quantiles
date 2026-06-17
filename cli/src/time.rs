/// Return the current UTC time as a strongly-typed [`OffsetDateTime`].
#[must_use]
pub fn now_utc() -> time::OffsetDateTime {
    time::OffsetDateTime::now_utc()
}

/// Format an [`OffsetDateTime`] to match the project's display convention
/// (`YYYY-MM-DDTHH:MM:SS.ffffffZ`).
///
/// # Panics
///
/// Panics only if the format description itself is malformed, which is a
/// compile-time invariant and can never happen in practice.
#[must_use]
pub fn format_utc(dt: time::OffsetDateTime) -> String {
    let format = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]Z"
    );
    dt.format(format)
        .expect("valid time format should always format successfully")
}

/// Format a [`time::Duration`] in a human-friendly way.
#[must_use]
pub fn format_duration(dur: time::Duration) -> String {
    format!("{:.3}s", dur.as_seconds_f64()).to_string()
}
