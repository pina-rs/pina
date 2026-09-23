// Auto-generated. Do not edit.
// ignore_for_file: type=lint



import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';


@immutable
class NoteCommitmentSeeds {
  const NoteCommitmentSeeds({
    required this.commitment,
  });

  final Address commitment;
}

/// Finds the program derived address for [NoteCommitment].
Future<(Address, int)> findNoteCommitmentPda({
  required NoteCommitmentSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'privacy-pool-note',
    getAddressEncoder().encode(seeds.commitment),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
