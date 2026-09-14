// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the HelloSolanaProgram program.
const helloSolanaProgramProgramAddress = Address(
  'DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A',
);

/// Known instructions for the HelloSolanaProgram program.
enum HelloSolanaProgramInstruction { hello }

/// Identifies the type of a HelloSolanaProgram instruction.
HelloSolanaProgramInstruction identifyHelloSolanaProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return HelloSolanaProgramInstruction.hello;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'helloSolanaProgram',
  });
}

/// A parsed instruction from the HelloSolanaProgram program.
sealed class ParsedHelloSolanaProgramInstruction {
  const ParsedHelloSolanaProgramInstruction(this.instructionType);

  final HelloSolanaProgramInstruction instructionType;
}

/// A parsed Hello instruction.
final class ParsedHello extends ParsedHelloSolanaProgramInstruction {
  const ParsedHello({required this.data})
    : super(HelloSolanaProgramInstruction.hello);

  final HelloInstructionData data;
}

/// Parses a HelloSolanaProgram instruction.
ParsedHelloSolanaProgramInstruction parseHelloSolanaProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyHelloSolanaProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    HelloSolanaProgramInstruction.hello => ParsedHello(
      data: parseHelloInstruction(instruction),
    ),
  };
}
