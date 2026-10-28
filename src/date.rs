//! Dates in date fields: saved as `2026-10-28`, which Obsidian reads as a date too, and
//! picked from a month's calendar.

use chrono::{Datelike, Days, Local, Months, NaiveDate};

/// Today, in the computer's time zone, so a session that runs past midnight UTC still
/// gets the day it was played on.
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// The date in `text`, if it is one written the way date fields save them: `2026-10-28`.
pub fn parse(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").ok()
}

/// A date the way date fields save it.
pub fn format(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// A date the way people read it, like "Wednesday, October 28, 2026".
pub fn long(date: NaiveDate) -> String {
    date.format("%A, %B %-d, %Y").to_string()
}

/// The month a calendar shows, by its first day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Month(NaiveDate);

impl Month {
    /// The month `date` is in.
    pub fn of(date: NaiveDate) -> Self {
        Month(date.with_day(1).unwrap_or(date))
    }

    /// The month `count` months after this one, or before it if `count` is negative.
    pub fn add(self, count: i32) -> Self {
        let months = Months::new(count.unsigned_abs());
        let first = if count < 0 {
            self.0.checked_sub_months(months)
        } else {
            self.0.checked_add_months(months)
        };
        first.map_or(self, Month)
    }

    /// The month's name and year, like "October 2026".
    pub fn title(self) -> String {
        self.0.format("%B %Y").to_string()
    }

    /// The month's weeks, Sunday first, with `None` for the days before the 1st and after
    /// the last day.
    pub fn weeks(self) -> Vec<[Option<NaiveDate>; 7]> {
        let blanks = self.0.weekday().num_days_from_sunday() as usize;
        let mut days = vec![None; blanks];
        let mut day = self.0;
        while day.month() == self.0.month() {
            days.push(Some(day));
            day = day + Days::new(1);
        }
        days.resize(days.len().div_ceil(7) * 7, None);
        days.chunks(7)
            .map(|week| week.try_into().expect("weeks have 7 days"))
            .collect()
    }
}

/// The names of the days, as the calendar's columns show them.
pub const WEEKDAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn reads_and_writes_dates() {
        assert_eq!(parse(" 2026-10-28 "), Some(day(2026, 10, 28)));
        assert_eq!(parse("Stardate 41153.7"), None);
        assert_eq!(parse("2026-02-30"), None);
        assert_eq!(format(day(812, 3, 4)), "0812-03-04");
        assert_eq!(long(day(2026, 10, 28)), "Wednesday, October 28, 2026");
    }

    #[test]
    fn lays_out_a_month() {
        let october = Month::of(day(2026, 10, 28));
        assert_eq!(october.title(), "October 2026");
        let weeks = october.weeks();
        // October 2026 starts on a Thursday and ends on a Saturday.
        assert_eq!(weeks.len(), 5);
        assert_eq!(weeks[0][..4], [None; 4]);
        assert_eq!(weeks[0][4], Some(day(2026, 10, 1)));
        assert_eq!(weeks[4][6], Some(day(2026, 10, 31)));

        assert_eq!(october.add(3).title(), "January 2027");
        assert_eq!(october.add(-10).title(), "December 2025");
    }
}
