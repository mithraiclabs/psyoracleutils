use anchor_lang::error_code;

#[error_code]
pub enum ErrorCode {
    #[msg("Number overflow in oracle math, contact the global admin.")]
    OracleNumberOverflow, // 6000
    #[msg("Pyth oracle must have negative expo")]
    PythOracleMustHaveNegativeExpo, // 6001
    #[msg("Pyth Price negative, or None")]
    PythPriceWasNoneOrNegative, // 6002
    #[msg("Oracle program does not own oracle")]
    AccountMustBeOwnedByOracleProgram, // 6003
    #[msg("Last available Oracle price is too old, or an invalid time was provided.")]
    OraclePriceExpired, // 6004,
    #[msg("Price exceeds allowed confidence.")]
    OracleBadConfidence, // 6005,
}
