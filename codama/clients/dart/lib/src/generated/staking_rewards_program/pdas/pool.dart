// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';

@immutable
class PoolSeeds {
  const PoolSeeds({required this.stakeMint, required this.rewardMint});

  final Address stakeMint;
  final Address rewardMint;
}

/// Finds the program derived address for [Pool].
Future<(Address, int)> findPoolPda({
  required PoolSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'pool',
    getAddressEncoder().encode(seeds.stakeMint),
    getAddressEncoder().encode(seeds.rewardMint),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
