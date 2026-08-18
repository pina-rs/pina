// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the PinaBpf program.
const pinaBpfProgramAddress = Address(
  '2nYtoevJCC8AFjdsfmkf8y1jN2nN9k4jVtD7G3f5n1Qe',
);

/// Known instructions for the PinaBpf program.
enum PinaBpfInstruction { hello }

/// Identifies the type of a PinaBpf instruction.
PinaBpfInstruction identifyPinaBpfInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return PinaBpfInstruction.hello;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'pinaBpf',
  });
}

/// A parsed instruction from the PinaBpf program.
sealed class ParsedPinaBpfInstruction {
  const ParsedPinaBpfInstruction(this.instructionType);

  final PinaBpfInstruction instructionType;
}

/// A parsed Hello instruction.
final class ParsedHello extends ParsedPinaBpfInstruction {
  const ParsedHello({required this.data}) : super(PinaBpfInstruction.hello);

  final HelloInstructionData data;
}

/// Parses a PinaBpf instruction.
ParsedPinaBpfInstruction parsePinaBpfInstruction(Instruction instruction) {
  return switch (identifyPinaBpfInstruction(instruction.data ?? Uint8List(0))) {
    PinaBpfInstruction.hello => ParsedHello(
      data: parseHelloInstruction(instruction),
    ),
  };
}
