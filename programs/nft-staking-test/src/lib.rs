use anchor_lang::{prelude::*, solana_program};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

declare_id!("3pvNETr3Kqf4zTuUmfZUj3QzAHxAXk4icc7F5YsztLar");

#[program]
pub mod nft_staking_test {
    use anchor_spl::token::Transfer;

    use super::*;

    pub fn stake(ctx: Context<Stake>) -> Result<()> {
        // Check if user_info has been initialized
        if !ctx.accounts.user_info.is_initialized {
            ctx.accounts.user_info.is_initialized = true;
            ctx.accounts.user_info.point_balance = 0;
            ctx.accounts.user_info.active_stake = 0;
        }

        // Proceed to transfer
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_nft_account.to_account_info(),
            to: ctx.accounts.pda_nft_account.to_account_info(),
            authority: ctx.accounts.initializer.to_account_info(),
        };
        let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(token_transfer_context, 1)?;

        // Populate staking_info info
        ctx.accounts.staking_info.mint = ctx.accounts.mint.key();
        ctx.accounts.staking_info.staker = ctx.accounts.initializer.key();
        ctx.accounts.staking_info.stake_start_time = Clock::get().unwrap().unix_timestamp as u64;
        ctx.accounts.staking_info.last_stake_redeem = Clock::get().unwrap().unix_timestamp as u64;
        ctx.accounts.staking_info.stake_state = StakeState::Staked;

        // Add user_info active stake count by 1
        ctx.accounts.user_info.active_stake =
            ctx.accounts.user_info.active_stake.checked_add(1).unwrap();

        Ok(())
    }

    pub fn redeem(ctx: Context<Redeem>) -> Result<()> {
        // Calculate rewards
        let current_time = Clock::get().unwrap().unix_timestamp as u64;
        let amount = current_time - ctx.accounts.staking_info.last_stake_redeem;

        // Add amount to user_info point balance
        ctx.accounts.user_info.point_balance = ctx
            .accounts
            .user_info
            .point_balance
            .checked_add(amount)
            .unwrap();

        // Update staking_info last stake_redeem
        ctx.accounts.staking_info.last_stake_redeem = current_time;

        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        // Proceed to transfer
        let auth_bump = ctx.bumps.staking_info;
        let seeds = &[
            b"stake_info".as_ref(),
            &ctx.accounts.initializer.key().to_bytes(),
            &ctx.accounts.mint.key().to_bytes(),
            &[auth_bump],
        ];
        let signer = &[&seeds[..]];
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Transfer {
            from: ctx.accounts.pda_nft_account.to_account_info(),
            to: ctx.accounts.user_nft_account.to_account_info(),
            authority: ctx.accounts.staking_info.to_account_info(),
        };
        let token_transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(token_transfer_context, 1)?;

        // Calculate any remaining balance
        let current_time = Clock::get().unwrap().unix_timestamp as u64;
        let amount = current_time - ctx.accounts.staking_info.last_stake_redeem;

        ctx.accounts.user_info.point_balance = ctx
            .accounts
            .user_info
            .point_balance
            .checked_add(amount)
            .unwrap();
        ctx.accounts.staking_info.last_stake_redeem = current_time;

        ctx.accounts.staking_info.stake_state = StakeState::Unstaked;

        ctx.accounts.user_info.active_stake =
            ctx.accounts.user_info.active_stake.checked_sub(1).unwrap();

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Stake<'info> {
    // Check account seed and init if required
    #[account(
        init_if_needed, 
        seeds=[b"user", initializer.key().as_ref()], 
        bump, 
        payer = initializer, 
        space = 8 + UserInfo::INIT_SPACE
    )]
    pub user_info: Box<Account<'info, UserInfo>>,
    // Check account seed and init if required
    #[account(
        init_if_needed,
        payer = initializer, 
        seeds =[ b"stake_info", initializer.key().as_ref(), mint.key().as_ref()], 
        bump, 
        space = 8 + UserStakeInfo::INIT_SPACE)]
    pub staking_info: Box<Account<'info, UserStakeInfo>>,
    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,
    // Check if token account owner is the initializer and check if token amount = 1
    #[account(
        mut,
        constraint = user_nft_account.owner.key() == initializer.key(),
        constraint = user_nft_account.amount == 1
    )]
    pub user_nft_account: Box<Account<'info, TokenAccount>>,
    // Init if needed
    #[account(
        init_if_needed,
        payer = initializer, // If init required, payer will be initializer
        associated_token::mint = mint, // If init required, mint will be set to Mint
        associated_token::authority = staking_info // If init required, authority set to PDA
    )]
    pub pda_nft_account: Box<Account<'info, TokenAccount>>,
    // mint is required to create new account for PDA and for checking
    pub mint: Account<'info, Mint>,
    // Token Program required to call transfer instruction
    pub token_program: Program<'info, Token>,
    // ATA Program required to create ATA for pda_nft_account
    pub associated_token_program: Program<'info, AssociatedToken>,
    // System Program requred since a new account may be created and there's a deduction of lamports (fees/rent)
    pub system_program: Program<'info, System>,
    // Rent required to get Rent
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Redeem<'info> {
    // Check account seed, mut required to increase amount
    #[account(mut, seeds=[b"user", payer.key().as_ref()], bump )]
    pub user_info: Account<'info, UserInfo>,
    // Check account seed, mut required to update redeem time
    #[account(mut, seeds=[b"stake_info", payer.key().as_ref(), mint.key().as_ref()], bump)]
    pub staking_info: Account<'info, UserStakeInfo>,
    // Check if payer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub payer: Signer<'info>,
    // mint is required to check staking_info and pda_nft_account
    pub mint: Account<'info, Mint>,
    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut, seeds=[b"user", initializer.key().as_ref()], bump )]
    pub user_info: Account<'info, UserInfo>,
    // Check account seed and init if required
    #[account(
        mut, seeds=[b"stake_info", initializer.key().as_ref(), mint.key().as_ref()], bump,
        constraint = initializer.key() == staking_info.staker,
        close = initializer
    )]
    pub staking_info: Account<'info, UserStakeInfo>,
    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,
    // Check if token account owner is correct owner, mint and has amount of 0
    #[account(
        mut,
        constraint = user_nft_account.owner.key() == initializer.key(),
        constraint = user_nft_account.mint == mint.key(),
        constraint = user_nft_account.amount == 0
    )]
    pub user_nft_account: Account<'info, TokenAccount>,
    // Check if accounts has correct owner, mint and has amount of 1
    #[account(
        mut,
        constraint = pda_nft_account.owner == staking_info.key(),
        constraint = pda_nft_account.mint == mint.key(),
        constraint = pda_nft_account.amount == 1,
    )]
    pub pda_nft_account: Account<'info, TokenAccount>,
    // mint is required to check staking_info, user_nft_account, and pda_nft_account
    #[account(constraint = staking_info.mint == mint.key())]
    pub mint: Account<'info, Mint>,
    // Token Program required to call transfer instruction
    pub token_program: Program<'info, Token>,
    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct UserInfo {
    is_initialized: bool,
    point_balance: u64,
    active_stake: u16,
}

#[account]
#[derive(InitSpace)]
pub struct UserStakeInfo {
    staker: Pubkey,
    mint: Pubkey,
    stake_start_time: u64,
    last_stake_redeem: u64,
    stake_state: StakeState,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, InitSpace)]
pub enum StakeState {
    Staked,
    Unstaked,
}
