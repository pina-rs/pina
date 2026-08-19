// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';

@immutable
class ProfileSeeds {
  const ProfileSeeds({required this.authority});

  final Address authority;
}

/// Finds the program derived address for [Profile].
Future<(Address, int)> findProfilePda({
  required ProfileSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'profile',
    getAddressEncoder().encode(seeds.authority),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
