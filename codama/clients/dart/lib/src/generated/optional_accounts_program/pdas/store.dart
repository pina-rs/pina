// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';

@immutable
class StoreSeeds {
  const StoreSeeds({required this.authority});

  final Address authority;
}

/// Finds the program derived address for [Store].
Future<(Address, int)> findStorePda({
  required StoreSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'store',
    getAddressEncoder().encode(seeds.authority),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
