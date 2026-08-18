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
class RegistryConfig {
  const RegistryConfig({
    required this.admin,
    required this.roleCount,
    required this.bump,
  }) : discriminator = 1;

  final int discriminator;
  final Address admin;
  final BigInt roleCount;
  final int bump;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is RegistryConfig &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          admin == other.admin &&
          roleCount == other.roleCount &&
          bump == other.bump;

  @override
  int get hashCode => Object.hash(discriminator, admin, roleCount, bump);

  @override
  String toString() =>
      'RegistryConfig(discriminator: $discriminator, admin: $admin, roleCount: $roleCount, bump: $bump)';
}

Encoder<RegistryConfig> getRegistryConfigEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('admin', getAddressEncoder()),
    ('roleCount', getU64Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RegistryConfig value) => <String, Object?>{
      'discriminator': 1,
      'admin': value.admin,
      'roleCount': value.roleCount,
      'bump': value.bump,
    },
  );
}

Decoder<RegistryConfig> getRegistryConfigDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('admin', getAddressDecoder()),
    ('roleCount', getU64Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'registryConfig account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (RegistryConfig, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (
      RegistryConfig(
        admin: map['admin']! as Address,
        roleCount: map['roleCount']! as BigInt,
        bump: map['bump']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RegistryConfig>(
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
      VariableSizeDecoder<RegistryConfig>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RegistryConfig, RegistryConfig> getRegistryConfigCodec() {
  return combineCodec(getRegistryConfigEncoder(), getRegistryConfigDecoder());
}

Account<RegistryConfig> decodeRegistryConfig(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getRegistryConfigDecoder());
}
