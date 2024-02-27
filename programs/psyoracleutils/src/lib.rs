use anchor_lang::prelude::*;

declare_id!("GsaXofgSvd93FuJuNRcAfU6sCRNEc3wFjhdrrSkKFV4B");

#[program]
pub mod psyoracleutils {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
