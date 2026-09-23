// Auto-generated. Do not edit.
// ignore_for_file: type=lint



import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';


@immutable
class SpendingLimitSeeds {
  const SpendingLimitSeeds({
    required this.multisig,
    required this.createKey,
  });

  final Address multisig;
  final Address createKey;
}

/// Finds the program derived address for [SpendingLimit].
Future<(Address, int)> findSpendingLimitPda({
  required SpendingLimitSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'spending-limit',
    getAddressEncoder().encode(seeds.multisig),
    getAddressEncoder().encode(seeds.createKey),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
