// Auto-generated. Do not edit.
// ignore_for_file: type=lint



import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';


@immutable
class AssetVaultSeeds {
  const AssetVaultSeeds({
    required this.mint,
  });

  final Address mint;
}

/// Finds the program derived address for [AssetVault].
Future<(Address, int)> findAssetVaultPda({
  required AssetVaultSeeds seeds,
  required Address programAddress,
}) async {
  final seedValues = <Object>[
    'privacy-pool-asset',
    getAddressEncoder().encode(seeds.mint),
  ];

  return getProgramDerivedAddress(
    programAddress: programAddress,
    seeds: seedValues,
  );
}
