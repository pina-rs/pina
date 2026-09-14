// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';

@immutable
class JournalSeeds {
  const JournalSeeds({required this.authority});

  final Address authority;
}

/// Finds the program derived address for [Journal].
Future<(Address, int)> findJournalPda({
  required JournalSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'compact-journal',
    getAddressEncoder().encode(seeds.authority),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
