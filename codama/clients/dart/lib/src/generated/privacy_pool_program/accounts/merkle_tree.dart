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
class MerkleTree {
  const MerkleTree({
    required this.bump,
    required this.nextLeafIndex,
    required this.nodes,
    required this.rootsLen,
    required this.roots,
  }) :
      discriminator = 3,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final BigInt nextLeafIndex;
  final Uint8List nodes;
  final int rootsLen;
  final Uint8List roots;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is MerkleTree &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          nextLeafIndex == other.nextLeafIndex &&
          nodes == other.nodes &&
          rootsLen == other.rootsLen &&
          roots == other.roots;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, bump, nextLeafIndex, nodes, rootsLen, roots);

  @override
  String toString() => 'MerkleTree(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, nextLeafIndex: $nextLeafIndex, nodes: $nodes, rootsLen: $rootsLen, roots: $roots)';
}


Encoder<MerkleTree> getMerkleTreeEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('nextLeafIndex', getU64Encoder()),
    ('nodes', fixEncoderSize(getBytesEncoder(), 8160, allowTruncation: false)),
    ('rootsLen', getU16Encoder()),
    ('roots', fixEncoderSize(getBytesEncoder(), 256, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (MerkleTree value) => <String, Object?>{
      'discriminator': 3,
      'migrationVersion': 0,
      'bump': value.bump,
      'nextLeafIndex': value.nextLeafIndex,
      'nodes': value.nodes,
      'rootsLen': value.rootsLen,
      'roots': value.roots,
    },
  );
}

Decoder<MerkleTree> getMerkleTreeDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('nextLeafIndex', getU64Decoder()),
    ('nodes', fixDecoderSize(getBytesDecoder(), 8160)),
    ('rootsLen', getU16Decoder()),
    ('roots', fixDecoderSize(getBytesDecoder(), 256)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'merkleTree account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (MerkleTree, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(3),
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
      MerkleTree(
      bump: map['bump']! as int,
      nextLeafIndex: map['nextLeafIndex']! as BigInt,
      nodes: map['nodes']! as Uint8List,
      rootsLen: map['rootsLen']! as int,
      roots: map['roots']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<MerkleTree>(
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
      VariableSizeDecoder<MerkleTree>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<MerkleTree, MerkleTree> getMerkleTreeCodec() {
  return combineCodec(getMerkleTreeEncoder(), getMerkleTreeDecoder());
}

Account<MerkleTree> decodeMerkleTree(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getMerkleTreeDecoder());
}

/// The account schema version this client was generated from.
const int merkleTreeMigrationVersion = 0;

/// Cheap envelope check for fetched `MerkleTree` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool merkleTreeNeedsMigration(List<int> data) {
	if (data.length < 2) {
		return false;
	}
	if (data[0] != 3) {
		return false;
	}
	return data[1] < 0;
}
