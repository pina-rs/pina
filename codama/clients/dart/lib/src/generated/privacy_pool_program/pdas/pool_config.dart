// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:solana_kit_addresses/solana_kit_addresses.dart';

/// Finds the program derived address for [PoolConfig].
Future<(Address, int)> findPoolConfigPda({
  required Address programAddress,
}) async {
  final seedValues = <Object>['privacy-pool-config'];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
