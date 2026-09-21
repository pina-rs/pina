// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';

@immutable
class RoleEntrySeeds {
  const RoleEntrySeeds({required this.registry, required this.roleId});

  final Address registry;
  final BigInt roleId;
}

/// Finds the program derived address for [RoleEntry].
Future<(Address, int)> findRoleEntryPda({
  required RoleEntrySeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'role-entry',
    getAddressEncoder().encode(seeds.registry),
    getU64Encoder().encode(seeds.roleId),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
