use crate::{
	pallet::{PoolIdAndAssetIdToVaultId, Pools as PoolIdToPoolInfo},
	mocks::{
		currency_factory::MockCurrencyId,
		tests::{
			ALICE, Balance, BOB, ExtBuilder, Origin, Pools, Test, Tokens, Vaults,
		},
	},
	// *,
};
use composable_traits::{
	pool::{
		Deposit, Pool, PoolConfig, Weight, WeightingMetric,
	},
	vault::{
		Vault,
	},
};

use crate::{Error};
use frame_support::{
	assert_noop, assert_ok,
	traits::fungibles::{Inspect, Mutate},
	sp_runtime::Perquintill,
};
use num_integer::Roots;
use sp_runtime::{
	traits::{
		One,
	},
};

// ----------------------------------------------------------------------------------------------------
//                                               Create                                              
// ----------------------------------------------------------------------------------------------------

#[test]
fn creating_a_pool_with_required_minimum_deposit_does_not_raise_an_error() {
	// Tests that if all the conditions are met to create a pool it will be created 
	//  |  successfully
	//  '-> Conditions:
	//        i.  deposit ≥ asset_ids.len() * (creation_deposit + existential_deposit)
	//        ii. user has ≥ deposit native tokens

	ExtBuilder::default().build().execute_with(|| {
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};

		// Condition i
		let deposit = Deposit {
			asset_id: MockCurrencyId::A,
			amount: 2_020, 
		};

		// Condition ii
		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));

		assert_ok!(Pools::create(Origin::signed(ALICE), config, deposit));
	});
}

#[test]
fn creating_a_pool_without_having_balance_trying_to_deposit_raises_an_error() {
	// Tests that if not all the conditions are met to create a pool it will not
	//  |  be created
	//  '-> Conditions:
	//        i.  deposit ≥ asset_ids.len() * (creation_deposit + existential_deposit)
	//        ii. user has ≥ deposit native tokens

	ExtBuilder::default().build().execute_with(|| {
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::A, MockCurrencyId::B],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};

		// Condition i
		let deposit = Deposit {
			asset_id: MockCurrencyId::A,
			amount: 2_020,
		};

		// does not mint tokens into ALICE's account - does not satisfy Condition ii

		// Condition ii
		assert_noop!(
			Pools::create(Origin::signed(ALICE), config, deposit), 
			Error::<Test>::IssuerDoesNotHaveBalanceTryingToDeposit
		);
	});
}

#[test]
fn creating_a_pool_without_required_minimum_creation_deposit_amount_raises_an_error() {
	// Tests that if not all the conditions are met to create a pool it will not
	//  |  be created
	//  '-> Conditions:
	//        i.  deposit ≥ asset_ids.len() * (creation_deposit + existential_deposit)
	//        ii. user has ≥ deposit native tokens

	ExtBuilder::default().build().execute_with(|| {
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::A, MockCurrencyId::B],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};
		// Creation deposit is too small - does not satisfy condition ii
		let deposit = Deposit {
			asset_id: MockCurrencyId::A,
			amount: 2_019,
		};

		// Condition ii
		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));

		assert_noop!(
			Pools::create(Origin::signed(ALICE), config, deposit), 
			Error::<Test>::InsufficientDepositToOpenPoolOfThisSize
		);
	});
}

#[test]
fn creating_a_pool_with_n_underlying_assets_tracks_n_seperate_vaults() {
	// Tests that when a pool is created to house n different assets, 
	//  |   PoolIdAndAssetIdToVaultId maintains n different
	//  |   key (pool_id, asset_id) -> value (vault_id) entries, one for each
	//  |   asset in the pool.
	//  '-> Conditions:
	//       i. a pool with n different (unique) assets must have n different
	//              (unique) underlying vaults

	ExtBuilder::default().build().execute_with(|| {
		let config = PoolConfig {
			asset_ids: vec![
				MockCurrencyId::A,    // To test for any value n < 7, comment out
				MockCurrencyId::B,    //     any of thesse Currencies.
				MockCurrencyId::C,
				MockCurrencyId::D,    // To test for any value n > 7, add extra
				MockCurrencyId::E,    //     MockCurrencyId's to mocks/currency_factor.rs
				MockCurrencyId::F,    //     and import them
				MockCurrencyId::G,
			],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};
		let number_of_assets = config.asset_ids.len();

		let deposit = Deposit {
			asset_id: MockCurrencyId::A,
			amount: 1_010 * number_of_assets as u128,
		};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 1_010 * number_of_assets as u128));
		assert_ok!(Pools::create(Origin::signed(ALICE), config.clone(), deposit));
		let pool_id = 1;

		// Condition i
		for asset_id in &config.asset_ids {
			assert_eq!(PoolIdAndAssetIdToVaultId::<Test>::contains_key(pool_id, *asset_id), true);
		}
	});
}

