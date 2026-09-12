// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';

@immutable
class CounterSeeds {
  const CounterSeeds({required this.authority});

  final Address authority;
}

/// Finds the program derived address for [Counter].
Future<(Address, int)> findCounterPda({
  required CounterSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'counter',
    getAddressEncoder().encode(seeds.authority),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
