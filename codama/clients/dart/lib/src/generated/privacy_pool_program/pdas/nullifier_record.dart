// Auto-generated. Do not edit.
// ignore_for_file: type=lint



import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';


@immutable
class NullifierRecordSeeds {
  const NullifierRecordSeeds({
    required this.nullifier,
  });

  final Address nullifier;
}

/// Finds the program derived address for [NullifierRecord].
Future<(Address, int)> findNullifierRecordPda({
  required NullifierRecordSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'privacy-pool-nullifier',
    getAddressEncoder().encode(seeds.nullifier),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
