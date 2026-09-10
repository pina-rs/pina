// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';

@immutable
class VestingSeeds {
  const VestingSeeds({
    required this.admin,
    required this.beneficiary,
    required this.mint,
  });

  final Address admin;
  final Address beneficiary;
  final Address mint;
}

/// Finds the program derived address for [Vesting].
Future<(Address, int)> findVestingPda({
  required VestingSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'vesting',
    getAddressEncoder().encode(seeds.admin),
    getAddressEncoder().encode(seeds.beneficiary),
    getAddressEncoder().encode(seeds.mint),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