#[test]
fn creating_a_pool_transfers_creation_deposit_from_users_account_into_pools_account() {
	// Tests that when a user successfully creates a Pool their Creation fee is transfered 
	//  |  into the Pools account
	//  |-> Pre-Conditions:
	//  |     i.   user (U) has at least n ≥ CreationFee native tokens in their account
	//  '-> Post-Conditions:
	//        ii.  user (U) has n` = n - △ native tokens in their account, where △ = CreationFee
	//        iii. pool (P) has △ native tokens in its account

	ExtBuilder::default().build().execute_with(|| {
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};

		// Condition i
		let deposit = Deposit {
			asset_id: MockCurrencyId::A,
			amount: 2_020, // Condition i
		};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));

		// Condition i - Alice originally has the native tokens
		let creation_fee = 2_020;
		assert_eq!(Tokens::balance(MockCurrencyId::A, &ALICE), creation_fee);

		assert_ok!(Pools::create(Origin::signed(ALICE), config, deposit));

		// Condition ii - Deposit is transfered out of Alices account
		assert_eq!(Tokens::balance(MockCurrencyId::A, &ALICE), 0);
		//Condition iii -  Deposit is transfered into Pools account
		assert_eq!(Tokens::balance(MockCurrencyId::A, &<Pools as Pool>::account_id(&1)), 2_020);
	});
}

// ----------------------------------------------------------------------------------------------------
//                                               Deposit                                              
// ----------------------------------------------------------------------------------------------------


#[test]
fn trying_to_deposit_to_a_pool_that_does_not_exist_raises_an_error() {
	// Tests that when trying to deposit assets into a pool using a pool id 
	//  |  that doesn't correspond to an active pool, then the deposit 
	//  |  extrinsic raises an error
	//  '-> Condition
	//        i. ∀ deposits d ⇒ pool_id must exist

	ExtBuilder::default().build().execute_with(|| {
		// No pool has been created
		let pool_id = 1;

		// Condition i
		let deposit = vec![Deposit{asset_id: MockCurrencyId::A, amount: 1_010}];
		assert_noop!(
			Pools::deposit(Origin::signed(ALICE), pool_id, deposit),
			Error::<Test>::PoolDoesNotExist
		);
	});
}

#[test]
fn trying_to_deposit_an_amount_larger_than_issuer_has_raises_an_error() {
	// Tests that the deposit extrinsic checks that the issuer has the balance
	//  |  they are trying to deposit
	//  '-> Conditions:
	//        i. ∀ deposits d of assets a1 ... an ⇒ user U has ≥ asset ai

	ExtBuilder::default().build().execute_with(|| {
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};
		let deposit = Deposit {
			asset_id: MockCurrencyId::A,
			amount: 2_020,
		};
		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));

		assert_ok!(Pools::create(Origin::signed(ALICE), config.clone(), deposit));
		let pool_id = 1;

		// don't mint any tokens of type MockCurrencyId::B or MockCurrencyId::C -
		//     does not satisfy condition i

		// Condition i
		let deposit = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 1_010},
			Deposit {asset_id: MockCurrencyId::C, amount: 1_010},
		];
		assert_noop!(
			Pools::deposit(Origin::signed(ALICE), pool_id, deposit),
			Error::<Test>::IssuerDoesNotHaveBalanceTryingToDeposit
		);
	});
}

#[test]
fn trying_to_deposit_an_amount_smaller_than_minimum_deposit_raises_an_error() {
	// Tests that, when trying to deposit assets into a pool and the amount
	//  |  being deposited is smaller than the pools minimum deposit requirement, 
	//  |  the deposit extrinsic raises an error
	//  '-> Condition
	//        i. ∀ deposits d with assets a1 ... an ⇒ ai >= min_deposit, where 1 ≤ i ≤ n

	ExtBuilder::default().build().execute_with(|| {
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_percent(10),
			deposit_max: Perquintill::from_percent(100),
			withdraw_min: Perquintill::from_percent(10),
			withdraw_max: Perquintill::from_percent(100),
			weighting_metric: WeightingMetric::Equal,
		};
		let deposit = Deposit {
			asset_id: MockCurrencyId::A,
			amount: 2_020,
		};
		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));

		assert_ok!(Pools::create(Origin::signed(ALICE), config.clone(), deposit));
		let pool_id = 1;

		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_000));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE, 1_000));

		let deposit = vec![
			Deposit{asset_id: MockCurrencyId::B, amount: 1_000},
			Deposit{asset_id: MockCurrencyId::C, amount: 1_000},
		];

		assert_ok!(Pools::deposit(Origin::signed(ALICE), pool_id, deposit));

		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_000));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE, 1_000));

		// Condition i
		let deposit = vec![
			Deposit{asset_id: MockCurrencyId::B, amount: 100},
			Deposit{asset_id: MockCurrencyId::C, amount: 100},
		];
		assert_noop!(
			Pools::deposit(Origin::signed(ALICE), pool_id, deposit),
			Error::<Test>::AmountMustBeGreaterThanMinimumDeposit
		);
	});
}

