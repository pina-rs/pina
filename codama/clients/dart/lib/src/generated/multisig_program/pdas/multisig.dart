// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';

@immutable
class MultisigSeeds {
  const MultisigSeeds({required this.createKey});

  final Address createKey;
}

/// Finds the program derived address for [Multisig].
Future<(Address, int)> findMultisigPda({
  required MultisigSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'multisig',
    getAddressEncoder().encode(seeds.createKey),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
