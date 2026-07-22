use serde::{Deserialize, Serialize};

/// Cron-style schedule for a folder in `Scheduled` mode.
///
/// Days of week: 0 = Sunday, 6 = Saturday. Time is 24-hour `HH:MM`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub enabled: bool,
    pub days_of_week: Vec<u8>,
    pub time: String,
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
    /// Build a 5-field cron expression (`minute hour * * day-of-week`).
    ///
    /// Returns `None` if the schedule is invalid.
    pub fn to_cron(&self) -> Option<String> {
        if self.days_of_week.is_empty() {
            return None;
        }
        let (h, m) = self.time.split_once(':')?;
        let h: u32 = h.parse().ok()?;
        let m: u32 = m.parse().ok()?;
        if h > 23 || m > 59 {
            return None;
        }
        let mut days: Vec<u8> = self
            .days_of_week
            .iter()
            .copied()
            .filter(|d| *d <= 6)
            .collect();
        days.sort_unstable();
        days.dedup();
        if days.is_empty() {
            return None;
        }
        let dow = if days.len() == 7 {
            "*".to_string()
        } else {
            days.iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        Some(format!("{m} {h} * * {dow}"))
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
        assert_eq!(s.to_cron().as_deref(), Some("0 9 * * 1,2,3,4,5"));
    }

    #[test]
    fn builds_cron_for_all_days() {
        let s = Schedule {
            enabled: true,
            days_of_week: vec![0, 1, 2, 3, 4, 5, 6],
            time: "23:30".to_string(),
        };
        assert_eq!(s.to_cron().as_deref(), Some("30 23 * * *"));
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
}