#[test]
fn trying_to_deposit_an_amount_that_doesnt_match_weight_metric_raises_an_error() {
	// Test that when depositing funds into a pool that the ratio of each asset deposited corresponds
	//  |  to the assets weight in the pool
	//  |-> Pre-Conditions:
	//  |     i.  Pool P is has weights w1 ... wn for assets a1 ... an
	//  '-> Post-Conditions:
	//        ii. ∀ deposits d consisting of assets a1 ... an ⇒ (ai / total(a1 ... an)) = wi, where 1 ≤ i ≤ n

	ExtBuilder::default().build().execute_with(|| {
		// Creating Pool
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			// Condition i
			weighting_metric: WeightingMetric::Equal,
		};
		let deposit = Deposit {asset_id: MockCurrencyId::A, amount: 2_020};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));
		assert_ok!(Pools::create(Origin::signed(ALICE), config, deposit));
		let pool_id = 1;

		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_000));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE,   999));

		let deposit = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 1_000},
			Deposit {asset_id: MockCurrencyId::C, amount:   999},
		];

		// Condition ii
		assert_noop!(
			Pools::deposit(Origin::signed(ALICE), pool_id, deposit),
			Error::<Test>::DepositDoesNotMatchWeightingMetric
		);
	});
}

#[test]
fn trying_to_deposit_an_amount_that_doesnt_match_weight_metric_raises_an_error_2() {
	// Test that when depositing funds into a pool that the ratio of each asset deposited corresponds
	//  |  to the assets weight in the pool
	//  |-> Pre-Conditions:
	//  |     i.  Pool P is has weights w1 ... wn for assets a1 ... an
	//  '-> Post-Conditions:
	//        ii. ∀ deposits d consisting of assets a1 ... an ⇒ (ai / total(a1 ... an)) = wi, where 1 ≤ i ≤ n

	ExtBuilder::default().build().execute_with(|| {
		// Pool Creation
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C, MockCurrencyId::D],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			// Condition i
			weighting_metric: WeightingMetric::Fixed(
				vec![
					Weight {
						asset_id: MockCurrencyId::B,
						weight: Perquintill::from_percent(50),
					},
					Weight {
						asset_id: MockCurrencyId::C,
						weight: Perquintill::from_percent(25),
					},
					Weight {
						asset_id: MockCurrencyId::D,
						weight: Perquintill::from_percent(25),
					},
				]
			),
		};
		let deposit = Deposit {asset_id: MockCurrencyId::A, amount: 3_030};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 3_030));
		assert_ok!(Pools::create(Origin::signed(ALICE), config, deposit));
		let pool_id = 1;

		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_000));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE,   501));
		assert_ok!(Tokens::mint_into(MockCurrencyId::D, &ALICE,   500));

		// Alice Deposits
		let deposits = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 1_000},
			Deposit {asset_id: MockCurrencyId::C, amount:   501},
			Deposit {asset_id: MockCurrencyId::D, amount:   500},
		];

		// Condition ii
		assert_noop!(
			Pools::deposit(Origin::signed(ALICE), pool_id, deposits),
			Error::<Test>::DepositDoesNotMatchWeightingMetric
		);
	});
}
	

#[test]
fn deposit_adds_amounts_to_underlying_vaults_and_removes_from_depositer() {
	// Tests that after the deposit extrinsic has been called, the assets deposited
	//  |  have been transfered out of the issuers account and into the underlying 
	//  |  vaults account.
	//  |-> Pre-Conditions:
	//  |     i.   ∀ deposits d of assets a1 ... an ⇒  user  U has ui asset ai before the deposit
	//  |     ii.  ∀ deposits d of assets a1 ... an ⇒ vault Vi has pi asset ai before the deposit
	//  '-> Post-Conditions:
	//        iii. ∀ deposits d of assets a1 ... an ⇒  user  U has ui - △i of asset ai after the deposit
	//        iv.  ∀ deposits d of assets a1 ... an ⇒ vault Vi has pi + △i of asset ai after the deposit
	
	ExtBuilder::default().build().execute_with(|| {
		// Creating Pool
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};
		let deposit = Deposit {asset_id: MockCurrencyId::A, amount: 2_020};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));
		assert_ok!(Pools::create(Origin::signed(ALICE), config, deposit));
		
		let pool_id = 1;

		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_010));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE, 1_010));

		// Pre-Conditions
		for asset_id in &vec![MockCurrencyId::B, MockCurrencyId::C] {
			// Condition i
			assert_eq!(Tokens::balance(*asset_id, &ALICE), 1_010);

			let vault_id = PoolIdAndAssetIdToVaultId::<Test>::get(pool_id, *asset_id);
			let vault_account = Vaults::account_id(&vault_id);

			// Condition ii
			assert_eq!(Tokens::balance(*asset_id, &vault_account), 0);
		}
	
		let deposit = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 1_010},
			Deposit {asset_id: MockCurrencyId::C, amount: 1_010},
		];

		// Depositing into Pool
		assert_ok!(Pools::deposit(Origin::signed(ALICE), pool_id, deposit));

		// Post-Conditions
		for asset_id in &vec![MockCurrencyId::B, MockCurrencyId::C] {
			// Condition iii
			assert_eq!(Tokens::balance(*asset_id, &ALICE), 0);

			let vault_id = PoolIdAndAssetIdToVaultId::<Test>::get(pool_id, *asset_id);
			let vault_account = Vaults::account_id(&vault_id);

			// Condition iv
			assert_eq!(Tokens::balance(*asset_id, &vault_account), 1_010);
		}
	});
}

