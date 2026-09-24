// Auto-generated. Do not edit.
// ignore_for_file: type=lint



import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';


@immutable
class VerifyingKeyAccountSeeds {
  const VerifyingKeyAccountSeeds({
    required this.slot,
  });

  final BigInt slot;
}

/// Finds the program derived address for [VerifyingKeyAccount].
Future<(Address, int)> findVerifyingKeyAccountPda({
  required VerifyingKeyAccountSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'privacy-pool-vkey',
    getU64Encoder().encode(seeds.slot),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
