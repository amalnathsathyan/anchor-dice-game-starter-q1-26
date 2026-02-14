use anchor_instruction_sysvar::Ed25519InstructionSignatures;
use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};
use solana_program::{
    ed25519_program, hash::hash,
    sysvar::instructions::load_instruction_at_checked,
};

use crate::{errors::DiceError, Bet};

pub const HOUSE_EDGE: u16 = 150; //1.5% basis point

#[derive(Accounts)]

pub struct ResolveBet<'info> {
    #[account(mut)]
    pub house: Signer<'info>,

    ///CHECK: Player Account
    #[account(mut)]
    pub player: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"vault", house.key().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        mut,
        close = player,
        has_one = player,
        seeds = [b"bet", vault.key().as_ref(), bet.seed.to_le_bytes().as_ref()],
        bump = bet.bump
    )]
    pub bet: Account<'info, Bet>,
    ///CHECK: Instruction Sysvar Account
    #[account(
        address = solana_program::sysvar::instructions::ID
    )]
    pub instruction_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> ResolveBet<'info> {
    pub fn verify_ed25519_signature(&mut self, sig: &[u8]) -> Result<()> {
        let ix = load_instruction_at_checked(0, &self.instruction_sysvar.to_account_info())
            .map_err(|_| DiceError::Ed25519Program)?;

        require_eq!(
            ix.program_id,
            ed25519_program::ID,
            DiceError::Ed25519Program
        );
        require_eq!(ix.accounts.len(), 0, DiceError::Ed25519Accounts);

        let signatures = Ed25519InstructionSignatures::unpack(&ix.data)
            .map_err(|_| DiceError::Ed25519Signature)?
            .0;

        require_eq!(signatures.len(), 1, DiceError::Ed25519Signature);

        let signature = &signatures[0];

        //checks if the signature has a header
        require!(signature.is_verifiable, DiceError::Ed25519Header);

        //checks if the signature is signed by the house
        require_keys_eq!(
            signature.public_key.ok_or(DiceError::Ed25519Pubkey)?,
            self.house.key(),
            DiceError::Ed25519Pubkey
        );

        //checks if the instruction sysvar contains the same signature we passed in.
        require!(
            &signature
                .signature
                .ok_or(DiceError::Ed25519Signature)?
                .eq(sig),
            DiceError::Ed25519Signature
        );

        //checks if the instruction sysvar contains the same data we provided
        require!(
            &signature
                .message
                .as_ref()
                .ok_or(DiceError::Ed25519Message)?
                .eq(&self.bet.to_slice()),
            DiceError::Ed25519Message
        );

        Ok(())
    }

    pub fn resolve_bet(&mut self, bumps: &ResolveBetBumps, sig: &[u8]) -> Result<()> {
        let hash = hash(sig).to_bytes();
        let mut hash_16: [u8; 16] = [0; 16];
        hash_16.copy_from_slice(&hash[0..16]);
        let lower = u128::from_le_bytes(hash_16);
        hash_16.copy_from_slice(&hash[16..]);
        let upper = u128::from_be_bytes(hash_16);

        //some random operations so that roll is never zero
        //and here we have a random number, actually verified to be signed by the house
        let roll = lower.wrapping_add(upper).wrapping_rem(100) as u8 + 1;
        if self.bet.roll > roll {
            let payout = (self.bet.amount as u128)
                .checked_mul(10000 - HOUSE_EDGE as u128).ok_or(DiceError::Overflow)?
                .checked_div(self.bet.roll as u128 - 1).ok_or(DiceError::Overflow)?
                .checked_div(100).ok_or(DiceError::Overflow)?;

            let signer_seeds: &[&[&[u8]]] = &[&[b"vault", &self.house.key().to_bytes(),&[bumps.vault]]];

            let transfer_accounts = Transfer {
                from: self.vault.to_account_info(),
                to: self.player.to_account_info(),
            };

            let ctx = CpiContext::new_with_signer(
                self.system_program.to_account_info(),
                transfer_accounts,
                signer_seeds,
            );
            transfer(ctx, payout as u64);
        }
        Ok(())
    }
}