#[test]
fn deposit_adds_gemoetric_mean_of_deposits_as_lp_tokens() {
	// Tests that when depositing to a newly created pool, an amount of lp tokens
	//  |  equivalent to the geometric mean of the deposit amounts is minted into 
	//  |  issuers account
	//  '-> Conditions:
	//        i. ∀ deposits d of assets a1 ... an (into an empty pool) 
	//               ⇒ lp_tokens_minted (lp) = nth-√(Π ai), where 1 ≤ i ≤ n
	
	ExtBuilder::default().build().execute_with(|| {
		// Pool Creation
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::D],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};
		let deposit = Deposit {asset_id: MockCurrencyId::A, amount: 2_020};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));
		assert_ok!(Pools::create(Origin::signed(ALICE), config, deposit));
		let pool_id = 1;

		// Mint tokens into Alices account
		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_010));
		assert_ok!(Tokens::mint_into(MockCurrencyId::D, &ALICE, 1_010));

		// Depositing balance into pool's underlying vaults
		let deposit = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 1_010},
			Deposit {asset_id: MockCurrencyId::D, amount: 1_010},
		];
		assert_ok!(Pools::deposit(Origin::signed(ALICE), pool_id, deposit));

		let lp_token_id = PoolIdToPoolInfo::<Test>::get(&pool_id).lp_token_id;
		let geometic_mean = 1_010;

		// Condition i
		assert_eq!(Tokens::balance(lp_token_id, &ALICE), geometic_mean);
	});
}

#[test]
fn deposit_into_nonempty_pool_mints_lp_tokens_proportional_to_ratio_of_assets_deposited() {
	// Tests that when a user is depositing assets into a non-empty pool (i.e. a pool
	//  |  that has already minted lp tokens), the amount of lp tokens minted is
	//  |  equivalent to the ratio of deposited assets to the pools balance of 
	//  |  each asset.
	//  |-> Pre-Conditions:
	//  |     i.  circulating supply of lp tokens (c) > 0
	//  '-> Post-Conditions:
	//        ii. ∀ deposits d of assets a1 ... an (into a non-empty pool) 
	//               ⇒ lp_tokens_minted (lp) = c x Σ (wi x (di/bi)),
	//                   where <  1 ≤ i ≤ n
	//                          | wi = the weight of asset i for the pool P
	//                          | di = the balance deposited of asset i
	//                          | bi = the balance of asset i in pool P before the deposit

	ExtBuilder::default().build().execute_with(|| {
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};
     let deposit = Deposit {asset_id: MockCurrencyId::A, amount: 2_020};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));
		assert_ok!(Pools::create(Origin::signed(ALICE), config, deposit));
		let pool_id = 1;

		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_010));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE, 1_010));

		let deposit = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 1_010},
			Deposit {asset_id: MockCurrencyId::C, amount: 1_010},
		];
		// Alice deposits
		assert_ok!(Pools::deposit(Origin::signed(ALICE), pool_id, deposit));

		// Condition i
		assert!(
			PoolIdToPoolInfo::<Test>::get(pool_id).lp_circulating_supply > 0
		);

		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &BOB, 1_010));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &BOB, 1_010));

		// depositing half as many assets should print half as many lp tokens
		let deposit = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 505},
			Deposit {asset_id: MockCurrencyId::C, amount: 505},
		];
		// Bob deposits
		assert_ok!(Pools::deposit(Origin::signed(BOB), pool_id, deposit));

		let lp_token_id = PoolIdToPoolInfo::<Test>::get(&pool_id).lp_token_id;
		let lp_tokens_minted = 505;

		// Condition ii
		assert_eq!(Tokens::balance(lp_token_id, &BOB), lp_tokens_minted);
	});
}

