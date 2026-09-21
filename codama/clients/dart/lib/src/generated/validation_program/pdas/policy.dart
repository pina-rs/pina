// Auto-generated. Do not edit.
// ignore_for_file: type=lint



import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';


@immutable
class PolicySeeds {
  const PolicySeeds({
    required this.authority,
  });

  final Address authority;
}

/// Finds the program derived address for [Policy].
Future<(Address, int)> findPolicyPda({
  required PolicySeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'validation-policy',
    getAddressEncoder().encode(seeds.authority),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
