// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../programs/prop_amm_program.dart' show propAmmProgramProgramAddress;

/// Discriminator reserved by Pina for the framework `Migrate` instruction.
const migrateDiscriminator = 255;

Uint8List getMigrateDiscriminatorBytes() =>
    Uint8List.fromList(const [255]);

/// Creates the framework-owned `Migrate` instruction: it runs the program's
/// on-demand account migrations on their own, so the payer authorizes exactly
/// the migration cost and the business instruction that follows sees current
/// data. The intended flow around any instruction that failed with a
/// migration version mismatch is catch -> migrate -> retry.
///
/// Every migratable account is optional: omitted slots become program-address
/// placeholders and trailing omitted slots are truncated, so a client sends
/// only the accounts it needs to migrate. The `payer` funds rent deficits and
/// must sign; omit it when no migration needs funding.
Instruction getMigrateInstruction({
  Address? programAddress,
  Address? payer,
  Address? systemProgram,
  Address? oracleState,
}) {
  final resolvedProgram = programAddress ?? propAmmProgramProgramAddress;
  final metas = <AccountMeta>[
    AccountMeta(
      address: payer ?? resolvedProgram,
      role: payer == null ? AccountRole.readonly : AccountRole.writableSigner,
    ),
    AccountMeta(address: systemProgram ?? resolvedProgram, role: AccountRole.readonly),
    AccountMeta(
      address: oracleState ?? resolvedProgram,
      role: oracleState == null ? AccountRole.readonly : AccountRole.writable,
    ),
  ];
  final provided = [payer, systemProgram, oracleState];
  var last = -1;
  for (var index = 0; index < provided.length; index++) {
    if (provided[index] != null) {
      last = index;
    }
  }
  return Instruction(
    programAddress: resolvedProgram,
    accounts: metas.sublist(0, last + 1),
    data: getMigrateDiscriminatorBytes(),
  );
}
