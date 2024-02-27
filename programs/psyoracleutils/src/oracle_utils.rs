use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};
use pyth_sdk_solana::{load_price_feed_from_account_info, Price, PriceFeed};
use std::convert::TryFrom;
#[allow(unused_imports)]
use switchboard_v2::{decimal::SwitchboardDecimal, AggregatorAccountData, SWITCHBOARD_PROGRAM_ID};

use crate::errors::ErrorCode;

pub mod pyth_info {
    use anchor_lang::declare_id;
    #[cfg(feature = "localnet")]
    declare_id!("E6xiKCViJ2E6YyfFEa7eRZx3ngX4KPSVTSVTLywaEwJ8");

    #[cfg(all(not(feature = "localnet"), feature = "devnet-deploy"))]
    declare_id!("gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s");

    #[cfg(all(not(feature = "localnet"), not(feature = "devnet-deploy")))]
    declare_id!("FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH");
}

/// Default price decimals
pub const PRICE_DECIMALS: u8 = 8;

#[derive(Clone, Copy, AnchorDeserialize, AnchorSerialize)]
#[repr(u8)]
pub enum OracleProvider {
    PYTH = 0,
    SWITCHBOARD = 1,
    INTERNAL = 2,
}

unsafe impl Zeroable for OracleProvider {
    fn zeroed() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

unsafe impl Pod for OracleProvider {}

impl From<u8> for OracleProvider {
    fn from(orig: u8) -> Self {
        match orig {
            0 => OracleProvider::PYTH,
            1 => OracleProvider::SWITCHBOARD,
            _ => {
                panic!("Unsupported oracle provider")
            }
        }
    }
}

/// Validates Oracle matches Pyth or Switchboard and returns the respective id
///
/// If localnet flag is enabled, always returns Pyth, regardless of the owner id
#[allow(dead_code, unused_variables)]
pub fn validate_and_get_oracle_id<'info>(
    oracle: &UncheckedAccount<'info>,
) -> Result<OracleProvider> {
    #[cfg(feature = "localnet")]
    {
        return Ok(OracleProvider::PYTH);
    }
    #[cfg(not(feature = "localnet"))]
    {
        if oracle.owner == &SWITCHBOARD_PROGRAM_ID {
            // Validate switchboard account is AggregateData account
            let agg_account: AccountLoader<'info, AggregatorAccountData> =
                AccountLoader::try_from(oracle)?;
            agg_account.load()?.get_result()?;
            Ok(OracleProvider::SWITCHBOARD)
        } else if pyth_info::check_id(oracle.owner) {
            load_pyth_price(
                oracle,
                Clock::get().unwrap().unix_timestamp,
                u32::MAX,
                u64::MAX,
            )?;
            Ok(OracleProvider::PYTH)
        } else {
            return err!(ErrorCode::AccountMustBeOwnedByOracleProgram);
        }
    }
}

/// Validates Oracle matches Pyth or Switchboard
///
/// If localnet flag is enabled, always returns Ok
#[allow(dead_code, unused_variables)]
pub fn validate_oracle<'info>(
    oracle: &UncheckedAccount<'info>,
    oracle_provider_id: u8,
) -> Result<()> {
    #[cfg(feature = "localnet")]
    {
        return Ok(());
    }
    #[cfg(not(feature = "localnet"))]
    {
        match oracle_provider_id.into() {
            OracleProvider::PYTH => {
                // Validate pyth oracle is owned by the correct address
                if oracle.owner != &pyth_info::ID {
                    return err!(ErrorCode::AccountMustBeOwnedByOracleProgram);
                }

                load_pyth_price(
                    oracle,
                    Clock::get().unwrap().unix_timestamp,
                    u32::MAX,
                    u64::MAX,
                )?;
            }
            OracleProvider::SWITCHBOARD => {
                // Validate switchboard account is AggregateData account
                let agg_account: AccountLoader<'info, AggregatorAccountData> =
                    AccountLoader::try_from(oracle)?;
                agg_account.load()?.get_result()?;
            }
            OracleProvider::INTERNAL => return Ok(()),
        }
        Ok(())
    }
}

