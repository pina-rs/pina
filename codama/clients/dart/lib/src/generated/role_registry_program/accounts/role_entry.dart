// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_accounts/solana_kit_accounts.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';


@immutable
class RoleEntry {
  const RoleEntry({
    required this.registry,
    required this.roleId,
    required this.grantee,
    required this.permissions,
    required this.active,
    required this.bump,
  }) :
      discriminator = 2,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final Address registry;
  final BigInt roleId;
  final Address grantee;
  final BigInt permissions;
  final bool active;
  final int bump;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is RoleEntry &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          registry == other.registry &&
          roleId == other.roleId &&
          grantee == other.grantee &&
          permissions == other.permissions &&
          active == other.active &&
          bump == other.bump;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, registry, roleId, grantee, permissions, active, bump);

  @override
  String toString() => 'RoleEntry(discriminator: $discriminator, migrationVersion: $migrationVersion, registry: $registry, roleId: $roleId, grantee: $grantee, permissions: $permissions, active: $active, bump: $bump)';
}


Encoder<RoleEntry> getRoleEntryEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('registry', getAddressEncoder()),
    ('roleId', getU64Encoder()),
    ('grantee', getAddressEncoder()),
    ('permissions', getU64Encoder()),
    ('active', getBooleanEncoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RoleEntry value) => <String, Object?>{
      'discriminator': 2,
      'migrationVersion': 0,
      'registry': value.registry,
      'roleId': value.roleId,
      'grantee': value.grantee,
      'permissions': value.permissions,
      'active': value.active,
      'bump': value.bump,
    },
  );
}

Decoder<RoleEntry> getRoleEntryDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('registry', getAddressDecoder()),
    ('roleId', getU64Decoder()),
    ('grantee', getAddressDecoder()),
    ('permissions', getU64Decoder()),
    ('active', getBooleanDecoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'roleEntry account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (RoleEntry, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(2),
    ).read(bytes, offset + 0);
    final (storedMigrationVersion, _) = getU8Decoder().read(bytes, offset + 1);
    if (storedMigrationVersion != 0) {
      throw StateError(
        storedMigrationVersion < 0
            ? 'migration version mismatch: expected 0, received $storedMigrationVersion (the data predates this client; migrate it by sending a transaction to the program, or decode it with a client generated from an older IDL)'
            : 'migration version mismatch: expected 0, received $storedMigrationVersion (the data was written by a newer program; upgrade this client)',
      );
    }
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      RoleEntry(
      registry: map['registry']! as Address,
      roleId: map['roleId']! as BigInt,
      grantee: map['grantee']! as Address,
      permissions: map['permissions']! as BigInt,
      active: map['active']! as bool,
      bump: map['bump']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RoleEntry>(
        fixedSize: structDecoder.fixedSize,
        read: (bytes, offset) {
          final bytesLength = bytes.length - offset;
          if (bytesLength < structDecoder.fixedSize) {
            throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
          }
          return readTopLevel(bytes, offset);
        },
      ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<RoleEntry>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RoleEntry, RoleEntry> getRoleEntryCodec() {
  return combineCodec(getRoleEntryEncoder(), getRoleEntryDecoder());
}

Account<RoleEntry> decodeRoleEntry(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getRoleEntryDecoder());
}

/// The account schema version this client was generated from.
const int roleEntryMigrationVersion = 0;

/// Cheap envelope check for fetched `RoleEntry` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool roleEntryNeedsMigration(List<int> data) {
	if (data.length < 2) {
		return false;
	}
	if (data[0] != 2) {
		return false;
	}
	return data[1] < 0;
}
