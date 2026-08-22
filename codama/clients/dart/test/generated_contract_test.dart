import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:pina_codama_clients/anchor_realloc.dart'
    show Sample, getSampleDecoder, getSampleEncoder;
import 'package:pina_codama_clients/profile_program.dart';
import 'package:solana_kit_accounts/solana_kit_accounts.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';
import 'package:solana_kit_rpc_types/solana_kit_rpc_types.dart';
import 'package:test/test.dart';

const systemAddress = Address('11111111111111111111111111111111');
final contractFixture =
    jsonDecode(File('../../contracts/profile_program.json').readAsStringSync())
        as Map<String, Object?>;

void main() {
  group('generated client inventory', () {
    test('contains a public entrypoint for every example IDL', () {
      final idls =
          Directory('../../idls')
              .listSync()
              .whereType<File>()
              .map((file) => file.uri.pathSegments.last)
              .where((name) => name.endsWith('.json'))
              .map((name) => name.substring(0, name.length - '.json'.length))
              .toList()
            ..sort();
      final generatedRoot = Directory('lib/src/generated');
      final generated =
          generatedRoot
              .listSync()
              .whereType<Directory>()
              .map(
                (directory) =>
                    directory.uri.pathSegments.reversed.skip(1).first,
              )
              .toList()
            ..sort();
      final entrypoints =
          Directory('lib')
              .listSync()
              .whereType<File>()
              .map((file) => file.uri.pathSegments.last)
              .where((name) => name.endsWith('.dart'))
              .map((name) => name.substring(0, name.length - '.dart'.length))
              .toList()
            ..sort();

      expect(idls, expectedPrograms);
      expect(generated, expectedPrograms);
      expect(entrypoints, expectedPrograms);
    });
  });

  group('ProfileState account codec', () {
    test('matches the exact zeropod wire layout and round-trips', () {
      final state = ProfileState(
        bump: 254,
        name: _boundedText('A\u0000B', 33),
        bio: _boundedText('bio', 129),
        tags: _tagBytes([BigInt.from(7), BigInt.from(9)]),
        favoriteTag: BigInt.from(42),
        active: true,
      );
      final encoded = getProfileStateEncoder().encode(state);
      final fixture = contractFixture['profileState']! as Map<String, Object?>;

      expect(encoded.length, fixture['size']);
      expect(
        encoded,
        orderedEquals(_decodeHex(fixture['encodedHex']! as String)),
      );
      expect(encoded.sublist(0, 2), orderedEquals([1, 254]));
      expect(encoded.sublist(2, 6), orderedEquals([3, 65, 0, 66]));
      expect(encoded.sublist(35, 39), orderedEquals([3, 98, 105, 111]));
      expect(encoded.sublist(164, 166), orderedEquals([2, 0]));
      expect(
        encoded.sublist(166, 174),
        orderedEquals([7, 0, 0, 0, 0, 0, 0, 0]),
      );
      expect(
        encoded.sublist(174, 182),
        orderedEquals([9, 0, 0, 0, 0, 0, 0, 0]),
      );
      expect(
        encoded.sublist(230, 239),
        orderedEquals([1, 42, 0, 0, 0, 0, 0, 0, 0]),
      );
      expect(encoded[239], 1);

      final decoded = getProfileStateDecoder().decode(encoded);
      expect(decoded.discriminator, 1);
      expect(decoded.bump, 254);
      expect(decoded.name, orderedEquals(_boundedText('A\u0000B', 33)));
      expect(decoded.bio, orderedEquals(_boundedText('bio', 129)));
      expect(
        decoded.tags,
        orderedEquals(_tagBytes([BigInt.from(7), BigInt.from(9)])),
      );
      expect(decoded.favoriteTag, BigInt.from(42));
      expect(decoded.active, isTrue);
    });

    test('rejects over-capacity fixed arrays instead of truncating', () {
      final overlongName = _profile(name: Uint8List(34));
      final tooManyTags = _profile(tags: Uint8List(67));

      expect(
        () => getProfileStateEncoder().encode(overlongName),
        throwsA(isA<SolanaError>()),
      );
      expect(
        () => getProfileStateEncoder().encode(tooManyTags),
        throwsA(isA<SolanaError>()),
      );
    });

    test('rejects invalid discriminators, booleans, and option tags', () {
      final canonical = getProfileStateEncoder().encode(_profile());
      final badDiscriminator = Uint8List.fromList(canonical)..[0] = 2;
      final badOption = Uint8List.fromList(canonical)..[230] = 2;
      final badBoolean = Uint8List.fromList(canonical)..[239] = 2;

      expect(
        () => getProfileStateDecoder().decode(badDiscriminator),
        throwsA(isA<SolanaError>()),
      );
      expect(
        () => getProfileStateDecoder().decode(badOption),
        throwsA(isA<SolanaError>()),
      );
      expect(
        () => getProfileStateDecoder().decode(badBoolean),
        throwsA(isA<SolanaError>()),
      );
    });

    test('treats bounded storage contents as opaque fixed bytes', () {
      final canonical = getProfileStateEncoder().encode(_profile());
      final fixture = contractFixture['profileState']! as Map<String, Object?>;
      final nameOffset = fixture['nameOffset']! as int;
      final tagsOffset = fixture['tagsOffset']! as int;
      final opaque = Uint8List.fromList(canonical)
        ..[nameOffset] = 33
        ..[nameOffset + 1] = 0xc3
        ..[nameOffset + 2] = 0x28
        ..[tagsOffset] = 9;

      final decoded = getProfileStateDecoder().decode(opaque);
      expect(decoded.name.sublist(0, 3), orderedEquals([33, 0xc3, 0x28]));
      expect(decoded.tags[0], 9);
    });

    test('treats inactive option capacity as unobservable', () {
      final none = getProfileStateEncoder().encode(_profile());

      for (var index = 231; index < 239; index++) {
        none[index] = 0xa5;
      }

      final decoded = getProfileStateDecoder().decode(none);
      expect(decoded.favoriteTag, isNull);
    });

    test('rejects truncation and permits trailing account capacity', () {
      final bytes = getProfileStateEncoder().encode(_profile());
      final account = _encodedProfile(bytes);
      final oversized = _encodedProfile(Uint8List.fromList([...bytes, 0]));
      final truncated = _encodedProfile(
        Uint8List.sublistView(bytes, 0, bytes.length - 1),
      );

      expect(decodeProfileState(account).data.discriminator, 1);
      expect(decodeProfileState(oversized).data.discriminator, 1);
      expect(() => decodeProfileState(truncated), throwsA(isA<SolanaError>()));
    });
  });

  group('resizable account codec', () {
    test('decodes a fixed header with trailing resized capacity', () {
      final canonical = getSampleEncoder().encode(
        const Sample(bump: 254, authority: systemAddress),
      );
      final resized = Uint8List.fromList([
        ...canonical,
        ...List<int>.filled(128, 0xa5),
      ]);

      final decoded = getSampleDecoder().decode(resized);

      expect(decoded.discriminator, 1);
      expect(decoded.bump, 254);
      expect(decoded.authority, systemAddress);
    });
  });

  group('Profile instruction codecs', () {
    test('builds and parses the exact initialize layout', () {
      final instruction = getInitializeInstruction(
        programAddress: profileProgramProgramAddress,
        authority: systemAddress,
        profile: systemAddress,
        systemProgram: systemAddress,
        bump: 9,
        name: _boundedText('name', 33),
        bio: _boundedText('bio', 129),
      );
      final data = instruction.data!;
      final fixture =
          contractFixture['initializeInstruction']! as Map<String, Object?>;

      expect(data.length, fixture['size']);
      expect(data, orderedEquals(_decodeHex(fixture['encodedHex']! as String)));
      expect(data.sublist(0, 2), orderedEquals([0, 9]));
      expect(data.sublist(2, 7), orderedEquals([4, 110, 97, 109, 101]));
      expect(data.sublist(35, 39), orderedEquals([3, 98, 105, 111]));

      final parsed = parseInitializeInstruction(instruction);
      expect(parsed.discriminator, 0);
      expect(parsed.bump, 9);
      expect(parsed.name, orderedEquals(_boundedText('name', 33)));
      expect(parsed.bio, orderedEquals(_boundedText('bio', 129)));
    });

    test('rejects malformed discriminators and trailing bytes', () {
      final canonical = getInitializeInstruction(
        programAddress: profileProgramProgramAddress,
        authority: systemAddress,
        profile: systemAddress,
        systemProgram: systemAddress,
        bump: 9,
        name: _boundedText('name', 33),
        bio: _boundedText('bio', 129),
      );
      final malformedData = Uint8List.fromList(canonical.data!)..[0] = 1;
      final malformed = Instruction(
        programAddress: canonical.programAddress,
        accounts: canonical.accounts,
        data: malformedData,
      );
      final trailing = Instruction(
        programAddress: canonical.programAddress,
        accounts: canonical.accounts,
        data: Uint8List.fromList([...canonical.data!, 0]),
      );

      expect(
        () => parseInitializeInstruction(malformed),
        throwsA(isA<SolanaError>()),
      );
      expect(
        () => parseInitializeInstruction(trailing),
        throwsA(isA<SolanaError>()),
      );
    });
  });

  test('generated enum codec pattern rejects undeclared discriminants', () {
    final decoder = _getStatusDecoder();

    expect(decoder.decode(Uint8List.fromList([0])), _Status.inactive);
    expect(decoder.decode(Uint8List.fromList([1])), _Status.active);
    expect(() => decoder.decode(Uint8List.fromList([2])), throwsRangeError);
  });
}