/// Get price for some asset. If you know the oracle type (Pyth or Switch), you can pass just the
/// validation values required, or just pass both regardless:
/// * conf_thresh_pyth for pyth
/// * conf_thresh_switch for switch
///
/// Panics if values required for validation are missing.
///
/// Rounds price up to the nearest lamport/unit if true, else rounds down.
pub fn get_oracle_price<'info>(
    oracle: &UncheckedAccount<'info>,
    desired_decimals: u8,
    current_time: i64,
    conf_thresh_pyth: Option<u32>,
    conf_thresh_switch: Option<f64>,
    max_age: u64,
    round_up: bool,
) -> Result<u64> {
    let converted_price = if oracle.owner == &SWITCHBOARD_PROGRAM_ID {
        if conf_thresh_switch.is_none() {
            panic!("Failed to provide paramters for a Switch query")
        }
        // Validate switchboard account is AggregateData account
        let agg_account: AccountLoader<'info, AggregatorAccountData> =
            AccountLoader::try_from(oracle)?;
        let acc = agg_account.load()?;

        // Validate confidence and age parameters
        let max_confidence_interval = SwitchboardDecimal::from_f64(conf_thresh_switch.unwrap());
        acc.check_confidence_interval(max_confidence_interval)
            .map_err(|_| error!(ErrorCode::OracleBadConfidence))?;
        // Note: safe case in most use cases, but uses try for sanity check
        let max_staleness: i64 = max_age.try_into().unwrap();
        acc.check_staleness(current_time, max_staleness)
            .map_err(|_| error!(ErrorCode::OraclePriceExpired))?;

        let price = acc.get_result()?;
        convert_switchboard_price(price, desired_decimals, round_up)?
    } else if pyth_info::check_id(oracle.owner) {
        if conf_thresh_pyth.is_none() {
            panic!("Failed to provide paramters for a Pyth query")
        }
        let (pyth_price, expo) =
            load_pyth_price(oracle, current_time, conf_thresh_pyth.unwrap(), max_age)?;

        convert_price_decimals(
            i128::from(pyth_price),
            expo.unsigned_abs(),
            desired_decimals,
            round_up,
        )?
    } else {
        return err!(ErrorCode::AccountMustBeOwnedByOracleProgram);
    };
    Ok(converted_price)
}

/// Fetches the pyth price from the oracle price feed given. Checks that price is within confidence
/// and freshness threshold.
///
/// Returns price, exponent.
///
/// Copied from PsyLend
#[allow(dead_code)]
#[allow(unused_variables)]
pub fn load_pyth_price(
    price_feed: &AccountInfo,
    current_time: i64,
    conf_threshold: u32,
    max_age: u64,
) -> Result<(i64, i32)> {
    let price_feed: PriceFeed = load_price_feed_from_account_info(price_feed).unwrap();
    let pyth_price: Price;
    let pyth_price_ema: Price;
    #[cfg(feature = "localnet")]
    {
        pyth_price = price_feed.get_price_unchecked();
        pyth_price_ema = price_feed
            .get_ema_price_no_older_than(current_time, u64::MAX)
            .unwrap();
    }
    #[cfg(not(feature = "localnet"))]
    {
        pyth_price = match price_feed.get_price_no_older_than(current_time, max_age) {
            Some(p) => p,
            None => return handle_expired_pyth_price(&price_feed, current_time),
        };
        pyth_price_ema = match price_feed.get_ema_price_no_older_than(current_time, max_age) {
            Some(p) => p,
            None => return handle_expired_pyth_price(&price_feed, current_time),
        };
    }

    // Expo should be negative
    if pyth_price.expo.is_positive() || pyth_price_ema.expo.is_positive() {
        return err!(ErrorCode::PythOracleMustHaveNegativeExpo);
    }

    // Price should be positive.
    if pyth_price.price <= 0 || pyth_price_ema.price <= 0 {
        return err!(ErrorCode::PythPriceWasNoneOrNegative);
    }

    if pyth_price_ema.expo != pyth_price.expo {
        panic!("Oracles where ema exponent differs from price exponent are not supported");
    }

    // Using TWAP conf for the spot price helps smooth price fluxuations and fail in high vol settings.
    // Safe cast to u64 because price is always positive here.
    let threshold = u64::try_from(
        u128::from(conf_threshold) * u128::from(pyth_price_ema.price.unsigned_abs())
            / u128::from(u32::MAX),
    )
    .unwrap();

    if pyth_price.conf > threshold {
        msg!(
            "pyth conf {:?} exceeds max accepted conf {:?}",
            pyth_price.conf,
            threshold
        );
        return err!(ErrorCode::OracleBadConfidence);
    }

    Ok((pyth_price.price, pyth_price.expo))
}

