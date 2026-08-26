use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub fn stamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("timestamp format")
}