ProfileState _profile({Uint8List? name, Uint8List? tags}) {
  return ProfileState(
    bump: 7,
    name: name ?? _boundedText('name', 33),
    bio: _boundedText('bio', 129),
    tags: tags ?? _tagBytes(const []),
    favoriteTag: null,
    active: false,
  );
}

Uint8List _boundedText(String value, int size) {
  final payload = utf8.encode(value);
  if (payload.length >= size || payload.length > 0xff) {
    throw ArgumentError.value(value, 'value', 'does not fit bounded storage');
  }

  return Uint8List(size)
    ..[0] = payload.length
    ..setRange(1, payload.length + 1, payload);
}

Uint8List _tagBytes(List<BigInt> values) {
  const capacity = 8;
  if (values.length > capacity) {
    throw RangeError.range(values.length, 0, capacity, 'values.length');
  }

  final bytes = Uint8List(2 + capacity * 8);
  ByteData.sublistView(bytes).setUint16(0, values.length, Endian.little);
  for (var index = 0; index < values.length; index++) {
    final value = values[index];
    if (value.isNegative || value.bitLength > 64) {
      throw ArgumentError.value(value, 'values[$index]', 'must fit in u64');
    }
    for (var byte = 0; byte < 8; byte++) {
      bytes[2 + index * 8 + byte] = ((value >> (byte * 8)) & BigInt.from(0xff))
          .toInt();
    }
  }
  return bytes;
}

