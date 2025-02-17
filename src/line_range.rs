use crate::error::*;

#[derive(Debug, Clone)]
pub struct LineRange {
    lower: usize,
    upper: usize,
}

impl Default for LineRange {
    fn default() -> LineRange {
        LineRange {
            lower: usize::MIN,
            upper: usize::MAX,
        }
    }
}

impl LineRange {
    pub fn new(from: usize, to: usize) -> Self {
        LineRange {
            lower: from,
            upper: to,
        }
    }

    pub fn from(range_raw: &str) -> Result<LineRange> {
        LineRange::parse_range(range_raw)
    }

    fn parse_range(range_raw: &str) -> Result<LineRange> {
        let mut new_range = LineRange::default();

        if range_raw.bytes().next().ok_or("Empty line range")? == b':' {
            new_range.upper = range_raw[1..].parse()?;
            return Ok(new_range);
        } else if range_raw.bytes().last().ok_or("Empty line range")? == b':' {
            new_range.lower = range_raw[..range_raw.len() - 1].parse()?;
            return Ok(new_range);
        }

        let line_numbers: Vec<&str> = range_raw.split(':').collect();
        match line_numbers.len() {
            1 => {
                new_range.lower = line_numbers[0].parse()?;
                new_range.upper = new_range.lower;
                Ok(new_range)
            }
            2 => {
                new_range.lower = line_numbers[0].parse()?;
                let first_byte = line_numbers[1].bytes().next();

                new_range.upper = if first_byte == Some(b'+') {
                    let more_lines = &line_numbers[1][1..]
                        .parse()
                        .map_err(|_| "Invalid character after +")?;
                    new_range.lower.saturating_add(*more_lines)
                } else if first_byte == Some(b'-') {
                    // this will prevent values like "-+5" even though "+5" is valid integer
                    if line_numbers[1][1..].bytes().next() == Some(b'+') {
                        return Err("Invalid character after -".into());
                    }
                    let prior_lines = &line_numbers[1][1..]
                        .parse()
                        .map_err(|_| "Invalid character after -")?;
                    let prev_lower = new_range.lower;
                    new_range.lower = new_range.lower.saturating_sub(*prior_lines);
                    prev_lower
                } else {
                    line_numbers[1].parse()?
                };

                Ok(new_range)
            }
            _ => Err(
                "Line range contained more than one ':' character. Expected format: 'N' or 'N:M'"
                    .into(),
            ),
        }
    }

    pub(crate) fn is_inside(&self, line: usize) -> bool {
        line >= self.lower && line <= self.upper
    }
}

#[test]
fn test_parse_full() {
    let range = LineRange::from("40:50").expect("Shouldn't fail on test!");
    assert_eq!(40, range.lower);
    assert_eq!(50, range.upper);
}

#[test]
fn test_parse_partial_min() {
    let range = LineRange::from(":50").expect("Shouldn't fail on test!");
    assert_eq!(usize::MIN, range.lower);
    assert_eq!(50, range.upper);
}

#[test]
fn test_parse_partial_max() {
    let range = LineRange::from("40:").expect("Shouldn't fail on test!");
    assert_eq!(40, range.lower);
    assert_eq!(usize::MAX, range.upper);
}

#[test]
fn test_parse_single() {
    let range = LineRange::from("40").expect("Shouldn't fail on test!");
    assert_eq!(40, range.lower);
    assert_eq!(40, range.upper);
}

#[test]
fn test_parse_fail() {
    let range = LineRange::from("40:50:80");
    assert!(range.is_err());
    let range = LineRange::from("40::80");
    assert!(range.is_err());
    let range = LineRange::from(":40:");
    assert!(range.is_err());
}

#[test]
fn test_parse_plus() {
    let range = LineRange::from("40:+10").expect("Shouldn't fail on test!");
    assert_eq!(40, range.lower);
    assert_eq!(50, range.upper);
}

#[test]
fn test_parse_plus_overflow() {
    let range = LineRange::from(&format!("{}:+1", usize::MAX)).expect("Shouldn't fail on test!");
    assert_eq!(usize::MAX, range.lower);
    assert_eq!(usize::MAX, range.upper);
}

#[test]
fn test_parse_plus_fail() {
    let range = LineRange::from("40:+z");
    assert!(range.is_err());
    let range = LineRange::from("40:+-10");
    assert!(range.is_err());
    let range = LineRange::from("40:+");
    assert!(range.is_err());
}

#[test]
fn test_parse_minus_success() {
    let range = LineRange::from("40:-10").expect("Shouldn't fail on test!");
    assert_eq!(30, range.lower);
    assert_eq!(40, range.upper);
}

