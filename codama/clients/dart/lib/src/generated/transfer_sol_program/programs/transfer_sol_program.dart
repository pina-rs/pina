// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';


/// The address of the TransferSolProgram program.
const transferSolProgramProgramAddress = Address('BuXKn8EiVMKF8zYThuea3xhLq3jUHTTwDDLfCoehq7WG');

/// Known instructions for the TransferSolProgram program.
enum TransferSolProgramInstruction {
  cpiTransfer,
  directTransfer,
}

/// Identifies the type of a TransferSolProgram instruction.
TransferSolProgramInstruction identifyTransferSolProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return TransferSolProgramInstruction.cpiTransfer;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return TransferSolProgramInstruction.directTransfer;
  }

  throw SolanaError(
    SolanaErrorCode.programClientsFailedToIdentifyInstruction,
    {
      'instructionData': data,
      'programName': 'transferSolProgram',
    },
  );
}

/// A parsed instruction from the TransferSolProgram program.
sealed class ParsedTransferSolProgramInstruction {
  const ParsedTransferSolProgramInstruction(this.instructionType);

  final TransferSolProgramInstruction instructionType;
}

/// A parsed CpiTransfer instruction.
final class ParsedCpiTransfer extends ParsedTransferSolProgramInstruction {
  const ParsedCpiTransfer({required this.data})
      : super(TransferSolProgramInstruction.cpiTransfer);

  final CpiTransferInstructionData data;
}

/// A parsed DirectTransfer instruction.
final class ParsedDirectTransfer extends ParsedTransferSolProgramInstruction {
  const ParsedDirectTransfer({required this.data})
      : super(TransferSolProgramInstruction.directTransfer);

  final DirectTransferInstructionData data;
}

/// Parses a TransferSolProgram instruction.
ParsedTransferSolProgramInstruction parseTransferSolProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyTransferSolProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    TransferSolProgramInstruction.cpiTransfer => ParsedCpiTransfer(
      data: parseCpiTransferInstruction(instruction),
    ),
    TransferSolProgramInstruction.directTransfer => ParsedDirectTransfer(
      data: parseDirectTransferInstruction(instruction),
    ),
  };
}