#[test]
fn depositing_funds_into_underlying_vaults_mints_vaults_lp_tokens_into_the_pools_account_and_not_issuers_account() {
	// Test that the lp tokens that are minted by the underlying vaults are kept by
	//  |  the Pools account and the issuer of the extrinsic never receives them
	//  |-> Pre-Conditions:
	//  |     i.   ∀ deposits d ⇒ pool (P) has πi of lp tokens from vault Vi before the deposit
	//  |     ii.  ∀ deposits d ⇒ user (U) has 0 lp tokens from vault Vi before the deposit
	//  '-> Post-Conditions:
	//        iii. ∀ deposits d ⇒ pool (P) has πi + △i of lp tokens from vault Vi after the deposit
	//                 where △i corresponds to the number of lp tokens minted by vaut Vi
	//        iv.  ∀ deposits d ⇒ user (U) has 0 lp tokens from vault Vi after the deposit

	ExtBuilder::default().build().execute_with(|| {
		// Pool Creation
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};
		let deposit = Deposit {asset_id: MockCurrencyId::A, amount: 2_020};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));
		assert_ok!(Pools::create(Origin::signed(ALICE), config, deposit));
		let pool_id = 1;

		let pool_account = <Pools as Pool>::account_id(&pool_id);

		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_010));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE, 1_010));

		let deposits = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 1_010},
			Deposit {asset_id: MockCurrencyId::C, amount: 1_010},
		];

		// Pre-Conditions
		for deposit in &deposits {
			let vault_id = PoolIdAndAssetIdToVaultId::<Test>::get(pool_id, deposit.asset_id);
			let vault_lp_token_id = Vaults::lp_asset_id(&vault_id).unwrap();

			// Condition i
			assert_eq!(Tokens::balance(vault_lp_token_id, &pool_account), 0);
			// Condition ii
			assert_eq!(Tokens::balance(vault_lp_token_id, &ALICE), 0);
		}

		// Depositing into Pool
		assert_ok!(Pools::deposit(Origin::signed(ALICE), pool_id, deposits.clone()));

		// Post-Conditions
		for deposit in &deposits {
			let vault_id = PoolIdAndAssetIdToVaultId::<Test>::get(pool_id, deposit.asset_id);
			let vault_lp_token_id = Vaults::lp_asset_id(&vault_id).unwrap();

			// The pool holds the vaults LP tokens
			assert_eq!(Tokens::balance(vault_lp_token_id, &pool_account), deposit.amount);
			// Alice never holds the vaults LP tokens
			assert_eq!(Tokens::balance(vault_lp_token_id, &ALICE), 0);
		}
	});
}

#[test]
fn depositing_funds_into_pool_keeps_track_of_circulating_supply() {
	// Test that all lp tokens that all are minted by the pool are kept track of
	//  |  by the pool
	//  |-> Pre-Conditions:
	//  |     i.  ∀ deposits d ⇒ pool (P) keeps track of π lp tokens in circulation before the deposit
	//  '-> Post-Conditions:
	//        ii. ∀ deposits d ⇒ pool (P) keeps track of π + △ lp tokens in circulation after the deposit,
	//                where △ = hte number of lp tokens minted from the deposit

	ExtBuilder::default().build().execute_with(|| {
		// Pool Creation
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};
		let deposit = Deposit {asset_id: MockCurrencyId::A, amount: 2_020};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));
		assert_ok!(Pools::create(Origin::signed(ALICE), config, deposit));
		let pool_id = 1;

		let lp_token_id = PoolIdToPoolInfo::<Test>::get(pool_id).lp_token_id;

		// Condition i
		assert_eq!(
			Tokens::balance(lp_token_id, &ALICE),
			0
		);

		// Alice Deposits
		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_010));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE, 1_010));

		let deposits = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 1_010},
			Deposit {asset_id: MockCurrencyId::C, amount: 1_010},
		];
		assert_ok!(Pools::deposit(Origin::signed(ALICE), pool_id, deposits.clone()));

		// Condition ii (for Alices deposti) & Condition i (for Bobs deposit)
		assert_eq!(
			Tokens::balance(lp_token_id, &ALICE),
			PoolIdToPoolInfo::<Test>::get(pool_id).lp_circulating_supply
		);

		// Bob Deposits
		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &BOB, 1_010));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &BOB, 1_010));

		let deposits = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 505},
			Deposit {asset_id: MockCurrencyId::C, amount: 505},
		];
		assert_ok!(Pools::deposit(Origin::signed(BOB), pool_id, deposits.clone()));

		// Condition ii
		assert_eq!(
			Tokens::balance(lp_token_id, &ALICE) + Tokens::balance(lp_token_id, &BOB),
			PoolIdToPoolInfo::<Test>::get(pool_id).lp_circulating_supply
		);
	});
}

// ----------------------------------------------------------------------------------------------------
//                                               Withdraw                                              
// ----------------------------------------------------------------------------------------------------

#[test]
fn trying_to_withdraw_from_a_pool_that_does_not_exist_raises_an_error() {
	// Tests that when trying to withdraw assets from a pool using a pool id 
	//  |  that doesn't correspond to an active pool, then the withdraw 
	//  |  extrinsic raises an error
	//  '-> Condition
	//        i. ∀ withdraws w ⇒ pool_id must exist

	ExtBuilder::default().build().execute_with(|| {
		// No pool has been created

		let pool_id = 1;

		// Condition i
		let deposit = vec![Deposit{asset_id: MockCurrencyId::A, amount: 1_010}];
		assert_noop!(
			Pools::deposit(Origin::signed(ALICE), pool_id, deposit),
			Error::<Test>::PoolDoesNotExist
		);
	});
}

