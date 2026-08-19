// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the AnchorDuplicateMutableAccounts program.
const anchorDuplicateMutableAccountsProgramAddress = Address(
  '4D6rvpR7TSPwmFottLGa5gpzMcJ76kN8bimQHV9rogjH',
);

/// Known instructions for the AnchorDuplicateMutableAccounts program.
enum AnchorDuplicateMutableAccountsInstruction {
  failsDuplicateMutable,
  allowsDuplicateMutable,
  allowsDuplicateReadonly,
}

/// Identifies the type of a AnchorDuplicateMutableAccounts instruction.
AnchorDuplicateMutableAccountsInstruction
identifyAnchorDuplicateMutableAccountsInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return AnchorDuplicateMutableAccountsInstruction.failsDuplicateMutable;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return AnchorDuplicateMutableAccountsInstruction.allowsDuplicateMutable;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return AnchorDuplicateMutableAccountsInstruction.allowsDuplicateReadonly;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'anchorDuplicateMutableAccounts',
  });
}

/// A parsed instruction from the AnchorDuplicateMutableAccounts program.
sealed class ParsedAnchorDuplicateMutableAccountsInstruction {
  const ParsedAnchorDuplicateMutableAccountsInstruction(this.instructionType);

  final AnchorDuplicateMutableAccountsInstruction instructionType;
}

/// A parsed FailsDuplicateMutable instruction.
final class ParsedFailsDuplicateMutable
    extends ParsedAnchorDuplicateMutableAccountsInstruction {
  const ParsedFailsDuplicateMutable({required this.data})
    : super(AnchorDuplicateMutableAccountsInstruction.failsDuplicateMutable);

  final FailsDuplicateMutableInstructionData data;
}

/// A parsed AllowsDuplicateMutable instruction.
final class ParsedAllowsDuplicateMutable
    extends ParsedAnchorDuplicateMutableAccountsInstruction {
  const ParsedAllowsDuplicateMutable({required this.data})
    : super(AnchorDuplicateMutableAccountsInstruction.allowsDuplicateMutable);

  final AllowsDuplicateMutableInstructionData data;
}

/// A parsed AllowsDuplicateReadonly instruction.
final class ParsedAllowsDuplicateReadonly
    extends ParsedAnchorDuplicateMutableAccountsInstruction {
  const ParsedAllowsDuplicateReadonly({required this.data})
    : super(AnchorDuplicateMutableAccountsInstruction.allowsDuplicateReadonly);

  final AllowsDuplicateReadonlyInstructionData data;
}

/// Parses a AnchorDuplicateMutableAccounts instruction.
ParsedAnchorDuplicateMutableAccountsInstruction
parseAnchorDuplicateMutableAccountsInstruction(Instruction instruction) {
  return switch (identifyAnchorDuplicateMutableAccountsInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    AnchorDuplicateMutableAccountsInstruction.failsDuplicateMutable =>
      ParsedFailsDuplicateMutable(
        data: parseFailsDuplicateMutableInstruction(instruction),
      ),
    AnchorDuplicateMutableAccountsInstruction.allowsDuplicateMutable =>
      ParsedAllowsDuplicateMutable(
        data: parseAllowsDuplicateMutableInstruction(instruction),
      ),
    AnchorDuplicateMutableAccountsInstruction.allowsDuplicateReadonly =>
      ParsedAllowsDuplicateReadonly(
        data: parseAllowsDuplicateReadonlyInstruction(instruction),
      ),
  };
}
