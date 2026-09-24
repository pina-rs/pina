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
class NoteCommitment {
  const NoteCommitment({
    required this.bump,
    required this.commitment,
    required this.viewPubkey,
    required this.envelopeLen,
    required this.envelope,
    required this.shares,
  }) :
      discriminator = 8,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Uint8List commitment;
  final Uint8List viewPubkey;
  final int envelopeLen;
  final Uint8List envelope;
  final Uint8List shares;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is NoteCommitment &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          commitment == other.commitment &&
          viewPubkey == other.viewPubkey &&
          envelopeLen == other.envelopeLen &&
          envelope == other.envelope &&
          shares == other.shares;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, bump, commitment, viewPubkey, envelopeLen, envelope, shares);

  @override
  String toString() => 'NoteCommitment(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, commitment: $commitment, viewPubkey: $viewPubkey, envelopeLen: $envelopeLen, envelope: $envelope, shares: $shares)';
}


Encoder<NoteCommitment> getNoteCommitmentEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('commitment', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('viewPubkey', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('envelopeLen', getU8Encoder()),
    ('envelope', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('shares', fixEncoderSize(getBytesEncoder(), 144, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (NoteCommitment value) => <String, Object?>{
      'discriminator': 8,
      'migrationVersion': 0,
      'bump': value.bump,
      'commitment': value.commitment,
      'viewPubkey': value.viewPubkey,
      'envelopeLen': value.envelopeLen,
      'envelope': value.envelope,
      'shares': value.shares,
    },
  );
}

Decoder<NoteCommitment> getNoteCommitmentDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('commitment', fixDecoderSize(getBytesDecoder(), 32)),
    ('viewPubkey', fixDecoderSize(getBytesDecoder(), 32)),
    ('envelopeLen', getU8Decoder()),
    ('envelope', fixDecoderSize(getBytesDecoder(), 128)),
    ('shares', fixDecoderSize(getBytesDecoder(), 144)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'noteCommitment account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (NoteCommitment, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(8),
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
      NoteCommitment(
      bump: map['bump']! as int,
      commitment: map['commitment']! as Uint8List,
      viewPubkey: map['viewPubkey']! as Uint8List,
      envelopeLen: map['envelopeLen']! as int,
      envelope: map['envelope']! as Uint8List,
      shares: map['shares']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<NoteCommitment>(
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
      VariableSizeDecoder<NoteCommitment>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<NoteCommitment, NoteCommitment> getNoteCommitmentCodec() {
  return combineCodec(getNoteCommitmentEncoder(), getNoteCommitmentDecoder());
}

Account<NoteCommitment> decodeNoteCommitment(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getNoteCommitmentDecoder());
}

/// The account schema version this client was generated from.
const int noteCommitmentMigrationVersion = 0;

/// Cheap envelope check for fetched `NoteCommitment` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool noteCommitmentNeedsMigration(List<int> data) {
	if (data.length < 2) {
		return false;
	}
	if (data[0] != 8) {
		return false;
	}
	return data[1] < 0;
}
