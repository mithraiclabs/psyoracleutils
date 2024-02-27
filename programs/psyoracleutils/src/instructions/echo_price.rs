use anchor_lang::prelude::*;

use crate::oracle_utils::{get_oracle_price, PRICE_DECIMALS};

#[derive(Accounts)]
pub struct EchoOraclePrice<'info> {
    /// CHECK: If this is not a Pyth or Switch Oracle, will fail.
    pub some_oracle: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<EchoOraclePrice>) -> Result<()> {
    let some_price = get_oracle_price(
        &ctx.accounts.some_oracle,
        PRICE_DECIMALS,
        Clock::get().unwrap().unix_timestamp,
        Some(u32::MAX / 20), // 5%
        Some(5000 as f64),
        30, // 30 seconds
        true,
    )?;

    msg!("price: {:?}", some_price);

    Ok(())
}
