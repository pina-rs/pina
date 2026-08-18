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
  }) : discriminator = 2;

  final int discriminator;
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
          registry == other.registry &&
          roleId == other.roleId &&
          grantee == other.grantee &&
          permissions == other.permissions &&
          active == other.active &&
          bump == other.bump;

  @override
  int get hashCode => Object.hash(
    discriminator,
    registry,
    roleId,
    grantee,
    permissions,
    active,
    bump,
  );

  @override
  String toString() =>
      'RoleEntry(discriminator: $discriminator, registry: $registry, roleId: $roleId, grantee: $grantee, permissions: $permissions, active: $active, bump: $bump)';
}

Encoder<RoleEntry> getRoleEntryEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
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
    ('registry', getAddressDecoder()),
    ('roleId', getU64Decoder()),
    ('grantee', getAddressDecoder()),
    ('permissions', getU64Decoder()),
    ('active', getBooleanDecoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'roleEntry account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (RoleEntry, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
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
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<RoleEntry>(
      fixedSize: structDecoder.fixedSize,
      read: (bytes, offset) {
        final bytesLength = bytes.length - offset;
        if (bytesLength != structDecoder.fixedSize) {
          throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
        }
        return readExact(bytes, offset);
      },
    ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<RoleEntry>(
        read: readExact,
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
