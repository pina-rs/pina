// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

import '../instructions/instructions.dart';


/// The address of the MultisigProgram program.
const multisigProgramProgramAddress = Address('5BeQ7VMZHYdnUD6PyrMd29WQo2DLfo7N2NDXCDQZ5MQc');

/// Known accounts for the MultisigProgram program.
enum MultisigProgramAccount {
  programConfig,
  multisig,
  proposal,
  spendingLimit,
}

/// Known instructions for the MultisigProgram program.
enum MultisigProgramInstruction {
  configInitialize,
  configUpdate,
  multisigCreate,
  multisigImport,
  proposalCreate,
  proposalActivate,
  proposalApprove,
  proposalReject,
  proposalRevoke,
  proposalCancel,
  vaultExecute,
  configExecute,
  configAuthorityExecute,
  spendingLimitUse,
  proposalClose,
}

/// Identifies the type of a MultisigProgram instruction.
MultisigProgramInstruction identifyMultisigProgramInstruction(
  Uint8List data,
) {
  if (containsBytes(data, getU8Encoder().encode(0), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.configInitialize;
  }
  if (containsBytes(data, getU8Encoder().encode(1), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.configUpdate;
  }
  if (containsBytes(data, getU8Encoder().encode(2), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.multisigCreate;
  }
  if (containsBytes(data, getU8Encoder().encode(3), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.multisigImport;
  }
  if (containsBytes(data, getU8Encoder().encode(4), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.proposalCreate;
  }
  if (containsBytes(data, getU8Encoder().encode(5), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.proposalActivate;
  }
  if (containsBytes(data, getU8Encoder().encode(6), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.proposalApprove;
  }
  if (containsBytes(data, getU8Encoder().encode(7), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.proposalReject;
  }
  if (containsBytes(data, getU8Encoder().encode(8), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.proposalRevoke;
  }
  if (containsBytes(data, getU8Encoder().encode(9), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.proposalCancel;
  }
  if (containsBytes(data, getU8Encoder().encode(10), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.vaultExecute;
  }
  if (containsBytes(data, getU8Encoder().encode(11), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.configExecute;
  }
  if (containsBytes(data, getU8Encoder().encode(12), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.configAuthorityExecute;
  }
  if (containsBytes(data, getU8Encoder().encode(13), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.spendingLimitUse;
  }
  if (containsBytes(data, getU8Encoder().encode(14), 0) && containsBytes(data, getU8Encoder().encode(0), 1)) {
    return MultisigProgramInstruction.proposalClose;
  }

  throw SolanaError(
    SolanaErrorCode.programClientsFailedToIdentifyInstruction,
    {
      'instructionData': data,
      'programName': 'multisigProgram',
    },
  );
}

/// A parsed instruction from the MultisigProgram program.
sealed class ParsedMultisigProgramInstruction {
  const ParsedMultisigProgramInstruction(this.instructionType);

  final MultisigProgramInstruction instructionType;
}

/// A parsed ConfigInitialize instruction.
final class ParsedConfigInitialize extends ParsedMultisigProgramInstruction {
  const ParsedConfigInitialize({required this.data})
      : super(MultisigProgramInstruction.configInitialize);

  final ConfigInitializeInstructionData data;
}

/// A parsed ConfigUpdate instruction.
final class ParsedConfigUpdate extends ParsedMultisigProgramInstruction {
  const ParsedConfigUpdate({required this.data})
      : super(MultisigProgramInstruction.configUpdate);

  final ConfigUpdateInstructionData data;
}

/// A parsed MultisigCreate instruction.
final class ParsedMultisigCreate extends ParsedMultisigProgramInstruction {
  const ParsedMultisigCreate({required this.data})
      : super(MultisigProgramInstruction.multisigCreate);

  final MultisigCreateInstructionData data;
}

/// A parsed MultisigImport instruction.
final class ParsedMultisigImport extends ParsedMultisigProgramInstruction {
  const ParsedMultisigImport({required this.data})
      : super(MultisigProgramInstruction.multisigImport);

  final MultisigImportInstructionData data;
}

/// A parsed ProposalCreate instruction.
final class ParsedProposalCreate extends ParsedMultisigProgramInstruction {
  const ParsedProposalCreate({required this.data})
      : super(MultisigProgramInstruction.proposalCreate);

  final ProposalCreateInstructionData data;
}

/// A parsed ProposalActivate instruction.
final class ParsedProposalActivate extends ParsedMultisigProgramInstruction {
  const ParsedProposalActivate({required this.data})
      : super(MultisigProgramInstruction.proposalActivate);

  final ProposalActivateInstructionData data;
}

/// A parsed ProposalApprove instruction.
final class ParsedProposalApprove extends ParsedMultisigProgramInstruction {
  const ParsedProposalApprove({required this.data})
      : super(MultisigProgramInstruction.proposalApprove);

  final ProposalApproveInstructionData data;
}

/// A parsed ProposalReject instruction.
final class ParsedProposalReject extends ParsedMultisigProgramInstruction {
  const ParsedProposalReject({required this.data})
      : super(MultisigProgramInstruction.proposalReject);

  final ProposalRejectInstructionData data;
}

/// A parsed ProposalRevoke instruction.
final class ParsedProposalRevoke extends ParsedMultisigProgramInstruction {
  const ParsedProposalRevoke({required this.data})
      : super(MultisigProgramInstruction.proposalRevoke);

  final ProposalRevokeInstructionData data;
}

/// A parsed ProposalCancel instruction.
final class ParsedProposalCancel extends ParsedMultisigProgramInstruction {
  const ParsedProposalCancel({required this.data})
      : super(MultisigProgramInstruction.proposalCancel);

  final ProposalCancelInstructionData data;
}

/// A parsed VaultExecute instruction.
final class ParsedVaultExecute extends ParsedMultisigProgramInstruction {
  const ParsedVaultExecute({required this.data})
      : super(MultisigProgramInstruction.vaultExecute);

  final VaultExecuteInstructionData data;
}

/// A parsed ConfigExecute instruction.
final class ParsedConfigExecute extends ParsedMultisigProgramInstruction {
  const ParsedConfigExecute({required this.data})
      : super(MultisigProgramInstruction.configExecute);

  final ConfigExecuteInstructionData data;
}

/// A parsed ConfigAuthorityExecute instruction.
final class ParsedConfigAuthorityExecute extends ParsedMultisigProgramInstruction {
  const ParsedConfigAuthorityExecute({required this.data})
      : super(MultisigProgramInstruction.configAuthorityExecute);

  final ConfigAuthorityExecuteInstructionData data;
}

/// A parsed SpendingLimitUse instruction.
final class ParsedSpendingLimitUse extends ParsedMultisigProgramInstruction {
  const ParsedSpendingLimitUse({required this.data})
      : super(MultisigProgramInstruction.spendingLimitUse);

  final SpendingLimitUseInstructionData data;
}

/// A parsed ProposalClose instruction.
final class ParsedProposalClose extends ParsedMultisigProgramInstruction {
  const ParsedProposalClose({required this.data})
      : super(MultisigProgramInstruction.proposalClose);

  final ProposalCloseInstructionData data;
}

/// Parses a MultisigProgram instruction.
ParsedMultisigProgramInstruction parseMultisigProgramInstruction(
  Instruction instruction,
) {
  return switch (identifyMultisigProgramInstruction(
    instruction.data ?? Uint8List(0),
  )) {
    MultisigProgramInstruction.configInitialize => ParsedConfigInitialize(
      data: parseConfigInitializeInstruction(instruction),
    ),
    MultisigProgramInstruction.configUpdate => ParsedConfigUpdate(
      data: parseConfigUpdateInstruction(instruction),
    ),
    MultisigProgramInstruction.multisigCreate => ParsedMultisigCreate(
      data: parseMultisigCreateInstruction(instruction),
    ),
    MultisigProgramInstruction.multisigImport => ParsedMultisigImport(
      data: parseMultisigImportInstruction(instruction),
    ),
    MultisigProgramInstruction.proposalCreate => ParsedProposalCreate(
      data: parseProposalCreateInstruction(instruction),
    ),
    MultisigProgramInstruction.proposalActivate => ParsedProposalActivate(
      data: parseProposalActivateInstruction(instruction),
    ),
    MultisigProgramInstruction.proposalApprove => ParsedProposalApprove(
      data: parseProposalApproveInstruction(instruction),
    ),
    MultisigProgramInstruction.proposalReject => ParsedProposalReject(
      data: parseProposalRejectInstruction(instruction),
    ),
    MultisigProgramInstruction.proposalRevoke => ParsedProposalRevoke(
      data: parseProposalRevokeInstruction(instruction),
    ),
    MultisigProgramInstruction.proposalCancel => ParsedProposalCancel(
      data: parseProposalCancelInstruction(instruction),
    ),
    MultisigProgramInstruction.vaultExecute => ParsedVaultExecute(
      data: parseVaultExecuteInstruction(instruction),
    ),
    MultisigProgramInstruction.configExecute => ParsedConfigExecute(
      data: parseConfigExecuteInstruction(instruction),
    ),
    MultisigProgramInstruction.configAuthorityExecute => ParsedConfigAuthorityExecute(
      data: parseConfigAuthorityExecuteInstruction(instruction),
    ),
    MultisigProgramInstruction.spendingLimitUse => ParsedSpendingLimitUse(
      data: parseSpendingLimitUseInstruction(instruction),
    ),
    MultisigProgramInstruction.proposalClose => ParsedProposalClose(
      data: parseProposalCloseInstruction(instruction),
    ),
  };
}
