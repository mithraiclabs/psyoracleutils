use anchor_lang::prelude::*;

declare_id!("GsaXofgSvd93FuJuNRcAfU6sCRNEc3wFjhdrrSkKFV4B");

pub mod errors;
pub mod instructions;
pub mod oracle_utils;

use crate::instructions::*;

#[program]
pub mod psyoracleutils {
    use super::*;

    pub fn echo_price(ctx: Context<EchoOraclePrice>) -> Result<()> {
        instructions::echo_price::handler(ctx)
    }
}
