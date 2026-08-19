// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the VestingProgram program.
const vestingProgramProgramAddress = Address(
  'FEa5fqN6NACrhWUZSBdGKybJKNxkdw8cdLvRvTARsFHh',
);

/// Known accounts for the VestingProgram program.
enum VestingProgramAccount { vestingState }

/// Known instructions for the VestingProgram program.
enum VestingProgramInstruction { initialize, claim, cancel }

/// Identifies the type of a VestingProgram instruction.
VestingProgramInstruction identifyVestingProgramInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return VestingProgramInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return VestingProgramInstruction.claim;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return VestingProgramInstruction.cancel;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'vestingProgram',
  });
}

/// A parsed instruction from the VestingProgram program.
sealed class ParsedVestingProgramInstruction {
  const ParsedVestingProgramInstruction(this.instructionType);

  final VestingProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedVestingProgramInstruction {
  const ParsedInitialize({required this.data})
    : super(VestingProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed Claim instruction.
final class ParsedClaim extends ParsedVestingProgramInstruction {
  const ParsedClaim({required this.data})
    : super(VestingProgramInstruction.claim);

  final ClaimInstructionData data;
}

/// A parsed Cancel instruction.
final class ParsedCancel extends ParsedVestingProgramInstruction {
  const ParsedCancel({required this.data})
    : super(VestingProgramInstruction.cancel);

  final CancelInstructionData data;
}

/// Parses a VestingProgram instruction.
ParsedVestingProgramInstruction parseVestingProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyVestingProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    VestingProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    VestingProgramInstruction.claim => ParsedClaim(
      data: parseClaimInstruction(instruction),
    ),
    VestingProgramInstruction.cancel => ParsedCancel(
      data: parseCancelInstruction(instruction),
    ),
  };
}
