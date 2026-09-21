//! Collision detection for account discriminators.
//!
//! The lint pass in [`crate::lints::deny_colliding_account_discriminators`]
//! reads rustc data structures and can only run inside the bundled driver,
//! which the coverage job cannot instrument. The decision logic lives here
//! instead, so the host test suite exercises it directly.

use std::collections::BTreeMap;

/// One reported collision: the type that claims a value, the value and width
/// it claims, and the other account types that claim the same pair.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Collision<'a> {
	pub(crate) type_name: &'a str,
	pub(crate) width: u64,
	pub(crate) value: u128,
	pub(crate) others: String,
}

/// Resolve which account types collide on `(width, value)`.
///
/// Pure so it can be tested on the host: the lint pass itself only runs
/// inside the bundled driver, which the coverage job cannot instrument.
/// Only types that implement pina's account traits are considered — events
/// share the `HasDiscriminator` shape but live in the log namespace, so a
/// value shared with an event is not a type-cosplay path.
pub(crate) fn resolve_collisions<'a>(
	discriminators: &'a [(&'a str, u64, u128)],
	account_types: &[&str],
) -> Vec<Collision<'a>> {
	let mut claims: BTreeMap<(u64, u128), Vec<&'a str>> = BTreeMap::new();
	for (type_name, width, value) in discriminators {
		if !account_types.contains(type_name) {
			continue;
		}
		claims.entry((*width, *value)).or_default().push(type_name);
	}

	let mut collisions = Vec::new();
	for ((width, value), group) in claims {
		if group.len() < 2 {
			continue;
		}
		for type_name in &group {
			let others = group
				.iter()
				.filter(|candidate| *candidate != type_name)
				.copied()
				.collect::<Vec<_>>()
				.join(", ");
			collisions.push(Collision {
				type_name,
				width,
				value,
				others,
			});
		}
	}
	collisions
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn colliding_values_report_each_type_with_its_counterpart() {
		let discriminators = [("VaultLedger", 1, 31), ("AdminRegistry", 1, 31)];

		let collisions = resolve_collisions(&discriminators, &["VaultLedger", "AdminRegistry"]);

		assert_eq!(
			collisions,
			vec![
				Collision {
					type_name: "VaultLedger",
					width: 1,
					value: 31,
					others: "AdminRegistry".to_owned(),
				},
				Collision {
					type_name: "AdminRegistry",
					width: 1,
					value: 31,
					others: "VaultLedger".to_owned(),
				},
			]
		);
	}

	#[test]
	fn distinct_values_do_not_collide() {
		let discriminators = [("VaultLedger", 1, 31), ("AdminRegistry", 1, 32)];

		let collisions = resolve_collisions(&discriminators, &["VaultLedger", "AdminRegistry"]);

		assert!(collisions.is_empty());
	}

	#[test]
	fn differing_widths_do_not_collide() {
		// The width is part of the serialized layout, so a u8 31 and a u16 31
		// are different account shapes rather than a substitution.
		let discriminators = [("VaultLedger", 1, 31), ("AdminRegistry", 2, 31)];

		let collisions = resolve_collisions(&discriminators, &["VaultLedger", "AdminRegistry"]);

		assert!(collisions.is_empty());
	}

	#[test]
	fn values_shared_with_non_account_types_do_not_collide() {
		// Events carry `HasDiscriminator` too, but they are logged rather than
		// deserialized, so a shared value is not a cosplay path.
		let discriminators = [("ProgramConfig", 1, 1), ("ProposalStatusEvent", 1, 1)];

		let collisions = resolve_collisions(&discriminators, &["ProgramConfig"]);

		assert!(collisions.is_empty());
	}

	#[test]
	fn three_way_collisions_name_every_other_type() {
		let discriminators = [("A", 1, 7), ("B", 1, 7), ("C", 1, 7)];

		let collisions = resolve_collisions(&discriminators, &["A", "B", "C"]);

		assert_eq!(collisions.len(), 3);
		assert_eq!(collisions[0].others, "B, C");
		assert_eq!(collisions[1].others, "A, C");
		assert_eq!(collisions[2].others, "A, B");
	}
}
