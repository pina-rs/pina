//! Well-known Solana program and sysvar IDs.
//!
//! Each sub-module declares a single `ID` constant via
//! [`solana_address::declare_id!`]. Import the module and use `module::ID` to
//! reference the address.
//! Each module below declares a canonical Solana `ID` constant using
//! `solana_address::declare_id!`.

#![no_std]

/// Address Lookup Table program ID.
pub mod address_lookup_table {
	solana_address::declare_id!("AddressLookupTab1e1111111111111111111111111");
}

/// BPF Loader v2 program ID.
pub mod bpf_loader {
	solana_address::declare_id!("BPFLoader2111111111111111111111111111111111");
}

/// Legacy BPF Loader v1 program ID.
pub mod bpf_loader_deprecated {
	solana_address::declare_id!("BPFLoader1111111111111111111111111111111111");
}

/// Upgradeable BPF loader program ID.
pub mod bpf_loader_upgradeable {
	solana_address::declare_id!("BPFLoaderUpgradeab1e11111111111111111111111");
}

/// Compute Budget program ID.
pub mod compute_budget {
	solana_address::declare_id!("ComputeBudget111111111111111111111111111111");
}

/// Config program ID.
pub mod config {
	solana_address::declare_id!("Config1111111111111111111111111111111111111");
}

/// Ed25519 signature verification program ID.
pub mod ed25519_program {
	solana_address::declare_id!("Ed25519SigVerify111111111111111111111111111");
}

/// Feature activation program ID.
pub mod feature {
	solana_address::declare_id!("Feature111111111111111111111111111111111111");
}

/// A designated address for burning lamports.
///
/// Lamports credited to this address will be removed from the total supply
/// (burned) at the end of the current block.
pub mod incinerator {
	solana_address::declare_id!("1nc1nerator11111111111111111111111111111111");
}

/// Loader v4 program ID.
pub mod loader_v4 {
	solana_address::declare_id!("LoaderV411111111111111111111111111111111111");
}

/// Native loader program ID.
pub mod native_loader {
	solana_address::declare_id!("NativeLoader1111111111111111111111111111111");
}

/// Secp256k1 signature verification program ID.
pub mod secp256k1_program {
	solana_address::declare_id!("KeccakSecp256k11111111111111111111111111111");
}

/// Secp256r1 signature verification program ID.
pub mod secp256r1_program {
	solana_address::declare_id!("Secp256r1SigVerify1111111111111111111111111");
}

/// Stake program IDs.
pub mod stake {
	//! Canonical stake program IDs.

	/// Stake config account program ID.
	pub mod config {
		solana_address::declare_id!("StakeConfig11111111111111111111111111111111");
	}
	// Stake program ID.
	solana_address::declare_id!("Stake11111111111111111111111111111111111111");
}

/// System program ID.
pub mod system_program {
	solana_address::declare_id!("11111111111111111111111111111111");
}

/// Vote program ID.
pub mod vote {
	solana_address::declare_id!("Vote111111111111111111111111111111111111111");
}

/// Sysvar owner and individual sysvar account IDs.
pub mod sysvar {
	// Owner address for sysvar accounts
	solana_address::declare_id!("Sysvar1111111111111111111111111111111111111");
	/// Clock sysvar ID.
	pub mod clock {
		solana_address::declare_id!("SysvarC1ock11111111111111111111111111111111");
	}
	/// Epoch rewards sysvar ID.
	pub mod epoch_rewards {
		solana_address::declare_id!("SysvarEpochRewards1111111111111111111111111");
	}
	/// Epoch schedule sysvar ID.
	pub mod epoch_schedule {
		solana_address::declare_id!("SysvarEpochSchedu1e111111111111111111111111");
	}
	/// Fees sysvar ID.
	pub mod fees {
		solana_address::declare_id!("SysvarFees111111111111111111111111111111111");
	}
	/// Instructions sysvar ID.
	pub mod instructions {
		solana_address::declare_id!("Sysvar1nstructions1111111111111111111111111");
	}
	/// Last restart slot sysvar ID.
	pub mod last_restart_slot {
		solana_address::declare_id!("SysvarLastRestartS1ot1111111111111111111111");
	}
	/// Recent blockhashes sysvar ID (deprecated on modern clusters).
	pub mod recent_blockhashes {
		solana_address::declare_id!("SysvarRecentB1ockHashes11111111111111111111");
	}
	/// Rent sysvar ID.
	pub mod rent {
		solana_address::declare_id!("SysvarRent111111111111111111111111111111111");
	}
	/// Rewards sysvar ID.
	pub mod rewards {
		solana_address::declare_id!("SysvarRewards111111111111111111111111111111");
	}
	/// Slot hashes sysvar ID.
	pub mod slot_hashes {
		solana_address::declare_id!("SysvarS1otHashes111111111111111111111111111");
	}
	/// Slot history sysvar ID.
	pub mod slot_history {
		solana_address::declare_id!("SysvarS1otHistory11111111111111111111111111");
	}
	/// Stake history sysvar ID.
	pub mod stake_history {
		solana_address::declare_id!("SysvarStakeHistory1111111111111111111111111");
	}
}

/// Zero-knowledge token proof program ID.
pub mod zk_token_proof_program {
	solana_address::declare_id!("ZkTokenProof1111111111111111111111111111111");
}

/// Zero-knowledge ElGamal proof program ID.
pub mod zk_elgamal_proof_program {
	solana_address::declare_id!("ZkE1Gama1Proof11111111111111111111111111111");
}
