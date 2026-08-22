import 'dart:typed_data';

import 'package:pina_codama_clients/optional_accounts_program.dart'
    show
        getInitInstruction,
        getInspectInstruction,
        getNoteInstruction,
        getStoreStateCodec,
        getTouchInstruction,
        parseInitInstruction,
        parseInspectInstruction,
        parseNoteInstruction,
        parseTouchInstruction,
        StoreState;

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';
import 'package:test/test.dart';

void main() {
  const programAddress = Address('ccdMMVpwebk8NxwJdY4CndxkLKUTM6fkaFUteAfFeci');
  const authority = Address('7VfCXTUz4mCJ7oP8b1cBDh9V2N4MWiNtfGr2K4tD9jUH');
  const store = Address('GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS');
  const witness = Address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
  const note = Address('4ibrEMW5F6hKnkW4jVedswYv6H6VtwPN6ar6dvXDN1nT');

  group('optional mutable account (touch)', () {
    test('omitted slot fills with a readonly program-address meta', () {
      final instruction = getTouchInstruction(
        programAddress: programAddress,
        authority: authority,
      );

      final accounts = instruction.accounts!;
      expect(accounts, hasLength(2));
      expect(accounts[0].address, authority);
      expect(accounts[0].role, AccountRole.readonlySigner);
      expect(accounts[1].address, programAddress);
      expect(accounts[1].role, AccountRole.readonly);
    });

    test('provided slot emits a writable meta for the given address', () {
      final instruction = getTouchInstruction(
        programAddress: programAddress,
        authority: authority,
        store: store,
      );

      final accounts = instruction.accounts!;
      expect(accounts, hasLength(2));
      expect(accounts[1].address, store);
      expect(accounts[1].role, AccountRole.writable);
    });
  });

  group('optional immutable and optional signer accounts (inspect)', () {
    test('both omitted keeps the fixed three-account layout', () {
      final instruction = getInspectInstruction(
        programAddress: programAddress,
        authority: authority,
      );

      final accounts = instruction.accounts!;
      expect(accounts, hasLength(3));
      for (final meta in accounts.skip(1)) {
        expect(meta.address, programAddress);
        expect(meta.role, AccountRole.readonly);
      }
    });

    test('provided store and witness keep their declared roles', () {
      final instruction = getInspectInstruction(
        programAddress: programAddress,
        authority: authority,
        store: store,
        witness: witness,
      );

      final accounts = instruction.accounts!;
      expect(accounts, hasLength(3));
      expect(accounts[1].address, store);
      expect(accounts[1].role, AccountRole.readonly);

      final witnessMeta = accounts[2];
      expect(witnessMeta.address, witness);
      expect(witnessMeta.role, AccountRole.readonlySigner);
    });
  });

  group('optional non-signer account (note)', () {
    test('accepts an arbitrary readonly address when provided', () {
      final instruction = getNoteInstruction(
        programAddress: programAddress,
        authority: authority,
        note: note,
      );

      final accounts = instruction.accounts!;
      expect(accounts, hasLength(2));
      expect(accounts[1].address, note);
      expect(accounts[1].role, AccountRole.readonly);
    });

    test('falls back to the program address when omitted', () {
      final instruction = getNoteInstruction(
        programAddress: programAddress,
        authority: authority,
      );

      final accounts = instruction.accounts!;
      expect(accounts, hasLength(2));
      expect(accounts[1].address, programAddress);
      expect(accounts[1].role, AccountRole.readonly);
    });
  });

  group('init baseline', () {
    test('keeps all three required slots with their declared roles', () {
      final instruction = getInitInstruction(
        programAddress: programAddress,
        authority: authority,
        store: store,
        systemProgram: const Address('11111111111111111111111111111111'),
        bump: 254,
      );

      final accounts = instruction.accounts!;
      expect(accounts, hasLength(3));
      expect(accounts[0].role, AccountRole.writableSigner);
      expect(accounts[1].role, AccountRole.writable);
      expect(accounts[2].role, AccountRole.readonly);
    });
  });

  group('parsers and codecs', () {
    test('round-trip every discriminator through the generated parsers', () {
      final touch = getTouchInstruction(
        programAddress: programAddress,
        authority: authority,
      );
      expect(parseTouchInstruction(touch).discriminator, 1);

      final inspect = getInspectInstruction(
        programAddress: programAddress,
        authority: authority,
      );
      expect(parseInspectInstruction(inspect).discriminator, 2);

      final noteIx = getNoteInstruction(
        programAddress: programAddress,
        authority: authority,
      );
      expect(parseNoteInstruction(noteIx).discriminator, 3);
    });

    test('init parser recovers the bump argument', () {
      final init = getInitInstruction(
        programAddress: programAddress,
        authority: authority,
        store: store,
        systemProgram: const Address('11111111111111111111111111111111'),
        bump: 217,
      );
      expect(parseInitInstruction(init).bump, 217);
    });

    test('store state codec round-trips the on-chain layout', () {
      final bytes = Uint8List.fromList([1, 42, 7, 0, 0, 0, 0, 0, 0, 0]);
      final state = getStoreStateCodec().decode(bytes);
      expect(state.discriminator, 1);
      expect(state.bump, 42);
      expect(state.count, BigInt.from(7));

      final encoded = getStoreStateCodec().encode(
        StoreState(bump: 42, count: BigInt.from(7)),
      );
      expect(encoded, orderedEquals(bytes));
    });
  });
}