#[test]
fn withdrawing_an_amount_of_lp_tokens_greater_than_owned_raises_error() {
	// Test that, when a user tries to withdraw by depositing a number of lp tokens 
	//  |  greater than the amount they currently own, an error is thrown
	//  '-> Condition
	//        i. ∀ users U ⇒ lp_tokens_deposited ≥ lp_tokens_owned

	ExtBuilder::default().build().execute_with(|| {
		// Pool Creation
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};
		let deposit = Deposit {asset_id: MockCurrencyId::A, amount: 2_020};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));
		assert_ok!(Pools::create(Origin::signed(ALICE), config.clone(), deposit));
		let pool_id = 1;

		// ALICE does not deposit anything, and thus does not have any lp tokens

		// Condition i
		assert_noop!(
			Pools::withdraw(Origin::signed(ALICE), pool_id, 1),
			Error::<Test>::IssuerDoesNotHaveLpTokensTryingToDeposit
		);
	});
}

#[test]
fn trying_to_withdraw_an_amount_outside_withdraw_bounds_raises_an_error() {
	// Tests that, when trying to withdraw assets from a pool and the amount of lp tokens
	//  |  being deposited is smaller than the pools minimum withdraw requirement, 
	//  |  the withdraw extrinsic raises an error
	//  '-> Condition
	//        i.  ∀ withdraw w of lp tokens lp ⇒ lp share of ai ≥ min_withdraw
	//        ii. ∀ withdraw w of lp tokens lp ⇒ lp share of ai ≤ max_withdraw

	ExtBuilder::default().build().execute_with(|| {
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_percent(0),
			deposit_max: Perquintill::from_percent(100),
			withdraw_min: Perquintill::from_percent(1),
			withdraw_max: Perquintill::from_percent(30),
			weighting_metric: WeightingMetric::Equal,
		};
		let deposit = Deposit {
			asset_id: MockCurrencyId::A,
			amount: 2_020,
		};
		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));

		assert_ok!(Pools::create(Origin::signed(ALICE), config.clone(), deposit));
		let pool_id = 1;

		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_000));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE, 1_000));

		let deposit = vec![
			Deposit{asset_id: MockCurrencyId::B, amount: 1_000},
			Deposit{asset_id: MockCurrencyId::C, amount: 1_000},
		];
		assert_ok!(Pools::deposit(Origin::signed(ALICE), pool_id, deposit));

		// Condition i
		assert_noop!(
			Pools::withdraw(Origin::signed(ALICE), pool_id, 9),
			Error::<Test>::AmountMustBeGreaterThanMinimumWithdraw
		);

		// Condition ii
		assert_noop!(
			Pools::withdraw(Origin::signed(ALICE), pool_id, 301),
			Error::<Test>::AmountMustBeLessThanMaximumWithdraw
		);
	});
}

#[test]
fn withdraw_transfers_users_share_of_underlying_assets_from_pools_account_into_the_users_account() {
	// Test that when a user deposits assets into the pool, receives lp tokens and
	//  |  deposits those lp tokens back into the pool the user receives their share 
	//  |  of the pools assets and it is transfered out of the pools (and vaults) account
	//  '-> Conditions:
	//        i.   User no longer has pools lp tokens
	//        ii.  User receives their share of assets back
	//        iii. Pool account burns the received lp tokens
	//        iv.  Pool account no longer has vaults lp tokens
	//        v.   Underlying vaults hold a balance of Bt - △t tokens t
	//        vi.  Underlying vaults burn their native lp tokens

	ExtBuilder::default().build().execute_with(|| {
		// Pool is created
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};
		let deposit = Deposit {asset_id: MockCurrencyId::A, amount: 2_020};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));
		assert_ok!(Pools::create(Origin::signed(ALICE), config.clone(), deposit));
		let pool_id = 1;

		// Alice Deposits
		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_010));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE, 1_010));

		let alices_deposit = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 1_010},
			Deposit {asset_id: MockCurrencyId::C, amount: 1_010},
		];
		assert_ok!(Pools::deposit(Origin::signed(ALICE), pool_id, alices_deposit.clone()));
		
		// Bob deposits
		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &BOB, 505));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &BOB, 505));

		let bobs_deposit = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 505},
			Deposit {asset_id: MockCurrencyId::C, amount: 505},
		];
		assert_ok!(Pools::deposit(Origin::signed(BOB), pool_id, bobs_deposit.clone()));

		// ALICE will withdraw half of her assets
		let pool_account = <Pools as Pool>::account_id(&pool_id);
		let pool_lp_token = PoolIdToPoolInfo::<Test>::get(&pool_id).lp_token_id;

		let pool_lp_tokens_received = Tokens::balance(pool_lp_token, &ALICE);
		let withdraw = pool_lp_tokens_received/2;
		assert_ok!(Pools::withdraw(Origin::signed(ALICE), pool_id, withdraw));

		// Vailidity Checking

		// Condition i
		assert_eq!(Tokens::balance(pool_lp_token, &ALICE), pool_lp_tokens_received - withdraw);
		
		// Condition ii
		for deposit in &alices_deposit {
			assert_eq!(Tokens::balance(deposit.asset_id, &ALICE), deposit.amount/2);
		}

		// Condition iii
		assert_eq!(Tokens::balance(pool_lp_token, &pool_account), 0);
		
		for asset_id in &config.asset_ids {
			let vault_id = PoolIdAndAssetIdToVaultId::<Test>::get(pool_id, *asset_id);
			let vault_account = Vaults::account_id(&vault_id);

			let vault_lp_token_id = Vaults::lp_asset_id(&vault_id).unwrap();

			// Condition iv
			assert_eq!(
				Tokens::balance(vault_lp_token_id, &pool_account), 
				Tokens::balance(*asset_id, &vault_account) 
			);

			// Condition v
			assert_eq!(
				Tokens::balance(*asset_id, &vault_account), 
				1_010, 
			);

			// Condition vi
			assert_eq!(Tokens::balance(vault_lp_token_id, &vault_account), 0);
		}
	});
}

