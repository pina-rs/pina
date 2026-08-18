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
class TodoState {
  const TodoState({
    required this.owner,
    required this.bump,
    required this.completed,
    required this.digest,
  }) : discriminator = 1;

  final int discriminator;
  final Address owner;
  final int bump;
  final bool completed;
  final Uint8List digest;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is TodoState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          owner == other.owner &&
          bump == other.bump &&
          completed == other.completed &&
          digest == other.digest;

  @override
  int get hashCode =>
      Object.hash(discriminator, owner, bump, completed, digest);

  @override
  String toString() =>
      'TodoState(discriminator: $discriminator, owner: $owner, bump: $bump, completed: $completed, digest: $digest)';
}

Encoder<TodoState> getTodoStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('owner', getAddressEncoder()),
    ('bump', getU8Encoder()),
    ('completed', getBooleanEncoder()),
    ('digest', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (TodoState value) => <String, Object?>{
      'discriminator': 1,
      'owner': value.owner,
      'bump': value.bump,
      'completed': value.completed,
      'digest': value.digest,
    },
  );
}

Decoder<TodoState> getTodoStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('owner', getAddressDecoder()),
    ('bump', getU8Decoder()),
    ('completed', getBooleanDecoder()),
    ('digest', fixDecoderSize(getBytesDecoder(), 32)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'todoState account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (TodoState, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (
      TodoState(
        owner: map['owner']! as Address,
        bump: map['bump']! as int,
        completed: map['completed']! as bool,
        digest: map['digest']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<TodoState>(
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
      VariableSizeDecoder<TodoState>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<TodoState, TodoState> getTodoStateCodec() {
  return combineCodec(getTodoStateEncoder(), getTodoStateDecoder());
}

Account<TodoState> decodeTodoState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getTodoStateDecoder());
}
