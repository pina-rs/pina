// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the TransferSol program.
const transferSolProgramAddress = Address(
  'BuXKn8EiVMKF8zYThuea3xhLq3jUHTTwDDLfCoehq7WG',
);

/// Known instructions for the TransferSol program.
enum TransferSolInstruction { cpiTransfer, directTransfer }

/// Identifies the type of a TransferSol instruction.
TransferSolInstruction identifyTransferSolInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return TransferSolInstruction.cpiTransfer;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return TransferSolInstruction.directTransfer;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'transferSol',
  });
}

/// A parsed instruction from the TransferSol program.
sealed class ParsedTransferSolInstruction {
  const ParsedTransferSolInstruction(this.instructionType);

  final TransferSolInstruction instructionType;
}

/// A parsed CpiTransfer instruction.
final class ParsedCpiTransfer extends ParsedTransferSolInstruction {
  const ParsedCpiTransfer({required this.data})
    : super(TransferSolInstruction.cpiTransfer);

  final CpiTransferInstructionData data;
}

/// A parsed DirectTransfer instruction.
final class ParsedDirectTransfer extends ParsedTransferSolInstruction {
  const ParsedDirectTransfer({required this.data})
    : super(TransferSolInstruction.directTransfer);

  final DirectTransferInstructionData data;
}

/// Parses a TransferSol instruction.
ParsedTransferSolInstruction parseTransferSolInstruction(
  Instruction instruction,
) {
  return switch (identifyTransferSolInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    TransferSolInstruction.cpiTransfer => ParsedCpiTransfer(
      data: parseCpiTransferInstruction(instruction),
    ),
    TransferSolInstruction.directTransfer => ParsedDirectTransfer(
      data: parseDirectTransferInstruction(instruction),
    ),
  };
}
