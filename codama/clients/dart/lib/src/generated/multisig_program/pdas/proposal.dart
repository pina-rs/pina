// Auto-generated. Do not edit.
// ignore_for_file: type=lint



import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';


@immutable
class ProposalSeeds {
  const ProposalSeeds({
    required this.multisig,
    required this.index,
  });

  final Address multisig;
  final BigInt index;
}

/// Finds the program derived address for [Proposal].
Future<(Address, int)> findProposalPda({
  required ProposalSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'proposal',
    getAddressEncoder().encode(seeds.multisig),
    getU64Encoder().encode(seeds.index),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