#[test]
fn withdrawing_funds_from_pool_updates_circulating_supply_of_lp_tokens() {
	// Test that the pool's counter of circulating supply of lp tokens is updated
	//  |  after lp tokens are burned after a withdraw.
	//  |-> Pre-Conditions:
	//  |     i.  ∀ withdraws w ⇒ pool (P) keeps track of π lp tokens in circulation before the withdraw
	//  '-> Post-Conditions:
	//        ii. ∀ withdraws w ⇒ pool (P) keeps track of π - △ lp tokens in circulation after the withdraw,
	//                where △ = the number of lp tokens burned from the withdraw

	ExtBuilder::default().build().execute_with(|| {
		// Pool Creation
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Equal,
		};
		let deposit = Deposit {asset_id: MockCurrencyId::A, amount: 2_020};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 2_020));
		assert_ok!(Pools::create(Origin::signed(ALICE), config, deposit));
		let pool_id = 1;

		// Alice Deposits
		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_010));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE, 1_010));

		let deposits = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 1_010},
			Deposit {asset_id: MockCurrencyId::C, amount: 1_010},
		];
		assert_ok!(Pools::deposit(Origin::signed(ALICE), pool_id, deposits.clone()));

		// Bob Deposits
		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &BOB, 1_010));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &BOB, 1_010));

		let deposits = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 505},
			Deposit {asset_id: MockCurrencyId::C, amount: 505},
		];
		assert_ok!(Pools::deposit(Origin::signed(BOB), pool_id, deposits.clone()));

		// Condition i
		let lp_token_id = PoolIdToPoolInfo::<Test>::get(pool_id).lp_token_id;
		assert_eq!(
			Tokens::balance(lp_token_id, &ALICE) + Tokens::balance(lp_token_id, &BOB),
			PoolIdToPoolInfo::<Test>::get(pool_id).lp_circulating_supply
		);

		// Alice withdraws half of here assets from the pool
		let pool_lp_token = PoolIdToPoolInfo::<Test>::get(&pool_id).lp_token_id;
		let withdraw = Tokens::balance(pool_lp_token, &ALICE)/2;
		assert_ok!(Pools::withdraw(Origin::signed(ALICE), pool_id, withdraw));

		// Condition ii
		assert_eq!(
			Tokens::balance(lp_token_id, &ALICE) + Tokens::balance(lp_token_id, &BOB),
			PoolIdToPoolInfo::<Test>::get(pool_id).lp_circulating_supply
		);
	});
}

