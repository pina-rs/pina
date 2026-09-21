// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:solana_kit_addresses/solana_kit_addresses.dart';

/// Finds the program derived address for [PoolVault].
Future<(Address, int)> findPoolVaultPda({
  required Address programAddress,
}) async {
  final seedValues = <Object>['privacy-pool-vault'];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
