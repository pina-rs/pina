// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:solana_kit_addresses/solana_kit_addresses.dart';

/// Finds the program derived address for [Authority].
Future<(Address, int)> findAuthorityPda({
  required Address programAddress,
}) async {
  final seedValues = <Object>['cpi-authority'];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