#[test]
fn withdrawing_funds_from_a_pool_with_uneven_weights_correctly_calculates_asset_amounts_to_withdraw() {
	// Test that when withdrawing funds from a pool that has asset weights that aren't equal to each other
	//  |  the withdraw extrinsic calculates the proportionate amount of each asset that corresponds to
	//  |  the share of lp tokens deposited
	//  |-> Pre-Conditions:
	//  |     i.  Pool P is not equal-weighted
	//  '-> Post-Conditions:                       circulating_supply_lp - deposited_lp
	//        ii. ∀ withdraws w ⇒ wi = bi x (1 - --------------------------------------), where 1 ≤ i ≤ n
	//                                                    circulating_supply_lp         

	ExtBuilder::default().build().execute_with(|| {
		// Pool Creation
		let config = PoolConfig {
			asset_ids: vec![MockCurrencyId::B, MockCurrencyId::C, MockCurrencyId::D],
			manager: ALICE,
			deposit_min: Perquintill::from_perthousand(0),
			deposit_max: Perquintill::from_perthousand(1_000),
			withdraw_min: Perquintill::from_perthousand(0),
			withdraw_max: Perquintill::from_perthousand(1_000),
			weighting_metric: WeightingMetric::Fixed(
				vec![
					Weight {
						asset_id: MockCurrencyId::B,
						weight: Perquintill::from_percent(50),
					},
					Weight {
						asset_id: MockCurrencyId::C,
						weight: Perquintill::from_percent(25),
					},
					Weight {
						asset_id: MockCurrencyId::D,
						weight: Perquintill::from_percent(25),
					},
				]
			),
		};
		let deposit = Deposit {asset_id: MockCurrencyId::A, amount: 3_030};

		assert_ok!(Tokens::mint_into(MockCurrencyId::A, &ALICE, 3_030));
		assert_ok!(Pools::create(Origin::signed(ALICE), config, deposit));
		let pool_id = 1;

		// Condition i
		assert_ok!(Tokens::mint_into(MockCurrencyId::B, &ALICE, 1_000));
		assert_ok!(Tokens::mint_into(MockCurrencyId::C, &ALICE, 500));
		assert_ok!(Tokens::mint_into(MockCurrencyId::D, &ALICE, 500));

		// Alice Deposits
		let deposits = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 1_000},
			Deposit {asset_id: MockCurrencyId::C, amount: 500},
			Deposit {asset_id: MockCurrencyId::D, amount: 500},
		];
		assert_ok!(Pools::deposit(Origin::signed(ALICE), pool_id, deposits.clone()));

		// Alice Withdraws
		let pool_lp_token = PoolIdToPoolInfo::<Test>::get(&pool_id).lp_token_id;
		let withdraw = Tokens::balance(pool_lp_token, &ALICE) /10;
		assert_ok!(Pools::withdraw(Origin::signed(ALICE), pool_id, withdraw));

		let required_withdrawn = vec![
			Deposit {asset_id: MockCurrencyId::B, amount: 100},
			Deposit {asset_id: MockCurrencyId::C, amount: 50},
			Deposit {asset_id: MockCurrencyId::D, amount: 50},
		];

		// Condition ii
		for asset in &required_withdrawn {
			let epsilon = asset.amount / 100;

			assert!(
				(asset.amount - epsilon) <= Tokens::balance(asset.asset_id, &ALICE),
			);

			assert!(
				Tokens::balance(asset.asset_id, &ALICE) <= (asset.amount - epsilon),
			);
		}
	});
}

// ----------------------------------------------------------------------------------------------------
//                                                 Math                                                
// ----------------------------------------------------------------------------------------------------

fn geometric_mean(deposits: Vec<Balance>) -> Balance {
	let mut result = Balance::one();

	for deposit in &deposits {
		result = result.checked_mul(*deposit).unwrap();
	}

	let number_of_assets = deposits.len() as u32;
	
	result.nth_root(number_of_assets)
}

#[test]
fn test_calculating_geometric_mean() {
	ExtBuilder::default().build().execute_with(|| {
		let deposit1: Balance = 1_010;
		let deposit2: Balance = 1_010;
		let deposit3: Balance =   505;

		let deposits = vec![deposit1, deposit2];
		assert_eq!(1_010, geometric_mean(deposits));

		let deposits = vec![deposit1, deposit2, deposit3];
		assert_eq!(801, geometric_mean(deposits));

		assert_eq!(2_446, geometric_mean(vec![7, 9_073, 647, 30_579, 69_701]));
	});
}

#[test]
fn test_minimum_maximum_deposit_bounds_as_a_percentage() {
	let percent = 10;
	let minimum_deposit = Perquintill::from_percent(percent);
	let maximum_deposit = Perquintill::from_percent(100-percent);
	let pool_balance: Vec<Balance> = 
		vec![
			1_000, 
			2_000,
			3_000
		];

	let deposits: Vec<Balance> = 
		vec![
			100, 
			200,
			300
		]; 

	for index in 0..deposits.len() {
		assert!(deposits[index] >= minimum_deposit * pool_balance[index]);
		assert!(deposits[index] <= maximum_deposit * pool_balance[index]);
	}
}

#[test]
fn test_calculating_percentage_of_balance() {
	let percentage = Perquintill::from_percent(10);

	let balance: Balance = 1000;

	assert_eq!(percentage * balance, 100);
}

#[test]
fn test_summing_vector_of_perquintill_values() {
	let epsilon = Perquintill::from_float(0.0000000000000001 as f64).deconstruct();
	let one = Perquintill::one().deconstruct();

	for size in 13..14 {
		let percentages = vec![Perquintill::from_float(1 as f64 / size as f64); size as usize];
		let sum = percentages
			.iter()
			.map(|weight| weight.deconstruct())
			.sum();

		assert!(one - epsilon < sum && sum < one + epsilon);
	};
}
