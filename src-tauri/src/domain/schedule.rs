use serde::{Deserialize, Serialize};

/// Cron-style schedule for a folder in `Scheduled` mode.
///
/// Days of week: 0 = Sunday, 6 = Saturday. Time is 24-hour `HH:MM`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub enabled: bool,
    pub days_of_week: Vec<u8>,
    pub time: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScheduleRegistration {
    generation: u64,
    schedule: Schedule,
    active: bool,
}

impl ScheduleRegistration {
    pub(crate) fn new(generation: u64, schedule: Schedule) -> Self {
        Self {
            generation,
            schedule,
            active: true,
        }
    }

    pub(crate) fn deactivate(&mut self) {
        self.active = false;
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    pub(crate) fn is_current(&self, generation: u64, schedule: &Schedule) -> bool {
        self.active && self.generation == generation && self.schedule == *schedule
    }

    pub(crate) fn schedule(&self) -> &Schedule {
        &self.schedule
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            enabled: false,
            days_of_week: vec![1, 2, 3, 4, 5], // Mon–Fri
            time: "09:00".to_string(),
        }
    }
}

impl Schedule {
    /// Build a 6-field cron expression (`second minute hour * * day-of-week`).
    ///
    /// Returns `None` if the schedule is invalid.
    pub fn to_cron(&self) -> Option<String> {
        if self.days_of_week.is_empty() || self.days_of_week.iter().any(|day| *day > 6) {
            return None;
        }
        let (h, m) = self.parsed_time()?;
        let mut days = self.days_of_week.to_vec();
        days.sort_unstable();
        days.dedup();
        if days.is_empty() {
            return None;
        }
        let dow = if days.len() == 7 {
            "*".to_string()
        } else {
            const DAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
            days.iter()
                .map(|d| DAY_NAMES[*d as usize])
                .collect::<Vec<_>>()
                .join(",")
        };
        Some(format!("0 {m} {h} * * {dow}"))
    }

    /// Calculate the next wall-clock occurrence after `now`.
    pub fn next_local_run_after(
        &self,
        now: chrono::NaiveDateTime,
    ) -> Option<chrono::NaiveDateTime> {
        if !self.enabled {
            return None;
        }
        let (hour, minute) = self.parsed_time()?;

        for offset in 0..=7 {
            let date = now.date().checked_add_days(chrono::Days::new(offset))?;
            let weekday = chrono::Datelike::weekday(&date).num_days_from_sunday() as u8;
            if !self.days_of_week.contains(&weekday) {
                continue;
            }
            let candidate = date.and_hms_opt(hour, minute, 0)?;
            if candidate > now {
                return Some(candidate);
            }
        }
        None
    }

    fn parsed_time(&self) -> Option<(u32, u32)> {
        let (hour, minute) = self.time.split_once(':')?;
        let hour: u32 = hour.parse().ok()?;
        let minute: u32 = minute.parse().ok()?;
        (hour <= 23 && minute <= 59).then_some((hour, minute))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_cron_for_weekdays_at_9() {
        let s = Schedule {
            enabled: true,
            days_of_week: vec![1, 2, 3, 4, 5],
            time: "09:00".to_string(),
        };
        assert_eq!(
            s.to_cron().as_deref(),
            Some("0 0 9 * * Mon,Tue,Wed,Thu,Fri")
        );
    }

    #[test]
    fn builds_cron_for_all_days() {
        let s = Schedule {
            enabled: true,
            days_of_week: vec![0, 1, 2, 3, 4, 5, 6],
            time: "23:30".to_string(),
        };
        assert_eq!(s.to_cron().as_deref(), Some("0 30 23 * * *"));
    }

    #[test]
    fn rejects_invalid_time() {
        let s = Schedule {
            enabled: true,
            days_of_week: vec![1],
            time: "25:00".to_string(),
        };
        assert_eq!(s.to_cron(), None);
    }

    #[test]
    fn rejects_any_invalid_day_instead_of_silently_dropping_it() {
        let schedule = Schedule {
            enabled: true,
            days_of_week: vec![1, 7],
            time: "09:00".to_string(),
        };
        assert_eq!(schedule.to_cron(), None);
    }

    #[test]
    fn calculates_next_local_occurrence() {
        let schedule = Schedule {
            enabled: true,
            days_of_week: vec![1, 3],
            time: "09:30".to_string(),
        };
        let monday_morning = chrono::NaiveDate::from_ymd_opt(2026, 7, 20)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap();
        let monday_late = monday_morning.date().and_hms_opt(10, 0, 0).unwrap();

        assert_eq!(
            schedule.next_local_run_after(monday_morning),
            monday_morning.date().and_hms_opt(9, 30, 0)
        );
        assert_eq!(
            schedule.next_local_run_after(monday_late),
            chrono::NaiveDate::from_ymd_opt(2026, 7, 22)
                .unwrap()
                .and_hms_opt(9, 30, 0)
        );
    }

    #[test]
    fn stale_schedule_registration_never_becomes_current() {
        let original = Schedule {
            enabled: true,
            days_of_week: vec![1],
            time: "09:00".into(),
        };
        let changed = Schedule {
            time: "10:00".into(),
            ..original.clone()
        };
        let mut registration = ScheduleRegistration::new(7, original.clone());

        assert!(registration.is_current(7, &original));
        assert!(!registration.is_current(6, &original));
        assert!(!registration.is_current(7, &changed));
        registration.deactivate();
        assert!(!registration.is_current(7, &original));
    }
}
