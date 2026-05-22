use anchor_lang::prelude::*;

#[error_code]
pub enum PobError {
    ZeroAmount,
    InvalidLockDuration,
    MathOverflow,
    NoActiveStake,
    NoRewards,
    PositionLocked,
    InvalidMint,
    InvalidStakingVault,
    InvalidTokenAccount,
    InsufficientRewardReserve,
    InvalidPumpAccount,
    UnclaimedCreatorFees,
    CannotShortenLock,
    InvalidProtocolFeeVault,
    InvalidProgramAccount,
    UnauthorizedProtocolFeeClaim,
}
