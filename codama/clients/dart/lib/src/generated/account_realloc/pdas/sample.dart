// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';

@immutable
class SampleSeeds {
  const SampleSeeds({required this.authority});

  final Address authority;
}

/// Finds the program derived address for [Sample].
Future<(Address, int)> findSamplePda({
  required SampleSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'sample',
    getAddressEncoder().encode(seeds.authority),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
