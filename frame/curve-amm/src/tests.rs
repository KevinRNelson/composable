use frame_support::assert_ok;

use crate::mock::*;
use composable_traits::dex::CurveAmm as CurveAmmTrait;
use frame_support::traits::fungibles::{Inspect, Mutate};
use sp_runtime::{
	traits::{Saturating, Zero},
	FixedPointNumber, FixedU128, Permill,
};
use sp_std::cmp::Ordering;

#[test]
fn compute_d_works() {
	let xp = vec![
		FixedU128::saturating_from_rational(11u128, 10u128),
		FixedU128::saturating_from_rational(88u128, 100u128),
	];
	let amp = FixedU128::saturating_from_rational(292u128, 100u128);
	let ann = CurveAmm::get_ann(amp, xp.len()).unwrap();
	let d = CurveAmm::get_d(&xp, ann);
	// expected d is 1.978195735374521596
	// expected precision is 1e-13
	let delta = d
		.map(|x| {
			x.saturating_sub(FixedU128::saturating_from_rational(
				1978195735374521596u128,
				10_000_000_000_000_000u128,
			))
			.saturating_abs()
		})
		.map(|x| x.cmp(&FixedU128::saturating_from_rational(1u128, 10_000_000_000_000u128)));
	assert_eq!(delta, Some(Ordering::Less));
}

#[test]
fn compute_d_empty() {
	let xp = vec![];
	let amp = FixedU128::saturating_from_rational(292u128, 100u128);
	let ann = CurveAmm::get_ann(amp, xp.len()).unwrap();
	let result = CurveAmm::get_d(&xp, ann);
	assert_eq!(result, Some(FixedU128::zero()));
}

#[test]
fn get_y_successful() {
	let i = 0;
	let j = 1;
	let x = FixedU128::saturating_from_rational(111u128, 100u128);
	let xp = vec![
		FixedU128::saturating_from_rational(11u128, 10u128),
		FixedU128::saturating_from_rational(88u128, 100u128),
	];
	let amp = FixedU128::saturating_from_rational(292u128, 100u128);
	let ann = CurveAmm::get_ann(amp, xp.len()).unwrap();

	let result = CurveAmm::get_y(i, j, x, &xp, ann);
	// expected y is 1.247108067356516682
	// expected precision is 1e-13
	let delta = result
		.map(|x| {
			x.saturating_sub(FixedU128::saturating_from_rational(
				1247108067356516682u128,
				10_000_000_000_000_000u128,
			))
			.saturating_abs()
		})
		.map(|x| x.cmp(&FixedU128::saturating_from_rational(1u128, 10_000_000_000_000u128)));
	assert_eq!(delta, Some(Ordering::Less));
}

#[test]
fn get_y_same_coin() {
	let i = 1;
	let j = 1;
	let x = FixedU128::saturating_from_rational(111u128, 100u128);
	let xp = vec![
		FixedU128::saturating_from_rational(11u128, 10u128),
		FixedU128::saturating_from_rational(88u128, 100u128),
	];
	let amp = FixedU128::saturating_from_rational(292u128, 100u128);
	let ann = CurveAmm::get_ann(amp, xp.len()).unwrap();

	let result = CurveAmm::get_y(i, j, x, &xp, ann);

	assert_eq!(result, None);
}

#[test]
fn get_y_i_greater_than_n() {
	let i = 33;
	let j = 1;
	let x = FixedU128::saturating_from_rational(111u128, 100u128);
	let xp = vec![
		FixedU128::saturating_from_rational(11u128, 10u128),
		FixedU128::saturating_from_rational(88u128, 100u128),
	];
	let amp = FixedU128::saturating_from_rational(292u128, 100u128);
	let ann = CurveAmm::get_ann(amp, xp.len()).unwrap();

	let result = CurveAmm::get_y(i, j, x, &xp, ann);

	assert_eq!(result, None);
}

#[test]
fn get_y_j_greater_than_n() {
	let i = 1;
	let j = 33;
	let x = FixedU128::saturating_from_rational(111u128, 100u128);
	let xp = vec![
		FixedU128::saturating_from_rational(11u128, 10u128),
		FixedU128::saturating_from_rational(88u128, 100u128),
	];
	let amp = FixedU128::saturating_from_rational(292u128, 100u128);
	let ann = CurveAmm::get_ann(amp, xp.len()).unwrap();

	let result = CurveAmm::get_y(i, j, x, &xp, ann);

	assert_eq!(result, None);
}

