// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_accounts/solana_kit_accounts.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';

@immutable
class ProfileState {
  const ProfileState({
    required this.bump,
    required this.name,
    required this.bio,
    required this.tags,
    required this.favoriteTag,
    required this.active,
  }) : discriminator = 1;

  final int discriminator;
  final int bump;
  final Uint8List name;
  final Uint8List bio;
  final Uint8List tags;
  final BigInt? favoriteTag;
  final bool active;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ProfileState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          bump == other.bump &&
          name == other.name &&
          bio == other.bio &&
          tags == other.tags &&
          favoriteTag == other.favoriteTag &&
          active == other.active;

  @override
  int get hashCode =>
      Object.hash(discriminator, bump, name, bio, tags, favoriteTag, active);

  @override
  String toString() =>
      'ProfileState(discriminator: $discriminator, bump: $bump, name: $name, bio: $bio, tags: $tags, favoriteTag: $favoriteTag, active: $active)';
}

Encoder<ProfileState> getProfileStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('name', fixEncoderSize(getBytesEncoder(), 33, allowTruncation: false)),
    ('bio', fixEncoderSize(getBytesEncoder(), 129, allowTruncation: false)),
    ('tags', fixEncoderSize(getBytesEncoder(), 66, allowTruncation: false)),
    (
      'favoriteTag',
      getNullableEncoder<BigInt>(
        getU64Encoder(),
        noneValue: const ZeroesNoneValue(),
      ),
    ),
    ('active', getBooleanEncoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ProfileState value) => <String, Object?>{
      'discriminator': 1,
      'bump': value.bump,
      'name': value.name,
      'bio': value.bio,
      'tags': value.tags,
      'favoriteTag': value.favoriteTag,
      'active': value.active,
    },
  );
}

Decoder<ProfileState> getProfileStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('name', fixDecoderSize(getBytesDecoder(), 33)),
    ('bio', fixDecoderSize(getBytesDecoder(), 129)),
    ('tags', fixDecoderSize(getBytesDecoder(), 66)),
    (
      'favoriteTag',
      getNullableDecoder<BigInt>(
        getU64Decoder(),
        noneValue: const ZeroesNoneValue(),
      ),
    ),
    ('active', getBooleanDecoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'profileState account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ProfileState, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (
      ProfileState(
        bump: map['bump']! as int,
        name: map['name']! as Uint8List,
        bio: map['bio']! as Uint8List,
        tags: map['tags']! as Uint8List,
        favoriteTag: map['favoriteTag'] as BigInt?,
        active: map['active']! as bool,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<ProfileState>(
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
      VariableSizeDecoder<ProfileState>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ProfileState, ProfileState> getProfileStateCodec() {
  return combineCodec(getProfileStateEncoder(), getProfileStateDecoder());
}

Account<ProfileState> decodeProfileState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getProfileStateDecoder());
}
