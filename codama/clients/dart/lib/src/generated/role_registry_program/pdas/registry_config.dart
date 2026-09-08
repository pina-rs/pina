// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';

@immutable
class RegistryConfigSeeds {
  const RegistryConfigSeeds({required this.admin});

  final Address admin;
}

/// Finds the program derived address for [RegistryConfig].
Future<(Address, int)> findRegistryConfigPda({
  required RegistryConfigSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'registry',
    getAddressEncoder().encode(seeds.admin),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
