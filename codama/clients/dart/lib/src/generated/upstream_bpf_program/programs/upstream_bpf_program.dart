// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the UpstreamBpfProgram program.
const upstreamBpfProgramProgramAddress = Address(
  '2oYypU3qZSK14274jeqP8XXGmXWn6Zhf4RRpLSDGURmk',
);

/// Known instructions for the UpstreamBpfProgram program.
enum UpstreamBpfProgramInstruction { hello, ping }

/// Identifies the type of a UpstreamBpfProgram instruction.
UpstreamBpfProgramInstruction identifyUpstreamBpfProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return UpstreamBpfProgramInstruction.hello;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return UpstreamBpfProgramInstruction.ping;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'upstreamBpfProgram',
  });
}

/// A parsed instruction from the UpstreamBpfProgram program.
sealed class ParsedUpstreamBpfProgramInstruction {
  const ParsedUpstreamBpfProgramInstruction(this.instructionType);

  final UpstreamBpfProgramInstruction instructionType;
}

/// A parsed Hello instruction.
final class ParsedHello extends ParsedUpstreamBpfProgramInstruction {
  const ParsedHello({required this.data})
    : super(UpstreamBpfProgramInstruction.hello);

  final HelloInstructionData data;
}

/// A parsed Ping instruction.
final class ParsedPing extends ParsedUpstreamBpfProgramInstruction {
  const ParsedPing({required this.data})
    : super(UpstreamBpfProgramInstruction.ping);

  final PingInstructionData data;
}

/// Parses a UpstreamBpfProgram instruction.
ParsedUpstreamBpfProgramInstruction parseUpstreamBpfProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyUpstreamBpfProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    UpstreamBpfProgramInstruction.hello => ParsedHello(
      data: parseHelloInstruction(instruction),
    ),
    UpstreamBpfProgramInstruction.ping => ParsedPing(
      data: parsePingInstruction(instruction),
    ),
  };
}