/// Logs Pyth price publish timing and returns expired price error.
#[allow(dead_code)]
#[allow(unused_variables)]
fn handle_expired_pyth_price(price_feed: &PriceFeed, current_time: i64) -> Result<(i64, i32)> {
    let publish_time = price_feed.get_price_unchecked().publish_time;
    let diff = publish_time.abs_diff(current_time);
    msg!(
        "time is: {:?} but price published at {:?} which was {:?} seconds ago (or in the future)",
        current_time,
        publish_time,
        diff
    );
    return err!(ErrorCode::OraclePriceExpired);
}

pub fn convert_switchboard_price(
    sb_value: SwitchboardDecimal,
    desired_decimals: u8,
    round_up: bool,
) -> Result<u64> {
    convert_price_decimals(
        sb_value.mantissa,
        sb_value.scale,
        desired_decimals,
        round_up,
    )
}

/// Used for decimal conversion of an u64 price. This is useful when getting an u64 price from an
/// oracle with a different decimal factor and converting to a desired decimal factor.
pub fn convert_price_decimals(
    starting_price: i128,
    starting_decimals: u32,
    desired_decimals: u8,
    round_up: bool,
) -> Result<u64> {
    let pow = i32::from(desired_decimals) - i32::try_from(starting_decimals).unwrap();
    let pow_u32 = pow.unsigned_abs();

    let multiplier = 10_i128.checked_pow(pow_u32).unwrap();
    // negative pow = must divide by the power, positive = multiply
    let value = if pow.is_negative() {
        if round_up {
            // Adjust for rounding up by adding one less than the multiplier
            // NOTE: Reduces valid input range, since prices are 64-bit it shouldn't matter.
            starting_price
                .checked_add(multiplier - 1)
                .ok_or(ErrorCode::OracleNumberOverflow)?
                .checked_div(multiplier)
                .ok_or(ErrorCode::OracleNumberOverflow)?
        } else {
            starting_price
                .checked_div(multiplier)
                .ok_or(ErrorCode::OracleNumberOverflow)?
        }
    } else {
        starting_price
            .checked_mul(multiplier)
            .ok_or(ErrorCode::OracleNumberOverflow)?
    };

    let res = u64::try_from(value).map_err(|_| ErrorCode::OracleNumberOverflow)?;
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switchboard_scale_up() {
        let switchboard_decimal = SwitchboardDecimal::new(3_143, 3);
        let new_scale = 5;
        let expected = 314_300;

        let result = convert_switchboard_price(switchboard_decimal, new_scale, false).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn switchboard_scale_down() {
        let switchboard_decimal = SwitchboardDecimal::new(3_141_592_653_589, 12);
        let new_scale = 1;
        let expected = 31;

        let result = convert_switchboard_price(switchboard_decimal, new_scale, false).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn convert_price_decimals_down() {
        let starting_price = 3_143;
        let starting_decimals = 3;
        let desired_decimals = 1;
        let expected = 31;

        let result =
            convert_price_decimals(starting_price, starting_decimals, desired_decimals, false)
                .unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn convert_price_decimals_up() {
        let starting_price = 3_143;
        let starting_decimals = 3;
        let desired_decimals = 7;
        let expected = 31_430_000;

        let result =
            convert_price_decimals(starting_price, starting_decimals, desired_decimals, false)
                .unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn convert_price_decimals_round_down() {
        let starting_price = 3_143_567;
        let starting_decimals = 6;
        let desired_decimals = 3;
        let expected = 3_143;

        let result =
            convert_price_decimals(starting_price, starting_decimals, desired_decimals, false)
                .unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn convert_price_decimals_round_up() {
        let starting_price = 3_143_567;
        let starting_decimals = 6;
        let desired_decimals = 3;
        let expected = 3_144;

        let result =
            convert_price_decimals(starting_price, starting_decimals, desired_decimals, true)
                .unwrap();
        assert_eq!(result, expected);
    }
}