enum _Status { inactive, active }

Decoder<_Status> _getStatusDecoder() {
  return transformDecoder(
    getU8Decoder(),
    (int value, Uint8List _, int _) => _Status.values[value],
  );
}

EncodedAccount _encodedProfile(Uint8List data) {
  return Account<Uint8List>(
    address: systemAddress,
    data: data,
    executable: false,
    lamports: Lamports(BigInt.zero),
    programAddress: profileProgramProgramAddress,
    space: BigInt.from(data.length),
  );
}

Uint8List _decodeHex(String value) {
  if (value.length.isOdd) {
    throw FormatException('hex strings must contain complete bytes', value);
  }

  return Uint8List.fromList([
    for (var offset = 0; offset < value.length; offset += 2)
      int.parse(value.substring(offset, offset + 2), radix: 16),
  ]);
}

const expectedPrograms = <String>[
  'anchor_declare_id',
  'anchor_declare_program',
  'anchor_duplicate_mutable_accounts',
  'anchor_errors',
  'anchor_events',
  'anchor_floats',
  'anchor_realloc',
  'anchor_system_accounts',
  'anchor_sysvars',
  'counter_program',
  'escrow_program',
  'hello_solana',
  'optional_accounts_program',
  'pina_bpf',
  'profile_program',
  'prop_amm_program',
  'role_registry_program',
  'staking_rewards_program',
  'todo_program',
  'transfer_sol',
  'vesting_program',
];
