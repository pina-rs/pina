// Auto-generated. Do not edit.
// ignore_for_file: type=lint



import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';


@immutable
class DisclosureRequestSeeds {
  const DisclosureRequestSeeds({
    required this.requester,
    required this.nonce,
  });

  final Address requester;
  final BigInt nonce;
}

/// Finds the program derived address for [DisclosureRequest].
Future<(Address, int)> findDisclosureRequestPda({
  required DisclosureRequestSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'privacy-pool-request',
    getAddressEncoder().encode(seeds.requester),
    getU64Encoder().encode(seeds.nonce),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
