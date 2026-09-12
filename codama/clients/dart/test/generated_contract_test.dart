import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:pina_codama_clients/account_realloc_program.dart'
    show Sample, getSampleDecoder, getSampleEncoder;
import 'package:pina_codama_clients/compact_accounts_program.dart'
    show Journal, getJournalDecoder, getJournalEncoder;
import 'package:pina_codama_clients/migrations_program.dart'
    as migrations_program;
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
    test('matches the exact PinaPod wire layout and round-trips', () {
      final state = ProfileState(
        bump: 254,
        name: 'A\u0000B',
        bio: 'bio',
        tags: [BigInt.from(7), BigInt.from(9)],
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
      expect(decoded.name, 'A\u0000B');
      expect(decoded.bio, 'bio');
      expect(decoded.tags, [BigInt.from(7), BigInt.from(9)]);
      expect(decoded.favoriteTag, BigInt.from(42));
      expect(decoded.active, isTrue);
    });

    test('rejects over-capacity semantic values instead of truncating', () {
      final overlongName = _profile(name: List.filled(33, 'x').join());
      final tooManyTags = _profile(tags: List.filled(9, BigInt.zero));

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

    test('rejects malformed semantic string and vector storage', () {
      final canonical = getProfileStateEncoder().encode(_profile());
      final fixture = contractFixture['profileState']! as Map<String, Object?>;
      final nameOffset = fixture['nameOffset']! as int;
      final tagsOffset = fixture['tagsOffset']! as int;
      final overlongName = Uint8List.fromList(canonical)..[nameOffset] = 33;
      final malformedUtf8 = Uint8List.fromList(canonical)
        ..[nameOffset] = 2
        ..[nameOffset + 1] = 0xc3
        ..[nameOffset + 2] = 0x28;
      final tooManyTags = Uint8List.fromList(canonical)..[tagsOffset] = 9;

      expect(
        () => getProfileStateDecoder().decode(overlongName),
        throwsA(anything),
      );
      expect(
        () => getProfileStateDecoder().decode(malformedUtf8),
        throwsA(anything),
      );
      expect(
        () => getProfileStateDecoder().decode(tooManyTags),
        throwsA(anything),
      );
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
    test('round trips active compact values without capacity padding', () {
      final values = [BigInt.zero, BigInt.one, BigInt.two];
      final encoded = getSampleEncoder().encode(
        Sample(bump: 254, authority: systemAddress, values: values),
      );
      final decoded = getSampleDecoder().decode(encoded);

      expect(encoded, hasLength(36 + values.length * 8));
      expect(decoded.discriminator, 1);
      expect(decoded.bump, 254);
      expect(decoded.authority, systemAddress);
      expect(decoded.values, values);
    });

    test('round trips compact headers and dynamic tails', () {
      final entries = [BigInt.from(5), BigInt.from(8), BigInt.from(13)];
      final markers = [21, 34];
      const title = 'piña';
      final encoded = getJournalEncoder().encode(
        Journal(
          bump: 7,
          authority: systemAddress,
          revision: 4,
          featuredEntry: BigInt.from(13),
          title: title,
          entries: entries,
          markers: markers,
          note: null,
        ),
      );
      final decoded = getJournalDecoder().decode(encoded);

      expect(
        encoded,
        hasLength(
          59 + utf8.encode(title).length + entries.length * 8 + markers.length,
        ),
      );
      expect(encoded.sublist(38, 47), [1, 13, 0, 0, 0, 0, 0, 0, 0]);
      expect(encoded[47], utf8.encode(title).length);
      expect(encoded.sublist(48, 50), [3, 0]);
      expect(encoded.sublist(50, 58), [2, 0, 0, 0, 0, 0, 0, 0]);
      expect(encoded[58], 0);
      expect(encoded.sublist(59, 64), utf8.encode(title));
      expect(decoded.discriminator, 1);
      expect(decoded.bump, 7);
      expect(decoded.authority, systemAddress);
      expect(decoded.revision, 4);
      expect(decoded.featuredEntry, BigInt.from(13));
      expect(decoded.title, title);
      expect(decoded.entries, entries);
      expect(decoded.markers, markers);
    });

    test('rejects compact capacities at encode and decode boundaries', () {
      Journal journal({
        List<BigInt> entries = const [],
        List<int> markers = const [],
        String title = '',
        String? note,
      }) => Journal(
        bump: 7,
        authority: systemAddress,
        revision: 4,
        featuredEntry: null,
        title: title,
        entries: entries,
        markers: markers,
        note: note,
      );

      expect(
        () => getJournalEncoder().encode(
          journal(entries: List.filled(9, BigInt.zero)),
        ),
        throwsA(anything),
      );
      expect(
        () => getJournalEncoder().encode(journal(markers: List.filled(9, 0))),
        throwsA(anything),
      );
      expect(
        () => getJournalEncoder().encode(
          journal(title: List.filled(25, 'x').join()),
        ),
        throwsA(anything),
      );
      expect(
        () => getJournalEncoder().encode(
          journal(note: List.filled(65, 'x').join()),
        ),
        throwsA(anything),
      );

      final empty = getJournalEncoder().encode(journal());
      final excessiveEntries = Uint8List.fromList(empty)..[48] = 9;
      final excessiveMarkers = Uint8List.fromList(empty)..[50] = 9;
      final invalidOption = Uint8List.fromList(empty)..[38] = 2;
      final invalidNoteOption = Uint8List.fromList(empty)..[58] = 2;
      final malformedUtf8 = Uint8List.fromList(
        getJournalEncoder().encode(journal(title: 'x')),
      )..[59] = 0xff;

      expect(
        () => getJournalDecoder().decode(excessiveEntries),
        throwsA(anything),
      );
      expect(
        () => getJournalDecoder().decode(excessiveMarkers),
        throwsA(anything),
      );
      expect(
        () => getJournalDecoder().decode(invalidOption),
        throwsA(anything),
      );
      expect(
        () => getJournalDecoder().decode(invalidNoteOption),
        throwsA(anything),
      );
      expect(
        () => getJournalDecoder().decode(malformedUtf8),
        throwsA(anything),
      );
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
        name: 'name',
        bio: 'bio',
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
      expect(parsed.name, 'name');
      expect(parsed.bio, 'bio');
    });

    test('rejects malformed discriminators and trailing bytes', () {
      final canonical = getInitializeInstruction(
        programAddress: profileProgramProgramAddress,
        authority: systemAddress,
        profile: systemAddress,
        systemProgram: systemAddress,
        bump: 9,
        name: 'name',
        bio: 'bio',
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
  group('migrations program migration helpers', () {
    test('exposes the reserved all-ones discriminator', () {
      expect(migrations_program.migrateDiscriminator, 255);
      expect(migrations_program.getMigrateDiscriminatorBytes(), [255]);
    });

    test('flags stale State envelopes only', () {
      expect(migrations_program.stateMigrationVersion, 2);
      final stale = List<int>.filled(10, 0)
        ..[0] = 1
        ..[1] = 0;
      expect(migrations_program.stateNeedsMigration(stale), isTrue);
      final current = List<int>.filled(10, 0)
        ..[0] = 1
        ..[1] = 2;
      expect(migrations_program.stateNeedsMigration(current), isFalse);
      final future = List<int>.filled(10, 0)
        ..[0] = 1
        ..[1] = 3;
      expect(migrations_program.stateNeedsMigration(future), isFalse);
      final foreign = List<int>.filled(10, 0)
        ..[0] = 9
        ..[1] = 0;
      expect(migrations_program.stateNeedsMigration(foreign), isFalse);
      expect(migrations_program.stateNeedsMigration(<int>[1]), isFalse);
    });

    test('composes placeholder-filled truncated migrate instructions', () {
      final program = migrations_program.migrationsProgramProgramAddress;
      final instruction = migrations_program.getMigrateInstruction(
        programAddress: program,
        payer: systemAddress,
        systemProgram: systemAddress,
        state: systemAddress,
      );
      expect(instruction.accounts!.length, 3);
      expect(instruction.accounts![0].address, systemAddress);
      expect(instruction.accounts![0].role, AccountRole.writableSigner);
      expect(instruction.accounts![1].role, AccountRole.readonly);
      expect(instruction.accounts![2].role, AccountRole.writable);
      expect(
        instruction.data!,
        migrations_program.getMigrateDiscriminatorBytes(),
      );
      expect(instruction.programAddress, program);

      final omitted = migrations_program.getMigrateInstruction(
        programAddress: program,
        compactState: systemAddress,
      );
      expect(omitted.accounts!.length, 5);
      expect(
        omitted.accounts!.take(4).map((meta) => meta.role),
        everyElement(AccountRole.readonly),
      );
      expect(omitted.accounts![4].role, AccountRole.writable);
    });
  });
}

ProfileState _profile({String? name, List<BigInt>? tags}) {
  return ProfileState(
    bump: 7,
    name: name ?? 'name',
    bio: 'bio',
    tags: tags ?? const [],
    favoriteTag: null,
    active: false,
  );
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
  'account_realloc_program',
  'compact_accounts_program',
  'counter_program',
  'custom_errors_program',
  'declare_id_program',
  'declare_program',
  'duplicate_mutable_accounts_program',
  'escrow_program',
  'events_program',
  'float_accounts_program',
  'hello_solana_program',
  'migrations_program',
  'optional_accounts_program',
  'pina_bpf_program',
  'profile_program',
  'prop_amm_program',
  'role_registry_program',
  'staking_rewards_program',
  'system_accounts_program',
  'sysvar_checks_program',
  'todo_program',
  'transfer_sol_program',
  'validation_program',
  'vesting_program',
];
