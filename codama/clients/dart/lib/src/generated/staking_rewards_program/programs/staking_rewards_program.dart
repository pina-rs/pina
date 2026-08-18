// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';

/// The address of the StakingRewardsProgram program.
const stakingRewardsProgramProgramAddress = Address(
  '9MBwKBjzTLtLe8PkHVhi5CfGxKo8gCYbMEg5NMt1tcvr',
);

/// Known accounts for the StakingRewardsProgram program.
enum StakingRewardsProgramAccount { poolState, positionState }

/// Known instructions for the StakingRewardsProgram program.
enum StakingRewardsProgramInstruction {
  initializePool,
  openPosition,
  deposit,
  withdraw,
  claim,
}

/// Identifies the type of a StakingRewardsProgram instruction.
StakingRewardsProgramInstruction identifyStakingRewardsProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0)) {
    return StakingRewardsProgramInstruction.initializePool;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0)) {
    return StakingRewardsProgramInstruction.openPosition;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0)) {
    return StakingRewardsProgramInstruction.deposit;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0)) {
    return StakingRewardsProgramInstruction.withdraw;
  }
  if (containsBytes(data, getU8Encoder().encode(4), 0)) {
    return StakingRewardsProgramInstruction.claim;
  }

  throw SolanaError(SolanaErrorCode.programClientsFailedToIdentifyInstruction, {
    'instructionData': data,
    'programName': 'stakingRewardsProgram',
  });
}

/// A parsed instruction from the StakingRewardsProgram program.
sealed class ParsedStakingRewardsProgramInstruction {
  const ParsedStakingRewardsProgramInstruction(this.instructionType);

  final StakingRewardsProgramInstruction instructionType;
}

/// A parsed InitializePool instruction.
final class ParsedInitializePool
    extends ParsedStakingRewardsProgramInstruction {
  const ParsedInitializePool({required this.data})
    : super(StakingRewardsProgramInstruction.initializePool);

  final InitializePoolInstructionData data;
}

/// A parsed OpenPosition instruction.
final class ParsedOpenPosition extends ParsedStakingRewardsProgramInstruction {
  const ParsedOpenPosition({required this.data})
    : super(StakingRewardsProgramInstruction.openPosition);

  final OpenPositionInstructionData data;
}

/// A parsed Deposit instruction.
final class ParsedDeposit extends ParsedStakingRewardsProgramInstruction {
  const ParsedDeposit({required this.data})
    : super(StakingRewardsProgramInstruction.deposit);

  final DepositInstructionData data;
}

/// A parsed Withdraw instruction.
final class ParsedWithdraw extends ParsedStakingRewardsProgramInstruction {
  const ParsedWithdraw({required this.data})
    : super(StakingRewardsProgramInstruction.withdraw);

  final WithdrawInstructionData data;
}

/// A parsed Claim instruction.
final class ParsedClaim extends ParsedStakingRewardsProgramInstruction {
  const ParsedClaim({required this.data})
    : super(StakingRewardsProgramInstruction.claim);

  final ClaimInstructionData data;
}

/// Parses a StakingRewardsProgram instruction.
ParsedStakingRewardsProgramInstruction parseStakingRewardsProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyStakingRewardsProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    StakingRewardsProgramInstruction.initializePool => ParsedInitializePool(
      data: parseInitializePoolInstruction(instruction),
    ),
    StakingRewardsProgramInstruction.openPosition => ParsedOpenPosition(
      data: parseOpenPositionInstruction(instruction),
    ),
    StakingRewardsProgramInstruction.deposit => ParsedDeposit(
      data: parseDepositInstruction(instruction),
    ),
    StakingRewardsProgramInstruction.withdraw => ParsedWithdraw(
      data: parseWithdrawInstruction(instruction),
    ),
    StakingRewardsProgramInstruction.claim => ParsedClaim(
      data: parseClaimInstruction(instruction),
    ),
  };
}
