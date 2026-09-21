// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';

@immutable
class PositionSeeds {
  const PositionSeeds({required this.pool, required this.owner});

  final Address pool;
  final Address owner;
}

/// Finds the program derived address for [Position].
Future<(Address, int)> findPositionPda({
  required PositionSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'position',
    getAddressEncoder().encode(seeds.pool),
    getAddressEncoder().encode(seeds.owner),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
