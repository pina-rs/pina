// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';


/// The address of the PrivacyPoolProgram program.
const privacyPoolProgramProgramAddress = Address('DGHJjbUsSzAiSypH4dupxkQK1WLVcevvYchmM7mNLn9D');

/// Known accounts for the PrivacyPoolProgram program.
enum PrivacyPoolProgramAccount {
  poolConfig,
  poolVault,
  merkleTree,
  nullifierSet,
  custodianRegistry,
  requesterRegistry,
  noteCommitment,
  disclosureRequest,
  disclosureLog,
  verifyingKeyAccount,
}

/// Known instructions for the PrivacyPoolProgram program.
enum PrivacyPoolProgramInstruction {
  initialize,
  setVerificationKey,
  setCustodians,
  registerRequester,
  deposit,
  withdraw,
  transfer,
  requestDisclosure,
  grantDisclosure,
  challengeDisclosure,
  resolveChallenge,
  approveDisclosure,
  cancelDisclosure,
}

/// Identifies the type of a PrivacyPoolProgram instruction.
PrivacyPoolProgramInstruction identifyPrivacyPoolProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.initialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.setVerificationKey;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.setCustodians;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.registerRequester;
  }
  if (containsBytes(data, getU8Encoder().encode(4), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.deposit;
  }
  if (containsBytes(data, getU8Encoder().encode(5), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.withdraw;
  }
  if (containsBytes(data, getU8Encoder().encode(6), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.transfer;
  }
  if (containsBytes(data, getU8Encoder().encode(7), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.requestDisclosure;
  }
  if (containsBytes(data, getU8Encoder().encode(8), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.grantDisclosure;
  }
  if (containsBytes(data, getU8Encoder().encode(9), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.challengeDisclosure;
  }
  if (containsBytes(data, getU8Encoder().encode(10), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.resolveChallenge;
  }
  if (containsBytes(data, getU8Encoder().encode(11), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.approveDisclosure;
  }
  if (containsBytes(data, getU8Encoder().encode(12), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return PrivacyPoolProgramInstruction.cancelDisclosure;
  }

  throw SolanaError(
    SolanaErrorCode.programClientsFailedToIdentifyInstruction,
    {
      'instructionData': data,
      'programName': 'privacyPoolProgram',
    },
  );
}

/// A parsed instruction from the PrivacyPoolProgram program.
sealed class ParsedPrivacyPoolProgramInstruction {
  const ParsedPrivacyPoolProgramInstruction(this.instructionType);

  final PrivacyPoolProgramInstruction instructionType;
}

/// A parsed Initialize instruction.
final class ParsedInitialize extends ParsedPrivacyPoolProgramInstruction {
  const ParsedInitialize({required this.data})
      : super(PrivacyPoolProgramInstruction.initialize);

  final InitializeInstructionData data;
}

/// A parsed SetVerificationKey instruction.
final class ParsedSetVerificationKey extends ParsedPrivacyPoolProgramInstruction {
  const ParsedSetVerificationKey({required this.data})
      : super(PrivacyPoolProgramInstruction.setVerificationKey);

  final SetVerificationKeyInstructionData data;
}

/// A parsed SetCustodians instruction.
final class ParsedSetCustodians extends ParsedPrivacyPoolProgramInstruction {
  const ParsedSetCustodians({required this.data})
      : super(PrivacyPoolProgramInstruction.setCustodians);

  final SetCustodiansInstructionData data;
}

/// A parsed RegisterRequester instruction.
final class ParsedRegisterRequester extends ParsedPrivacyPoolProgramInstruction {
  const ParsedRegisterRequester({required this.data})
      : super(PrivacyPoolProgramInstruction.registerRequester);

  final RegisterRequesterInstructionData data;
}

/// A parsed Deposit instruction.
final class ParsedDeposit extends ParsedPrivacyPoolProgramInstruction {
  const ParsedDeposit({required this.data})
      : super(PrivacyPoolProgramInstruction.deposit);

  final DepositInstructionData data;
}

/// A parsed Withdraw instruction.
final class ParsedWithdraw extends ParsedPrivacyPoolProgramInstruction {
  const ParsedWithdraw({required this.data})
      : super(PrivacyPoolProgramInstruction.withdraw);

  final WithdrawInstructionData data;
}

/// A parsed Transfer instruction.
final class ParsedTransfer extends ParsedPrivacyPoolProgramInstruction {
  const ParsedTransfer({required this.data})
      : super(PrivacyPoolProgramInstruction.transfer);

  final TransferInstructionData data;
}

/// A parsed RequestDisclosure instruction.
final class ParsedRequestDisclosure extends ParsedPrivacyPoolProgramInstruction {
  const ParsedRequestDisclosure({required this.data})
      : super(PrivacyPoolProgramInstruction.requestDisclosure);

  final RequestDisclosureInstructionData data;
}

/// A parsed GrantDisclosure instruction.
final class ParsedGrantDisclosure extends ParsedPrivacyPoolProgramInstruction {
  const ParsedGrantDisclosure({required this.data})
      : super(PrivacyPoolProgramInstruction.grantDisclosure);

  final GrantDisclosureInstructionData data;
}

/// A parsed ChallengeDisclosure instruction.
final class ParsedChallengeDisclosure extends ParsedPrivacyPoolProgramInstruction {
  const ParsedChallengeDisclosure({required this.data})
      : super(PrivacyPoolProgramInstruction.challengeDisclosure);

  final ChallengeDisclosureInstructionData data;
}

/// A parsed ResolveChallenge instruction.
final class ParsedResolveChallenge extends ParsedPrivacyPoolProgramInstruction {
  const ParsedResolveChallenge({required this.data})
      : super(PrivacyPoolProgramInstruction.resolveChallenge);

  final ResolveChallengeInstructionData data;
}

/// A parsed ApproveDisclosure instruction.
final class ParsedApproveDisclosure extends ParsedPrivacyPoolProgramInstruction {
  const ParsedApproveDisclosure({required this.data})
      : super(PrivacyPoolProgramInstruction.approveDisclosure);

  final ApproveDisclosureInstructionData data;
}

/// A parsed CancelDisclosure instruction.
final class ParsedCancelDisclosure extends ParsedPrivacyPoolProgramInstruction {
  const ParsedCancelDisclosure({required this.data})
      : super(PrivacyPoolProgramInstruction.cancelDisclosure);

  final CancelDisclosureInstructionData data;
}

/// Parses a PrivacyPoolProgram instruction.
ParsedPrivacyPoolProgramInstruction parsePrivacyPoolProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyPrivacyPoolProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    PrivacyPoolProgramInstruction.initialize => ParsedInitialize(
      data: parseInitializeInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.setVerificationKey => ParsedSetVerificationKey(
      data: parseSetVerificationKeyInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.setCustodians => ParsedSetCustodians(
      data: parseSetCustodiansInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.registerRequester => ParsedRegisterRequester(
      data: parseRegisterRequesterInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.deposit => ParsedDeposit(
      data: parseDepositInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.withdraw => ParsedWithdraw(
      data: parseWithdrawInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.transfer => ParsedTransfer(
      data: parseTransferInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.requestDisclosure => ParsedRequestDisclosure(
      data: parseRequestDisclosureInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.grantDisclosure => ParsedGrantDisclosure(
      data: parseGrantDisclosureInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.challengeDisclosure => ParsedChallengeDisclosure(
      data: parseChallengeDisclosureInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.resolveChallenge => ParsedResolveChallenge(
      data: parseResolveChallengeInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.approveDisclosure => ParsedApproveDisclosure(
      data: parseApproveDisclosureInstruction(instruction),
    ),
    PrivacyPoolProgramInstruction.cancelDisclosure => ParsedCancelDisclosure(
      data: parseCancelDisclosureInstruction(instruction),
    ),
  };
}
