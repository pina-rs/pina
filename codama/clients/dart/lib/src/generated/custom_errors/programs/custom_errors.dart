// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the CustomErrors program.
const customErrorsProgramAddress = Address(
  'Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS',
);

/// Known instructions for the CustomErrors program.
enum CustomErrorsInstruction {
  hello,
  helloNoMsg,
  helloNext,
  requireEq,
  requireNeq,
  requireGt,
  requireGte,
}

/// Identifies the type of a CustomErrors instruction.
CustomErrorsInstruction identifyCustomErrorsInstruction(Uint8List data) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return CustomErrorsInstruction.hello;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return CustomErrorsInstruction.helloNoMsg;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return CustomErrorsInstruction.helloNext;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0)) {
    return CustomErrorsInstruction.requireEq;
  }
  if (containsBytes(data, getU8Encoder().encode(4), 0)) {
    return CustomErrorsInstruction.requireNeq;
  }
  if (containsBytes(data, getU8Encoder().encode(5), 0)) {
    return CustomErrorsInstruction.requireGt;
  }
  if (containsBytes(data, getU8Encoder().encode(6), 0)) {
    return CustomErrorsInstruction.requireGte;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'customErrors',
  });
}

/// A parsed instruction from the CustomErrors program.
sealed class ParsedCustomErrorsInstruction {
  const ParsedCustomErrorsInstruction(this.instructionType);

  final CustomErrorsInstruction instructionType;
}

/// A parsed Hello instruction.
final class ParsedHello extends ParsedCustomErrorsInstruction {
  const ParsedHello({required this.data})
    : super(CustomErrorsInstruction.hello);

  final HelloInstructionData data;
}

/// A parsed HelloNoMsg instruction.
final class ParsedHelloNoMsg extends ParsedCustomErrorsInstruction {
  const ParsedHelloNoMsg({required this.data})
    : super(CustomErrorsInstruction.helloNoMsg);

  final HelloNoMsgInstructionData data;
}

/// A parsed HelloNext instruction.
final class ParsedHelloNext extends ParsedCustomErrorsInstruction {
  const ParsedHelloNext({required this.data})
    : super(CustomErrorsInstruction.helloNext);

  final HelloNextInstructionData data;
}

/// A parsed RequireEq instruction.
final class ParsedRequireEq extends ParsedCustomErrorsInstruction {
  const ParsedRequireEq({required this.data})
    : super(CustomErrorsInstruction.requireEq);

  final RequireEqInstructionData data;
}

/// A parsed RequireNeq instruction.
final class ParsedRequireNeq extends ParsedCustomErrorsInstruction {
  const ParsedRequireNeq({required this.data})
    : super(CustomErrorsInstruction.requireNeq);

  final RequireNeqInstructionData data;
}

/// A parsed RequireGt instruction.
final class ParsedRequireGt extends ParsedCustomErrorsInstruction {
  const ParsedRequireGt({required this.data})
    : super(CustomErrorsInstruction.requireGt);

  final RequireGtInstructionData data;
}

/// A parsed RequireGte instruction.
final class ParsedRequireGte extends ParsedCustomErrorsInstruction {
  const ParsedRequireGte({required this.data})
    : super(CustomErrorsInstruction.requireGte);

  final RequireGteInstructionData data;
}

/// Parses a CustomErrors instruction.
ParsedCustomErrorsInstruction parseCustomErrorsInstruction(
  Instruction instruction,
) {
  return switch (identifyCustomErrorsInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    CustomErrorsInstruction.hello => ParsedHello(
      data: parseHelloInstruction(instruction),
    ),
    CustomErrorsInstruction.helloNoMsg => ParsedHelloNoMsg(
      data: parseHelloNoMsgInstruction(instruction),
    ),
    CustomErrorsInstruction.helloNext => ParsedHelloNext(
      data: parseHelloNextInstruction(instruction),
    ),
    CustomErrorsInstruction.requireEq => ParsedRequireEq(
      data: parseRequireEqInstruction(instruction),
    ),
    CustomErrorsInstruction.requireNeq => ParsedRequireNeq(
      data: parseRequireNeqInstruction(instruction),
    ),
    CustomErrorsInstruction.requireGt => ParsedRequireGt(
      data: parseRequireGtInstruction(instruction),
    ),
    CustomErrorsInstruction.requireGte => ParsedRequireGte(
      data: parseRequireGteInstruction(instruction),
    ),
  };
}
