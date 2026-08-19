// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

@immutable
class UpdateProfileInstructionData {
  const UpdateProfileInstructionData({required this.name, required this.bio})
    : discriminator = 1;

  final int discriminator;
  final Uint8List name;
  final Uint8List bio;
}

Encoder<UpdateProfileInstructionData> getUpdateProfileInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('name', fixEncoderSize(getBytesEncoder(), 33, allowTruncation: false)),
    ('bio', fixEncoderSize(getBytesEncoder(), 129, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (UpdateProfileInstructionData value) => <String, Object?>{
      'discriminator': 1,
      'name': value.name,
      'bio': value.bio,
    },
  );
}

Decoder<UpdateProfileInstructionData> getUpdateProfileInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('name', fixDecoderSize(getBytesDecoder(), 33)),
    ('bio', fixDecoderSize(getBytesDecoder(), 129)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'updateProfile instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (UpdateProfileInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      UpdateProfileInstructionData(
        name: map['name']! as Uint8List,
        bio: map['bio']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<UpdateProfileInstructionData>(
        fixedSize: structDecoder.fixedSize,
        read: (bytes, offset) {
          final bytesLength = bytes.length - offset;
          if (bytesLength != structDecoder.fixedSize) {
            throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
          }
          return readTopLevel(bytes, offset);
        },
      ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<UpdateProfileInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<UpdateProfileInstructionData, UpdateProfileInstructionData>
getUpdateProfileInstructionDataCodec() {
  return combineCodec(
    getUpdateProfileInstructionDataEncoder(),
    getUpdateProfileInstructionDataDecoder(),
  );
}

/// Creates a [UpdateProfile] instruction.
Instruction getUpdateProfileInstruction({
  required Address programAddress,
  required Address authority,
  required Address profile,
  required Uint8List name,
  required Uint8List bio,
}) {
  final instructionData = UpdateProfileInstructionData(name: name, bio: bio);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      AccountMeta(address: profile, role: AccountRole.writable),
    ],
    data: getUpdateProfileInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [UpdateProfile] instruction from raw instruction data.
UpdateProfileInstructionData parseUpdateProfileInstruction(
  Instruction instruction,
) {
  return getUpdateProfileInstructionDataDecoder().decode(instruction.data!);
}