#[test]
fn add_remove_liquidity() {
	new_test_ext().execute_with(|| {
		let assets = vec![MockCurrencyId::USDC, MockCurrencyId::USDT];
		let amp_coeff = FixedU128::saturating_from_rational(1000i128, 1i128);
		let fee = Permill::zero();
		let admin_fee = Permill::zero();

		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &ALICE), 0);
		assert_ok!(Tokens::mint_into(MockCurrencyId::USDT, &ALICE, 200000));
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &ALICE), 200000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &ALICE), 0);
		assert_ok!(Tokens::mint_into(MockCurrencyId::USDC, &ALICE, 200000));
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &ALICE), 200000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &BOB), 0);
		assert_ok!(Tokens::mint_into(MockCurrencyId::USDT, &BOB, 200000));
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &BOB), 200000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &BOB), 0);
		assert_ok!(Tokens::mint_into(MockCurrencyId::USDC, &BOB, 200000));
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &BOB), 200000);
		let p = CurveAmm::create_pool(&ALICE, assets, amp_coeff, fee, admin_fee);
		assert_ok!(&p);
		let pool_id = p.unwrap();
		let pool = CurveAmm::pool(pool_id);
		assert!(pool.is_some());
		let pool_lp_asset = CurveAmm::pool_lp_asset(pool_id);
		assert!(pool_lp_asset.is_some());
		let pool_lp_asset = pool_lp_asset.unwrap();
		// 1 USDC = 1 USDT
		let amounts = vec![130000u128, 130000u128];
		assert_ok!(CurveAmm::add_liquidity(&ALICE, pool_id, amounts.clone(), 0u128));
		let alice_balance = Tokens::balance(pool_lp_asset, &ALICE);
		assert_ne!(alice_balance, 0);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &ALICE), 200000 - 130000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &ALICE), 200000 - 130000);
		let pool = CurveAmm::pool(pool_id);
		assert!(pool.is_some());
		assert_ok!(CurveAmm::add_liquidity(&BOB, pool_id, amounts.clone(), 0u128));
		let bob_balance = Tokens::balance(pool_lp_asset, &BOB);
		assert_ne!(bob_balance, 0);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &BOB), 200000 - 130000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &BOB), 200000 - 130000);
		let min_amt = vec![0u128, 0u128];
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &CurveAmm::account_id(&pool_id)), 260000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &CurveAmm::account_id(&pool_id)), 260000);
		assert_ok!(CurveAmm::remove_liquidity(&ALICE, pool_id, alice_balance, min_amt.clone()));
		assert_eq!(Tokens::balance(pool_lp_asset, &ALICE), 0);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &ALICE), 200000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &ALICE), 200000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &CurveAmm::account_id(&pool_id)), 130000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &CurveAmm::account_id(&pool_id)), 130000);
		assert_ok!(CurveAmm::remove_liquidity(&BOB, pool_id, bob_balance, min_amt.clone()));
		assert_eq!(Tokens::balance(pool_lp_asset, &BOB), 0);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &BOB), 200000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &BOB), 200000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &CurveAmm::account_id(&pool_id)), 0);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &CurveAmm::account_id(&pool_id)), 0);
	});
}

#[test]
fn exchange_test() {
	new_test_ext().execute_with(|| {
		let assets = vec![MockCurrencyId::USDC, MockCurrencyId::USDT];
		let amp_coeff = FixedU128::saturating_from_rational(1000i128, 1i128);
		let fee = Permill::zero();
		let admin_fee = Permill::zero();

		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &ALICE), 0);
		assert_ok!(Tokens::mint_into(MockCurrencyId::USDT, &ALICE, 200000));
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &ALICE), 200000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &ALICE), 0);
		assert_ok!(Tokens::mint_into(MockCurrencyId::USDC, &ALICE, 200000));
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &ALICE), 200000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &BOB), 0);
		assert_ok!(Tokens::mint_into(MockCurrencyId::USDT, &BOB, 200000));
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &BOB), 200000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &BOB), 0);
		assert_ok!(Tokens::mint_into(MockCurrencyId::USDC, &BOB, 200000));
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &BOB), 200000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &CHARLIE), 0);
		assert_ok!(Tokens::mint_into(MockCurrencyId::USDT, &CHARLIE, 200000));
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &CHARLIE), 200000);
		let p = CurveAmm::create_pool(&ALICE, assets, amp_coeff, fee, admin_fee);
		assert_ok!(&p);
		let pool_id = p.unwrap();
		let pool = CurveAmm::pool(pool_id);
		let pool_lp_asset = CurveAmm::pool_lp_asset(pool_id);
		assert!(pool.is_some());
		assert!(pool_lp_asset.is_some());
		let pool_lp_asset = pool_lp_asset.unwrap();
		// 1 USDC = 1 USDT
		let amounts = vec![130000u128, 130000u128];
		assert_ok!(CurveAmm::add_liquidity(&ALICE, pool_id, amounts.clone(), 0u128));
		let alice_balance = Tokens::balance(pool_lp_asset, &ALICE);
		assert_ne!(alice_balance, 0);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &ALICE), 200000 - 130000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &ALICE), 200000 - 130000);
		let pool = CurveAmm::pool(pool_id);
		assert!(pool.is_some());
		assert_ok!(CurveAmm::add_liquidity(&BOB, pool_id, amounts.clone(), 0u128));
		let bob_balance = Tokens::balance(pool_lp_asset, &BOB);
		assert_ne!(bob_balance, 0);
		assert_eq!(Tokens::balance(MockCurrencyId::USDT, &BOB), 200000 - 130000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &BOB), 200000 - 130000);
		assert_eq!(Tokens::balance(MockCurrencyId::USDC, &CHARLIE), 0);
		assert_ok!(CurveAmm::exchange(&CHARLIE, pool_id, 1, 0, 65000, 0));
		assert!(65000 - Tokens::balance(MockCurrencyId::USDC, &CHARLIE) < 10);
	});
}