#[test]
fn test_parse_minus_edge_cases_success() {
    let range = LineRange::from("5:-4").expect("Shouldn't fail on test!");
    assert_eq!(1, range.lower);
    assert_eq!(5, range.upper);
    let range = LineRange::from("5:-5").expect("Shouldn't fail on test!");
    assert_eq!(0, range.lower);
    assert_eq!(5, range.upper);
    let range = LineRange::from("5:-100").expect("Shouldn't fail on test!");
    assert_eq!(0, range.lower);
    assert_eq!(5, range.upper);
}

#[test]
fn test_parse_minus_fail() {
    let range = LineRange::from("40:-z");
    assert!(range.is_err());
    let range = LineRange::from("40:-+10");
    assert!(range.is_err());
    let range = LineRange::from("40:-");
    assert!(range.is_err());
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RangeCheckResult {
    // Within one of the given ranges
    InRange,

    // Before the first range or within two ranges
    BeforeOrBetweenRanges,

    // Line number is outside of all ranges and larger than the last range.
    AfterLastRange,
}

#[derive(Debug, Clone)]
pub struct LineRanges {
    ranges: Vec<LineRange>,
    largest_upper_bound: usize,
}

impl LineRanges {
    pub fn none() -> LineRanges {
        LineRanges::from(vec![])
    }

    pub fn all() -> LineRanges {
        LineRanges::from(vec![LineRange::default()])
    }

    pub fn from(ranges: Vec<LineRange>) -> LineRanges {
        let largest_upper_bound = ranges.iter().map(|r| r.upper).max().unwrap_or(usize::MAX);
        LineRanges {
            ranges,
            largest_upper_bound,
        }
    }

    pub(crate) fn check(&self, line: usize) -> RangeCheckResult {
        if self.ranges.iter().any(|r| r.is_inside(line)) {
            RangeCheckResult::InRange
        } else if line < self.largest_upper_bound {
            RangeCheckResult::BeforeOrBetweenRanges
        } else {
            RangeCheckResult::AfterLastRange
        }
    }
}

impl Default for LineRanges {
    fn default() -> Self {
        Self::all()
    }
}

#[derive(Debug, Clone)]
pub struct HighlightedLineRanges(pub LineRanges);

impl Default for HighlightedLineRanges {
    fn default() -> Self {
        HighlightedLineRanges(LineRanges::none())
    }
}

#[cfg(test)]
fn ranges(rs: &[&str]) -> LineRanges {
    LineRanges::from(rs.iter().map(|r| LineRange::from(r).unwrap()).collect())
}

#[test]
fn test_ranges_simple() {
    let ranges = ranges(&["3:8"]);

    assert_eq!(RangeCheckResult::BeforeOrBetweenRanges, ranges.check(2));
    assert_eq!(RangeCheckResult::InRange, ranges.check(5));
    assert_eq!(RangeCheckResult::AfterLastRange, ranges.check(9));
}

#[test]
fn test_ranges_advanced() {
    let ranges = ranges(&["3:8", "11:20", "25:30"]);

    assert_eq!(RangeCheckResult::BeforeOrBetweenRanges, ranges.check(2));
    assert_eq!(RangeCheckResult::InRange, ranges.check(5));
    assert_eq!(RangeCheckResult::BeforeOrBetweenRanges, ranges.check(9));
    assert_eq!(RangeCheckResult::InRange, ranges.check(11));
    assert_eq!(RangeCheckResult::BeforeOrBetweenRanges, ranges.check(22));
    assert_eq!(RangeCheckResult::InRange, ranges.check(28));
    assert_eq!(RangeCheckResult::AfterLastRange, ranges.check(31));
}

#[test]
fn test_ranges_open_low() {
    let ranges = ranges(&["3:8", ":5"]);

    assert_eq!(RangeCheckResult::InRange, ranges.check(1));
    assert_eq!(RangeCheckResult::InRange, ranges.check(3));
    assert_eq!(RangeCheckResult::InRange, ranges.check(7));
    assert_eq!(RangeCheckResult::AfterLastRange, ranges.check(9));
}

#[test]
fn test_ranges_open_high() {
    let ranges = ranges(&["3:", "2:5"]);

    assert_eq!(RangeCheckResult::BeforeOrBetweenRanges, ranges.check(1));
    assert_eq!(RangeCheckResult::InRange, ranges.check(3));
    assert_eq!(RangeCheckResult::InRange, ranges.check(5));
    assert_eq!(RangeCheckResult::InRange, ranges.check(9));
}

#[test]
fn test_ranges_all() {
    let ranges = LineRanges::all();

    assert_eq!(RangeCheckResult::InRange, ranges.check(1));
}

#[test]
fn test_ranges_none() {
    let ranges = LineRanges::none();

    assert_ne!(RangeCheckResult::InRange, ranges.check(1));
}
