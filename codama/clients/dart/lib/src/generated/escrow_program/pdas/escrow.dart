// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';

@immutable
class EscrowSeeds {
  const EscrowSeeds({required this.maker, required this.seed});

  final Address maker;
  final BigInt seed;
}

/// Finds the program derived address for [Escrow].
Future<(Address, int)> findEscrowPda({
  required EscrowSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'escrow',
    getAddressEncoder().encode(seeds.maker),
    getU64Encoder().encode(seeds.seed),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
