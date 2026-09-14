// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';


/// The address of the CustomErrorsProgram program.
const customErrorsProgramProgramAddress = Address('Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS');

/// Known instructions for the CustomErrorsProgram program.
enum CustomErrorsProgramInstruction {
  hello,
  helloNoMsg,
  helloNext,
  requireEq,
  requireNeq,
  requireGt,
  requireGte,
}

/// Identifies the type of a CustomErrorsProgram instruction.
CustomErrorsProgramInstruction identifyCustomErrorsProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return CustomErrorsProgramInstruction.hello;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return CustomErrorsProgramInstruction.helloNoMsg;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return CustomErrorsProgramInstruction.helloNext;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0)) {
    return CustomErrorsProgramInstruction.requireEq;
  }
  if (containsBytes(data, getU8Encoder().encode(4), 0)) {
    return CustomErrorsProgramInstruction.requireNeq;
  }
  if (containsBytes(data, getU8Encoder().encode(5), 0)) {
    return CustomErrorsProgramInstruction.requireGt;
  }
  if (containsBytes(data, getU8Encoder().encode(6), 0)) {
    return CustomErrorsProgramInstruction.requireGte;
  }

  throw SolanaError(
    SolanaErrorCode.programClientsFailedToIdentifyInstruction,
    {
      'instructionData': data,
      'programName': 'customErrorsProgram',
    },
  );
}

/// A parsed instruction from the CustomErrorsProgram program.
sealed class ParsedCustomErrorsProgramInstruction {
  const ParsedCustomErrorsProgramInstruction(this.instructionType);

  final CustomErrorsProgramInstruction instructionType;
}

/// A parsed Hello instruction.
final class ParsedHello extends ParsedCustomErrorsProgramInstruction {
  const ParsedHello({required this.data})
      : super(CustomErrorsProgramInstruction.hello);

  final HelloInstructionData data;
}

/// A parsed HelloNoMsg instruction.
final class ParsedHelloNoMsg extends ParsedCustomErrorsProgramInstruction {
  const ParsedHelloNoMsg({required this.data})
      : super(CustomErrorsProgramInstruction.helloNoMsg);

  final HelloNoMsgInstructionData data;
}

/// A parsed HelloNext instruction.
final class ParsedHelloNext extends ParsedCustomErrorsProgramInstruction {
  const ParsedHelloNext({required this.data})
      : super(CustomErrorsProgramInstruction.helloNext);

  final HelloNextInstructionData data;
}

/// A parsed RequireEq instruction.
final class ParsedRequireEq extends ParsedCustomErrorsProgramInstruction {
  const ParsedRequireEq({required this.data})
      : super(CustomErrorsProgramInstruction.requireEq);

  final RequireEqInstructionData data;
}

/// A parsed RequireNeq instruction.
final class ParsedRequireNeq extends ParsedCustomErrorsProgramInstruction {
  const ParsedRequireNeq({required this.data})
      : super(CustomErrorsProgramInstruction.requireNeq);

  final RequireNeqInstructionData data;
}

/// A parsed RequireGt instruction.
final class ParsedRequireGt extends ParsedCustomErrorsProgramInstruction {
  const ParsedRequireGt({required this.data})
      : super(CustomErrorsProgramInstruction.requireGt);

  final RequireGtInstructionData data;
}

/// A parsed RequireGte instruction.
final class ParsedRequireGte extends ParsedCustomErrorsProgramInstruction {
  const ParsedRequireGte({required this.data})
      : super(CustomErrorsProgramInstruction.requireGte);

  final RequireGteInstructionData data;
}

/// Parses a CustomErrorsProgram instruction.
ParsedCustomErrorsProgramInstruction parseCustomErrorsProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyCustomErrorsProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    CustomErrorsProgramInstruction.hello => ParsedHello(
      data: parseHelloInstruction(instruction),
    ),
    CustomErrorsProgramInstruction.helloNoMsg => ParsedHelloNoMsg(
      data: parseHelloNoMsgInstruction(instruction),
    ),
    CustomErrorsProgramInstruction.helloNext => ParsedHelloNext(
      data: parseHelloNextInstruction(instruction),
    ),
    CustomErrorsProgramInstruction.requireEq => ParsedRequireEq(
      data: parseRequireEqInstruction(instruction),
    ),
    CustomErrorsProgramInstruction.requireNeq => ParsedRequireNeq(
      data: parseRequireNeqInstruction(instruction),
    ),
    CustomErrorsProgramInstruction.requireGt => ParsedRequireGt(
      data: parseRequireGtInstruction(instruction),
    ),
    CustomErrorsProgramInstruction.requireGte => ParsedRequireGte(
      data: parseRequireGteInstruction(instruction),
    ),
  };
}
