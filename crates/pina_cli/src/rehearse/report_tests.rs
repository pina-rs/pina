use super::*;

#[test]
fn unit_statistics_use_the_lower_median() {
	assert_eq!(
		unit_stats(&mut [9, 1, 5, 7]),
		UnitStats {
			min: 1,
			median: 5,
			max: 9,
		}
	);
	assert_eq!(
		unit_stats(&mut [4]),
		UnitStats {
			min: 4,
			median: 4,
			max: 4,
		}
	);
}

#[test]
fn text_spellings_match_the_json_document() {
	fn json(value: impl Serialize) -> String {
		serde_json::to_value(value)
			.ok()
			.and_then(|value| value.as_str().map(str::to_owned))
			.unwrap_or_default()
	}

	for status in [
		RehearsalStatus::Unchanged,
		RehearsalStatus::CuChanged,
		RehearsalStatus::StateChanged,
		RehearsalStatus::OutcomeChanged,
		RehearsalStatus::Skipped,
	] {
		assert_eq!(json(status), status.as_str());
	}

	for reason in [
		SkipReason::Unavailable,
		SkipReason::Undecodable,
		SkipReason::NotProfiled,
		SkipReason::FailedInBoth,
	] {
		assert_eq!(json(reason), reason.as_str());
	}
}
