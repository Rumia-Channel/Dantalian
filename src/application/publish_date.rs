pub fn normalize_publish_date(raw: Option<&str>) -> Option<String> {
    let normalized: String = raw?
        .trim()
        .chars()
        .map(|ch| match ch {
            '０'..='９' => char::from_u32(ch as u32 - '０' as u32 + '0' as u32).unwrap(),
            '－' => '-',
            '／' => '/',
            '．' => '.',
            _ => ch,
        })
        .collect();
    let s = normalized.trim();
    if s.is_empty() {
        return None;
    }

    if s.chars().all(|ch| ch.is_ascii_digit()) {
        return match s.len() {
            4 => normalize_date_parts(&[&s[0..4]]),
            6 => normalize_date_parts(&[&s[0..4], &s[4..6]]),
            8 => normalize_date_parts(&[&s[0..4], &s[4..6], &s[6..8]]),
            _ => None,
        };
    }

    let separated = s.replace('年', "-").replace('月', "-").replace('日', "");
    let parts: Vec<&str> = separated
        .split(|ch| matches!(ch, '-' | '/' | '.'))
        .filter(|part| !part.is_empty())
        .collect();
    normalize_date_parts(&parts)
}

pub fn normalize_publish_date_input(raw: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = raw.map(str::trim) else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    normalize_publish_date(Some(value))
        .map(Some)
        .ok_or_else(|| "日付は YYYY-MM-DD または YYYY-MM-NN 形式で入力してください".to_string())
}

fn normalize_date_parts(parts: &[&str]) -> Option<String> {
    let year = parse_year(parts.first().copied()?)?;
    match parts.len() {
        1 => Some(format!("{:04}-NN-NN", year)),
        2 => {
            let month = parts[1].to_ascii_uppercase();
            if month == "NN" {
                Some(format!("{:04}-NN-NN", year))
            } else {
                Some(format!("{:04}-{:02}-NN", year, parse_month(&month)?))
            }
        }
        3 => {
            let month = parts[1].to_ascii_uppercase();
            let day = parts[2].to_ascii_uppercase();
            if month == "NN" && day == "NN" {
                return Some(format!("{:04}-NN-NN", year));
            }
            let month = parse_month(&month)?;
            if day == "NN" {
                return Some(format!("{:04}-{:02}-NN", year, month));
            }
            let day = day.parse::<u32>().ok()?;
            chrono::NaiveDate::from_ymd_opt(year, month, day)
                .map(|date| date.format("%Y-%m-%d").to_string())
        }
        _ => None,
    }
}

fn parse_year(value: &str) -> Option<i32> {
    if value.len() != 4 || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let year = value.parse::<i32>().ok()?;
    (1900..=2999).contains(&year).then_some(year)
}

fn parse_month(value: &str) -> Option<u32> {
    let month = value.parse::<u32>().ok()?;
    (1..=12).contains(&month).then_some(month)
}

#[cfg(test)]
mod tests {
    use super::{normalize_publish_date, normalize_publish_date_input};

    #[test]
    fn normalizes_ndl_month_only_dates() {
        assert_eq!(
            normalize_publish_date(Some("2025.11")),
            Some("2025-11-NN".to_string())
        );
        assert_eq!(
            normalize_publish_date(Some("２０２５年１１月")),
            Some("2025-11-NN".to_string())
        );
    }

    #[test]
    fn normalizes_full_and_year_only_dates() {
        assert_eq!(
            normalize_publish_date(Some("2024/5/2")),
            Some("2024-05-02".to_string())
        );
        assert_eq!(
            normalize_publish_date(Some("2024")),
            Some("2024-NN-NN".to_string())
        );
        assert_eq!(
            normalize_publish_date(Some("2024-05-NN")),
            Some("2024-05-NN".to_string())
        );
    }

    #[test]
    fn rejects_invalid_date_input() {
        assert!(normalize_publish_date_input(Some("2024-13")).is_err());
        assert!(normalize_publish_date_input(Some("2024-02-31")).is_err());
        assert_eq!(normalize_publish_date_input(Some("  ")), Ok(None));
    }
}
