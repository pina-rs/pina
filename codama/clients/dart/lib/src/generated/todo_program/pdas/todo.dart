// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';

@immutable
class TodoSeeds {
  const TodoSeeds({required this.owner});

  final Address owner;
}

/// Finds the program derived address for [Todo].
Future<(Address, int)> findTodoPda({
  required TodoSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>['todo', getAddressEncoder().encode(seeds.owner)];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
